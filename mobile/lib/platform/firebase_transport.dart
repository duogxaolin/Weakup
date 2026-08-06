/// A [RemoteTransport] backed by Cloud Firestore, with FCM as a wake-up signal only.
///
/// # What this library is, and what it deliberately is not
///
/// This is plumbing. It writes documents, reads documents, and turns the bytes it finds into
/// [CommandEnvelope]s. It makes no judgement about any of them and has no way to express one —
/// the interface it implements offers no such member, and the source-text test in
/// `test/platform/firebase_transport_source_test.dart` fails the build if a name suggesting one
/// ever appears here.
///
/// That absence is the whole reason a hosted relay is an acceptable place to run this service.
/// The relay stores opaque bytes it cannot read; the target checks the signature itself against
/// a key the relay never holds. A compromised relay can drop a command, delay it, duplicate it,
/// reorder two, or return content it invented outright. None of those change what the target
/// does, because `evaluateCommand` recomputes the signature check from the bytes rather than
/// believing anything carried alongside them.
///
/// # The signed content is opaque here
///
/// [envelopeToDocument] writes the sender, the command name, the creation instant, the nonce,
/// and the signature bytes, and [documentToEnvelope] reads them back. Neither inspects what they
/// mean. The signature is carried as a Firestore `Blob` and is never parsed, truncated, or
/// normalised, because a byte changed in transit must produce a *signature check failure* at the
/// target rather than a repair here.
///
/// # Hand-written wire types (design D8)
///
/// The document shape is written by hand rather than generated, and **every field this code
/// depends on is parsed explicitly**: a missing field is an error, never a silent default. The
/// trade-off is stated in design D8 — an API change breaks at runtime rather than at compile
/// time — and explicit parsing is what turns that break into a named error instead of a command
/// that silently reads as something it is not.
///
/// # FCM wakes this device; the listener is what delivers (design D4)
///
/// A push carries **no command content**. It is a signal to look, and [FirebaseTransport.listen]
/// is what actually delivers the envelope. Putting command content in a push would place it in a
/// channel neither device controls, and a delivered-but-unread push would become a command the
/// target believes it never received.
///
/// This also degrades honestly: if push fails entirely — a revoked token, a Play Services
/// outage, a user who disabled notifications — the system still works with more latency, because
/// the listener remains the delivery path. `firebase_transport_test.dart` tests exactly that.
///
/// Mirrors `desktop/src-tauri/src/platform/firebase_transport.rs`.

// ignore_for_file: prefer_initializing_formals
library;

import 'dart:async';
import 'dart:convert';

import '../application/transport.dart';
import '../core/app_error.dart';
import '../domain/command_envelope.dart';
import '../domain/device_id.dart';
import '../domain/presence.dart';
import '../domain/remote_command.dart';

/// How often a running device reports that it is present.
///
/// **This is tied to [onlineThresholdSeconds], which is 90.** That threshold was chosen to
/// tolerate one missed report on a 60-second heartbeat plus jitter; reporting on this interval is
/// what makes it mean what its own documentation says. Changing this number without editing the
/// presence rule would silently change what "online" means for every device, in a file that does
/// not mention presence.
///
/// Quota is the other constraint and it is not close: two devices at 60 seconds is roughly 2,880
/// writes per day against a 20,000 free-tier limit. A 10-second interval would be 17,280 and
/// would leave almost nothing for commands.
const int presenceReportIntervalSeconds = 60;

/// A heartbeat at or beyond the online threshold would report a device as absent between two
/// consecutive successful reports, which is a false alarm rather than a detection.
///
/// Dart has no compile-time assertion over two constants from different libraries, so this is a
/// `const bool` the unit test pins. The Rust mirror checks the same relationship with
/// `const _: () = assert!(...)`, which fails the build outright.
const bool presenceIntervalFitsOnlineThreshold =
    presenceReportIntervalSeconds < onlineThresholdSeconds;

/// The collection holding one document per device in the account.
const String devicesCollection = 'devices';

/// The collection holding opaque signed envelopes addressed to one device.
const String commandsCollection = 'commands';

// ---------------------------------------------------------------------------
// The document seam.
// ---------------------------------------------------------------------------

/// One stored document, reduced to what this library actually needs.
///
/// A plain map rather than a `DocumentSnapshot` so the parsing below is exercised by tests that
/// never construct a Firestore client.
final class RelayDocument {
  const RelayDocument({required this.id, required this.fields});

