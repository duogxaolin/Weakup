/// The seam a relay sits behind, and what a relay is required to be incapable of.
///
/// # A transport moves bytes and decides nothing
///
/// The interface below can register a device, report that it is present, list the account's
/// devices, send an envelope, and receive envelopes addressed here. That is the whole surface.
/// There is deliberately **no member by which an implementation could report that a command is
/// genuine** — no flag, no parameter, no method.
///
/// This is what makes a hosted relay an acceptable place to run this service rather than a
/// concentration of risk. The guarantee is not that the operator behaves well; it is that the
/// operator's cooperation is not sufficient. A relay can drop a command, delay it, duplicate it,
/// or deliver two out of order. It cannot cause one to be obeyed, because the target checks the
/// signature itself against a key the relay never holds.
///
/// A source-text test in `test/application/transport_source_test.dart` fails the build if a
/// member is ever added whose name suggests it carries such a claim. That test lives in its own
/// file rather than beside the implementation, because a file's own string literals would match
/// its own grep.
///
/// # Liveness is data, not a verdict
///
/// [RemoteTransport.reportPresence] records an instant and [RemoteTransport.listDevices] returns
/// instants. Neither returns "online". Whether a device counts as online is decided from that
/// instant by the presence rule, at whichever device is asking — otherwise two devices could ask
/// the same relay about the same peer and be told different things, and a network-dependent
/// judgement would appear to the user as a fact.
///
/// # Why account identity is a separate interface
///
/// [AuthProvider] is not part of [RemoteTransport]. Account identity decides which devices a user
/// can *see*; pairing decides which they can *command*, and the authorization rules already state
/// that same-account access is not sufficient. One interface carrying both concerns invites an
/// implementation in which a valid session becomes authority — exactly what those rules forbid —
/// and makes it awkward to replace either half alone.
///
/// Mirrors `desktop/src-tauri/src/application/transport.rs`.
library;

import '../domain/command_envelope.dart';
import '../domain/device_id.dart';

/// What a transport reports about one device in the account.
///
/// Carries an instant rather than a state. See the library comment: the online-or-not judgement
/// belongs to the presence rule at the asking device.
final class DevicePresenceRecord {
  const DevicePresenceRecord({
    required this.deviceId,
    required this.displayName,
    required this.lastReportedAt,
  });

  /// Which device this is.
  final DeviceId deviceId;

  /// A label the person recognises, for display only. It confers nothing.
  final String displayName;

  /// When this device last reported that it was present, or `null` if it never has.
  ///
  /// `null` rather than a distant past instant: "never spoke" and "spoke long ago" are
  /// different facts, and flattening them would make a brand-new device look stale.
  final DateTime? lastReportedAt;

  DevicePresenceRecord copyWith({String? displayName, DateTime? lastReportedAt}) =>
      DevicePresenceRecord(
        deviceId: deviceId,
        displayName: displayName ?? this.displayName,
        lastReportedAt: lastReportedAt ?? this.lastReportedAt,
      );

  @override
  String toString() => 'DevicePresenceRecord($deviceId, $displayName, $lastReportedAt)';
}

/// The account a device belongs to.
///
/// Deliberately thin. It answers "whose devices should this one be shown", and nothing about what
/// may be commanded.
final class AccountId {
  const AccountId(this.value);

  final String value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is AccountId && other.value == value);

  @override
  int get hashCode => value.hashCode;

  @override
  String toString() => value;
}

/// Carries opaque envelopes between devices in one account, and reports liveness.
///
/// Every method moves data or reports an instant. None of them makes, or reports, a judgement
/// about whether a command should be obeyed — see the library comment for why that absence is
/// the design rather than an omission.
abstract interface class RemoteTransport {
  /// Makes this device known to the account, so other devices can list it.
  ///
  /// Being listed is not permission to command. Pairing is a separate act.
  Future<void> registerDevice(DeviceId deviceId, String displayName);

  /// Records that this device is present, as of [at].
  Future<void> reportPresence(DeviceId deviceId, DateTime at);

  /// The account's devices and when each last reported presence.
  Future<List<DevicePresenceRecord>> listDevices();

  /// Hands a signed envelope to the transport for delivery to [target].
  ///
  /// The transport does not inspect the envelope and has no way to indicate an opinion about it.
  /// Success means the transport accepted it for delivery, not that the target obeyed it — and
  /// certainly not that it should.
  Future<void> sendEnvelope(DeviceId target, CommandEnvelope envelope);

  /// Takes the envelopes waiting for [deviceId], emptying the queue.
  ///
  /// The order is whatever the transport gives, and may repeat an envelope already delivered.
  /// The receiving device judges each one on its own merits, which is what makes those faults
  /// survivable.
  Future<List<CommandEnvelope>> receiveEnvelopes(DeviceId deviceId);
}