  /// The document's own identifier, used to avoid delivering one twice in a session.
  final String id;

  /// The stored fields, exactly as the relay returned them.
  final Map<String, Object?> fields;
}

/// Where the bytes actually go.
///
/// Extracted as an interface for one reason: the hostile-relay tests need a relay that
/// fabricates, alters, and malforms responses, and they must run with no network. A stub
/// implementing this returns whatever a compromised relay could return, and the parsing and the
/// decision then run for real.
abstract interface class RelayDocuments {
  /// Creates or replaces one document in [collection].
  Future<void> write(String collection, String id, Map<String, Object?> fields);

  /// Merges [fields] into an existing document, leaving other fields alone.
  Future<void> merge(String collection, String id, Map<String, Object?> fields);

  /// Adds a document with a relay-assigned identifier.
  Future<void> add(String collection, Map<String, Object?> fields);

  /// Every document in [collection].
  Future<List<RelayDocument>> readAll(String collection);

  /// Documents in [collection] whose [field] equals [value].
  Future<List<RelayDocument>> readWhere(String collection, String field, Object? value);

  /// Emits whenever documents matching [field]/[value] change.
  ///
  /// This is what removes the need for a polling loop: a poll interval short enough to feel
  /// immediate would consume the free tier's daily read quota within hours.
  Stream<List<RelayDocument>> watchWhere(String collection, String field, Object? value);
}

/// The wake-up signal, separated from delivery.
///
/// Deliberately carries no envelope and no command name — see design D4 and the library comment.
/// The only thing an implementation may report is *that* something arrived.
abstract interface class WakeUpSignal {
  /// Emits when the relay suggests there may be something to collect.
  ///
  /// Carries no payload at all. A signal that carried a command would be a second delivery path,
  /// and the two could disagree.
  Stream<void> get signals;
}

/// A wake-up signal that never fires.
///
/// Not only a test double: this is the honest representation of a device whose push has failed
/// entirely — a revoked token, an outage, notifications switched off. Commands must still arrive
/// through the listener, which is the degradation design D4 claims and
/// `firebase_transport_test.dart` tests.
final class NoWakeUpSignal implements WakeUpSignal {
  const NoWakeUpSignal();

  @override
  Stream<void> get signals => const Stream<void>.empty();
}

// ---------------------------------------------------------------------------
// The transport.
// ---------------------------------------------------------------------------

/// Carries opaque envelopes between the account's devices through Firestore.
///
/// Holds no key, checks nothing, and reports no verdict. See the library comment.
final class FirebaseTransport implements RemoteTransport {
  // `prefer_initializing_formals` would have these spelled `this._documents` and
  // `this._wakeUp`. A private initializing formal is not nameable by a caller in another
  // library — `FirebaseTransport(_documents: ...)` does not compile — so taking the lint's
  // advice would make the constructor uncallable from the tests and from the app. Public
  // parameter names with private fields is the shape that works; the lint does not model it.
  FirebaseTransport({
    required this.deviceId,
    required RelayDocuments documents,
    WakeUpSignal wakeUp = const NoWakeUpSignal(),
  })  : _documents = documents,
        _wakeUp = wakeUp;

  /// The device this instance speaks for.
  ///
  /// Presence reports are scoped to it: [reportPresence] refuses to write a record for any other
  /// device, so this transport cannot make a peer appear present. The relay's own rules enforce
  /// the same thing; both exist because either alone would be a single point of failure for a
  /// property the spec states outright.
  final DeviceId deviceId;

  /// Where documents are read and written.
  ///
  /// Private and never exposed: a caller reaching the raw store could write a presence record
  /// for a peer, which is the one thing [reportPresence] exists to refuse.
  final RelayDocuments _documents;

  /// The wake-up signal, which carries no command content (design D4).
  final WakeUpSignal _wakeUp;

  /// Document ids already consumed, so a listener and a read cannot both deliver the same one
  /// twice within a session.
  ///
  /// Note what this is *not*: it is not replay defence. The nonce store in the acceptance rule is
  /// that, and it survives a restart. This only avoids pointless duplicate work, and the target
  /// would refuse a genuine duplicate regardless.
  final Set<String> _consumed = {};

  /// The interval a caller should report presence on.
  ///
  /// Exposed as a getter rather than left to the caller to remember, so the constant has exactly
  /// one reader and the comment tying it to the presence threshold is one hop from any call site.
  static Duration get presenceInterval =>
      const Duration(seconds: presenceReportIntervalSeconds);

  @override
  Future<void> registerDevice(DeviceId deviceId, String displayName) async {
    // A device writes its own record. Being listed confers nothing — pairing is a separate act —
    // so this is a directory entry, not a grant.
    await _documents.write(devicesCollection, deviceId.value, {
      'deviceId': deviceId.value,
      'displayName': displayName,
    });
  }

  @override
  Future<void> reportPresence(DeviceId deviceId, DateTime at) async {
    // A device reports only its own presence. Refused here rather than trusting the relay's rules
    // to refuse, so the property holds even against a relay whose rules failed entirely — the
    // same argument the acceptance rule makes about signatures.
    if (deviceId != this.deviceId) {
      throw ArgumentError.value(
        deviceId.value,
        'deviceId',
        'A device can report only its own presence.',
      );
    }

    // An instant, never a state. Whether this counts as online is decided from this value by the
    // presence rule, at whichever device is asking. `merge` rather than `write` so a heartbeat
    // does not erase the display name.
    await _documents.merge(devicesCollection, deviceId.value, {
      'lastReportedAt': at.toUtc().toIso8601String(),
    });
  }

  @override
  Future<List<DevicePresenceRecord>> listDevices() async {
    final documents = await _documents.readAll(devicesCollection);
    return documents.map(documentToPresenceRecord).toList();
  }

  @override
  Future<void> sendEnvelope(DeviceId target, CommandEnvelope envelope) async {
    await _documents.add(commandsCollection, envelopeToDocument(target, envelope));
  }

  @override
  Future<List<CommandEnvelope>> receiveEnvelopes(DeviceId deviceId) async {
    final documents =
        await _documents.readWhere(commandsCollection, 'target', deviceId.value);
    return _consume(documents);
  }

  /// Delivers commands as they arrive, without polling.
  ///
  /// This is the single delivery path. [wakeUps] may reduce how long a sleeping device takes to
  /// consult it, but a device that never receives a push still receives its commands here — just
  /// later. That is the whole of design D4's degradation claim.
  ///
  /// **NOT EXERCISED against a real relay.** No test in this repository signs in to Firestore or
  /// opens a real channel. What is tested is the parsing and the delivery semantics, against a
  /// stub.
  Stream<CommandEnvelope> listen() {
    final controller = StreamController<CommandEnvelope>();

    final subscription = _documents
        .watchWhere(commandsCollection, 'target', deviceId.value)
        .listen(
      (documents) {
        try {
          for (final envelope in _consume(documents)) {
            controller.add(envelope);
          }
        } catch (error) {
          // A malformed document is reported rather than swallowed. Dropping it silently would
          // make a relay that corrupts one command indistinguishable from one that delivered
          // nothing, and the user would see a command vanish with no reason given.
          controller.addError(error);
        }
      },
      onError: controller.addError,
    );

    controller.onCancel = subscription.cancel;
    return controller.stream;
  }

  /// Emits whenever a push suggests there may be something to collect.
  ///
  /// Carries no command content, by construction: the type is `void`. A caller reacting to this
  /// re-reads through [receiveEnvelopes] or relies on [listen]; nothing here can shortcut to
  /// acting on a command, because there is no command in it.
  Stream<void> get wakeUps => _wakeUp.signals;

  /// Turns documents into envelopes, skipping ones already delivered this session.
  List<CommandEnvelope> _consume(List<RelayDocument> documents) {
    final envelopes = <CommandEnvelope>[];

    for (final document in documents) {
      if (document.id.isNotEmpty && _consumed.contains(document.id)) {
        continue;
      }

      // A malformed envelope throws rather than being skipped, for the reason in [listen].
      final envelope = documentToEnvelope(document.fields);

      if (document.id.isNotEmpty) {
        _consumed.add(document.id);
      }
      envelopes.add(envelope);
    }

    return envelopes;
  }
}

// ---------------------------------------------------------------------------
// Wire types, hand-written (design D8).
// ---------------------------------------------------------------------------