/// Establishes which account this device belongs to.
///
/// Separate from [RemoteTransport] by design (see the library comment): neither may substitute
/// for the other, and replacing one must not require touching the other.
abstract interface class AuthProvider {
  /// The account this device is currently acting as, or `null` when signed out.
  Future<AccountId?> currentAccount();
}

/// The faults a transport is assumed capable of, each switched on per test.
///
/// A fake that only behaves proves the happy path and nothing else. The spec requires the rules
/// survive a transport that misbehaves, and these are the four faults the design depends on
/// surviving: a dropped command is an ordinary mobile network, a delayed one is a backgrounded
/// app, a duplicate is a retry after an ambiguous timeout, and a reorder is two commands racing
/// through different paths.
///
/// Default is well-behaved, so a test that does not opt in reads plainly.
final class TransportFaults {
  const TransportFaults({
    this.dropEverything = false,
    this.delayDelivery = false,
    this.duplicateDelivery = false,
    this.reorderDelivery = false,
  });

  /// Accept envelopes and deliver none of them.
  final bool dropEverything;

  /// Hold envelopes until [FakeTransport.releaseDelayed] is called.
  final bool delayDelivery;

  /// Deliver every envelope twice.
  final bool duplicateDelivery;

  /// Deliver in the reverse of the order sent.
  final bool reorderDelivery;
}

/// An in-memory [RemoteTransport] that can misbehave on demand.
///
/// This is the honest expression of a property the design claims: a relay may drop, delay,
/// duplicate, and reorder, but cannot forge. The fake does all four; what it has no way to do —
/// because the interface offers no such member — is make a command more likely to be obeyed.
final class FakeTransport implements RemoteTransport {
  FakeTransport({this.faults = const TransportFaults()});

  final TransportFaults faults;

  final List<DevicePresenceRecord> _devices = [];
  final Map<DeviceId, List<CommandEnvelope>> _queued = {};
  final Map<DeviceId, List<CommandEnvelope>> _held = {};

  /// Releases everything held back by [TransportFaults.delayDelivery], as a late delivery would.
  ///
  /// The command's own creation instant is unchanged by the wait, which is exactly how a real
  /// delayed delivery behaves and why the freshness rule catches it.
  void releaseDelayed() {
    for (final entry in _held.entries) {
      _queued.putIfAbsent(entry.key, () => []).addAll(entry.value);
    }
    _held.clear();
  }

  /// How many envelopes are waiting for [deviceId], for a test asserting a drop.
  int queuedCount(DeviceId deviceId) => _queued[deviceId]?.length ?? 0;

  @override
  Future<void> registerDevice(DeviceId deviceId, String displayName) async {
    final index = _devices.indexWhere((record) => record.deviceId == deviceId);
    if (index >= 0) {
      _devices[index] = _devices[index].copyWith(displayName: displayName);
    } else {
      _devices.add(DevicePresenceRecord(
        deviceId: deviceId,
        displayName: displayName,
        lastReportedAt: null,
      ));
    }
  }

  @override
  Future<void> reportPresence(DeviceId deviceId, DateTime at) async {
    final index = _devices.indexWhere((record) => record.deviceId == deviceId);
    if (index >= 0) {
      _devices[index] = _devices[index].copyWith(lastReportedAt: at);
    } else {
      _devices.add(DevicePresenceRecord(
        deviceId: deviceId,
        displayName: deviceId.value,
        lastReportedAt: at,
      ));
    }
  }

  @override
  Future<List<DevicePresenceRecord>> listDevices() async =>
      List<DevicePresenceRecord>.unmodifiable(_devices);

  @override
  Future<void> sendEnvelope(DeviceId target, CommandEnvelope envelope) async {
    if (faults.dropEverything) {
      // Accepted and discarded, exactly as a relay losing a message would behave. The sender is
      // told nothing is wrong, which is the point: it cannot tell.
      return;
    }

    final destination = faults.delayDelivery
        ? _held.putIfAbsent(target, () => [])
        : _queued.putIfAbsent(target, () => []);

    destination.add(envelope);
    if (faults.duplicateDelivery) {
      destination.add(envelope);
    }
  }

  @override
  Future<List<CommandEnvelope>> receiveEnvelopes(DeviceId deviceId) async {
    final envelopes = _queued.remove(deviceId) ?? [];
    if (faults.reorderDelivery) {
      return envelopes.reversed.toList();
    }
    return envelopes;
  }
}

/// An [AuthProvider] that returns a fixed account.
///
/// There is no OAuth and no real identity behind this. It exists so that the seam is exercised
/// and so the transport can be written against an interface rather than against a sign-in
/// library.
final class FakeAuthProvider implements AuthProvider {
  const FakeAuthProvider(AccountId account) : _account = account;

  /// A provider reporting no account, as a signed-out device would.
  const FakeAuthProvider.signedOut() : _account = null;

  final AccountId? _account;

  @override
  Future<AccountId?> currentAccount() async => _account;
}