/// Encodes one envelope as stored fields.
///
/// The signature travels as base64 — exactly the bytes given. Nothing here interprets it, and
/// nothing repairs it: a byte altered in transit must fail the signature check at the target
/// rather than be normalised into something that passes.
Map<String, Object?> envelopeToDocument(DeviceId target, CommandEnvelope envelope) => {
      'target': target.value,
      'sender': envelope.sender.value,
      'command': envelope.command.wireName,
      'createdAt': envelope.createdAt.toUtc().toIso8601String(),
      'nonce': envelope.nonce,
      'signature': base64Encode(envelope.signature),
    };

/// Reads stored fields back into an envelope.
///
/// **Every field is required.** A missing one throws, never defaults — design D8's whole
/// argument. A defaulted `createdAt` would make a stale command look fresh, a defaulted nonce
/// would collide with every other defaulted nonce, and a defaulted empty signature would fail the
/// check for a reason the user could not act on.
CommandEnvelope documentToEnvelope(Map<String, Object?> fields) {
  final sender = DeviceId.tryParse(_stringField(fields, 'sender'));
  if (sender == null) {
    throw StorageError(message: 'the relay sent a document whose sender is not a usable device id');
  }

  final commandName = _stringField(fields, 'command');
  final command = RemoteCommand.values
      .where((candidate) => candidate.wireName == commandName)
      .firstOrNull;
  if (command == null) {
    // An unknown command name is refused rather than mapped to something plausible. Guessing here
    // would let a relay steer a command towards a different action.
    throw StorageError(
      message: 'the relay sent an unrecognised command name "$commandName"',
    );
  }

  final createdAtRaw = _stringField(fields, 'createdAt');
  final createdAt = DateTime.tryParse(createdAtRaw);
  if (createdAt == null) {
    throw StorageError(message: 'the relay sent an unparseable createdAt "$createdAtRaw"');
  }

  final nonce = _stringField(fields, 'nonce');

  final signatureRaw = fields['signature'];
  final List<int> signature;
  if (signatureRaw is List<int>) {
    // Already bytes: `cloud_firestore` hands a stored `Blob` back this way.
    signature = signatureRaw;
  } else if (signatureRaw is String) {
    try {
      signature = base64Decode(signatureRaw);
    } on FormatException {
      // These bytes are relay-chosen in the threat model, so a bad encoding is an ordinary
      // refusal rather than a crash.
      throw StorageError(message: 'the relay sent a signature that is not valid base64');
    }
  } else {
    throw _missing('signature');
  }

  return CommandEnvelope(
    sender: sender,
    command: command,
    createdAt: createdAt.toUtc(),
    nonce: nonce,
    signature: signature,
  );
}

/// Reads one stored device document into a presence record.
///
/// `lastReportedAt` is the one field here that may legitimately be absent: a device that has
/// registered but never reported has no instant, and `null` says exactly that. Substituting a
/// distant past instant would make a brand-new device look stale, and substituting `now` would
/// make one that has never spoken look present.
DevicePresenceRecord documentToPresenceRecord(RelayDocument document) {
  final fields = document.fields;

  final deviceId = DeviceId.tryParse(_stringField(fields, 'deviceId'));
  if (deviceId == null) {
    throw StorageError(message: 'the relay sent a device document with an unusable device id');
  }

  final displayName = _stringField(fields, 'displayName');

  DateTime? lastReportedAt;
  final raw = fields['lastReportedAt'];
  if (raw != null) {
    if (raw is DateTime) {
      lastReportedAt = raw.toUtc();
    } else if (raw is String) {
      final parsed = DateTime.tryParse(raw);
      if (parsed == null) {
        throw StorageError(message: 'the relay sent an unparseable lastReportedAt "$raw"');
      }
      lastReportedAt = parsed.toUtc();
    } else {
      throw StorageError(message: 'the relay sent a lastReportedAt that is neither a string nor an instant');
    }
  }

  return DevicePresenceRecord(
    deviceId: deviceId,
    displayName: displayName,
    lastReportedAt: lastReportedAt,
  );
}

/// Reads a required string field.
String _stringField(Map<String, Object?> fields, String name) {
  final value = fields[name];
  if (value is! String) {
    throw _missing(name);
  }
  return value;
}

/// The error for a field the relay did not send.
///
/// Named rather than inline so every absence reads the same way in a log, and so the "missing is
/// an error" rule is one function rather than a habit.
StorageError _missing(String field) => StorageError(
      message: 'the relay sent a document with no $field; every field this code depends on '
          'is required, because a defaulted one would be indistinguishable from a real value',
    );
