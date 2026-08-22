/// The pairing exchange: presenting a code at one device and redeeming it at the other.
///
/// Pairing is the only act that confers authority to command a device. The rules that decide
/// whether a grant may still be redeemed already exist in [evaluateGrant] and are pinned by
/// shared vectors. This library adds the two things that were missing around them: a code a
/// person can read off one screen and type into another, and the exchange that turns a redeemed
/// code into a pairing recorded at both ends.
///
/// # What this library does *not* decide
///
/// Expiry and single use are not reimplemented here. [PairingCodeIssuer.redeem] builds a
/// [PairingGrant] from what it knows — whether it issued this code, whether it has already been
/// redeemed, when it was issued — and hands the decision to [evaluateGrant]. A second
/// implementation of those rules would be a second source of truth, and the one that drifted
/// would be whichever was not vector-verified.
///
/// # Why the alphabet excludes `0`/`O` and `1`/`I`/`l`
///
/// Not cosmetic. A user who mistypes a code gets [GrantRejection.noSuchGrant], which reads to
/// them as indistinguishable from an expired code — so they retry, and keep retrying, until the
/// grant really has expired. Removing the glyphs that are routinely confused removes that
/// failure rather than documenting it. See [pairingCodeAlphabet].
///
/// Six characters from a 31-glyph alphabet is about 29.7 bits (31^6 ≈ 8.9e8). That is weak in
/// isolation and is
/// adequate here only because of three properties that hold together:
///
/// - the grant expires — 300 seconds when shown at the machine;
/// - it can be redeemed once, after which the code is spent;
/// - it is accepted only by the device that issued it, which holds it in memory and never sends
///   it anywhere.
///
/// Weakening any one of those three makes the code length wrong, and the right response would be
/// to lengthen the code rather than to accept the loss quietly.
///
/// # Both sides or neither
///
/// [completePairingAtRequester] records the issuer **only after** the issuer has confirmed it
/// recorded the requester, and if its own write then fails it withdraws the issuer's record too.
/// A half-recorded pairing is the worst outcome available: the requesting device believes it is
/// authorized, sends commands, and every one is refused for a reason the user cannot see
/// anywhere in either interface. An outright failure is strictly better, because it tells the
/// user to try again.
///
/// The withdrawal is a revocation rather than a deletion, because that is what the store offers
/// and the reason is the same one the store's own comment gives: the record that an exchange was
/// attempted is worth keeping, and a revoked pairing confers nothing — its key is absent from
/// the map `evaluateCommand` checks against.
///
/// Mirrors `desktop/src-tauri/src/application/pairing_flow.rs`.
library;

// `prefer_initializing_formals` is suppressed here, and the reason is not style. Taking its
// advice means writing `PairingCodeIssuer({Random? this._random})`, which needs the
// `private-named-parameters` language feature and a minimum SDK of 3.12. This package targets
// lower, and the analyzer rejects the suggested form outright with `experiment_not_enabled`.
// Even once available, a private named parameter cannot be supplied from another library, so
// the constructor would become uncallable from the tests that inject a seeded generator. Two
// other files in this package carry this same suppression for the same verified reason.
// ignore_for_file: prefer_initializing_formals

import 'dart:math';

import '../core/app_error.dart';
import '../core/result.dart';
import '../domain/device_id.dart';
import '../domain/pairing_grant.dart';
import '../domain/signature.dart';

// ---------------------------------------------------------------------------
// The code itself
// ---------------------------------------------------------------------------

/// The glyphs a pairing code may contain.
///
/// The 36 alphanumerics of the Latin alphabet less the five that are routinely misread:
/// `0`/`O` and `1`/`I`/`L`. Uppercase only, so a code read off a screen and typed in any case
/// normalises to exactly one string.
const String pairingCodeAlphabet = '23456789ABCDEFGHJKMNPQRSTUVWXYZ';

/// How many characters a pairing code has. See the library comment for why six is enough only
/// alongside expiry, single use, and issuer-only acceptance.
const int pairingCodeLength = 6;

/// The confusable glyphs that must never appear in a code, named so the reason survives.
///
/// `l` is lowercase deliberately: the alphabet is uppercase, so the glyph a user confuses with
/// `1` is the lowercase `l` they might type. Its uppercase form `L` is what is excluded from the
/// alphabet, and both are checked.
const List<String> confusableGlyphs = ['0', 'O', '1', 'I', 'l', 'L'];

/// A pairing code, as shown at the issuing device and typed at the other.
///
/// A wrapper rather than a bare [String] so that a code and a device id cannot be passed in each
/// other's place, and so normalisation happens in exactly one place.
final class PairingCode {
  const PairingCode._(this.value);

  /// Accepts a code a person typed, normalising case and ignoring spacing.
  ///
  /// Normalisation is deliberately generous about everything that does not change which code was
  /// meant — case, surrounding whitespace, and the internal spaces or hyphens a user adds when
  /// transcribing in groups. It is deliberately strict about glyphs outside the alphabet: those
  /// are refused here rather than being looked up and reported as unrecognised, so a genuinely
  /// mistyped character does not consume the attempt.
  ///
  /// Confusable glyphs are **not** silently corrected. Mapping `O` to `0` would be guessing at
  /// intent, and a code that is accepted as something other than what was typed is harder to
  /// reason about than one that is refused.
  static Result<PairingCode> parse(String entered) {
    final normalised = entered
        .replaceAll(RegExp(r'[\s-]'), '')
        .toUpperCase();

    if (normalised.length != pairingCodeLength) {
      return Result.failure(
        const ValidationError(
          message: 'A pairing code is $pairingCodeLength characters long.',
        ),
      );
    }

    for (final glyph in normalised.split('')) {
      if (!pairingCodeAlphabet.contains(glyph)) {
        // Named rather than generic: "that is not a character a code contains" is actionable,
        // and it is a different situation from a code that was not issued.
        return Result.failure(
          ValidationError(
            message: "A pairing code does not contain '$glyph'. Check the code shown on "
                'the other device and try again.',
          ),
        );
      }
    }

    return Result.success(PairingCode._(normalised));
  }

  /// The code as it should be displayed and compared.
  final String value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is PairingCode && other.value == value);

  @override
  int get hashCode => value.hashCode;

  @override
  String toString() => value;
}

/// Draws a fresh code from the platform's cryptographic random source.
///
/// [Random.secure] rather than [Random], and that is not interchangeable: the default generator
/// is seeded predictably enough that an observer who learns one code can narrow the next. A code
/// drawn from a weak source is one an attacker can reproduce, and the grant it guards is what
/// authorises commanding this machine.
///
/// `nextInt` over the alphabet length is uniform by contract here, so no rejection sampling is
/// needed — unlike the Rust side, which draws raw bytes and must discard the ones that would
/// bias the fold.
PairingCode generatePairingCode({Random? random}) {
  final rng = random ?? Random.secure();
  final buffer = StringBuffer();
  for (var i = 0; i < pairingCodeLength; i++) {
    buffer.write(pairingCodeAlphabet[rng.nextInt(pairingCodeAlphabet.length)]);
  }
  return PairingCode._(buffer.toString());
}

// ---------------------------------------------------------------------------
// The store seam
// ---------------------------------------------------------------------------

/// The two store operations the exchange needs.
///
/// A narrow interface rather than the concrete `PairingStore`, so the flow can be exercised
/// against a store whose write fails — which is the only way to test the both-or-neither
/// guarantee at all. The real store satisfies it through [DriftPairingRecorder].
///
/// Deliberately two members. A flow that could read the whole pairing list, or revoke anything
/// it liked, would be a broader capability than recording the peer it just paired with.
abstract interface class PairingRecorder {
  /// Records a pairing with [peer], or re-establishes a revoked one.
  Future<Result<void>> recordPairing({
    required DeviceId peer,
    required VerifyingKey verifyingKey,
    required DateTime pairedAt,
  });

  /// Withdraws [peer]'s authority, used to undo a half-completed exchange.
  Future<Result<void>> revokePairing({
    required DeviceId peer,
    required DateTime revokedAt,
  });
}

// ---------------------------------------------------------------------------
// The issuing side
// ---------------------------------------------------------------------------

/// What this device tells a peer about itself when pairing.
///
/// Public material only: an identifier and the verifying key peers record. There is no private
/// half here and no route to one — the identity that signs holds its key and never hands it over.
final class PairingIdentity {
  const PairingIdentity({required this.deviceId, required this.verifyingKey});

  final DeviceId deviceId;
  final VerifyingKey verifyingKey;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is PairingIdentity &&
          other.deviceId == deviceId &&
          other.verifyingKey == verifyingKey);

  @override
  int get hashCode => Object.hash(deviceId, verifyingKey);

  @override
  String toString() => 'PairingIdentity($deviceId)';
}

/// One outstanding code and the grant state [evaluateGrant] needs to judge it.
final class _OutstandingGrant {
  _OutstandingGrant({
    required this.delivery,
    required this.issuedAt,
    required this.redeemed,
  });

  final GrantDelivery delivery;
  final DateTime issuedAt;
  bool redeemed;
}

/// Holds the codes this device has issued and not yet seen redeemed.
///
/// In memory rather than in the database, and that is the decision. A code is meaningful only
/// while the person is standing at the machine, its lifetime is 300 seconds, and a restart in the
/// middle of pairing is a restart the user will notice and repeat. Persisting it would keep a
/// redeemable secret on disk to no benefit.
///
/// It is also what makes "accepted only by the device that issued it" true rather than
/// aspirational: the code is never sent to a relay, so no other device can be asked about it.
final class PairingCodeIssuer {
  PairingCodeIssuer({Random? random}) : _random = random;

  final Random? _random;
  final Map<PairingCode, _OutstandingGrant> _outstanding = {};

  /// Issues a code to show the person, valid for its delivery path's lifetime from [now].
  ///
  /// The lifetime is not stored: it is derived from [delivery] by the domain rule, so this side
  /// has no opportunity to disagree with the rule about when a code dies.
  PairingCode issue({required GrantDelivery delivery, required DateTime now}) {
    final code = generatePairingCode(random: _random);
    _outstanding[code] = _OutstandingGrant(
      delivery: delivery,
      issuedAt: now,
      redeemed: false,
    );
    return code;
  }

  /// Judges a code presented back, and marks it spent if it was good.
  ///
  /// The verdict comes from [evaluateGrant]; this method's job is to describe the grant honestly
  /// — including `recognised: false` for a code it never issued, which is what keeps a mistyped
  /// code distinguishable from an expired one.
  ///
  /// Marking spent happens **here**, at the moment the code is accepted, rather than after the
  /// pairing is recorded. A code that could be redeemed twice while a first exchange was still in
  /// flight would not be single use, and single use is what makes an observed code worthless
  /// afterwards.
  GrantValidity redeem({required PairingCode code, required DateTime now}) {
    final state = _outstanding[code];

    if (state == null) {
      // Never issued, or issued by some other device. Either way this device does not stand
      // behind any `issuedAt` for it, so no age is computed — which is exactly the ordering
      // `evaluateGrant` pins.
      return evaluateGrant(
        grant: PairingGrant(
          delivery: GrantDelivery.atMachine,
          issuedAt: now,
          redeemed: false,
          recognised: false,
        ),
        now: now,
      );
    }

    final validity = evaluateGrant(
      grant: PairingGrant(
        delivery: state.delivery,
        issuedAt: state.issuedAt,
        redeemed: state.redeemed,
        recognised: true,
      ),
      now: now,
    );

    if (validity.isValid) {
      state.redeemed = true;
    }

    return validity;
  }

  /// Forgets an expired or spent code, so the map does not grow without bound.
  ///
  /// Retaining a redeemed code until it is swept is deliberate: dropping it the instant it was
  /// used would make a second presentation report [GrantRejection.noSuchGrant] rather than
  /// [GrantRejection.alreadyUsed], and those tell the person two different things. One means
  /// check the code; the other means the code is spent and a new one is needed.
  void forgetStale(DateTime now) {
    _outstanding.removeWhere((_, state) {
      final age = now.difference(state.issuedAt).inSeconds;
      // Kept for a second lifetime past expiry so alreadyUsed and expired remain reportable for
      // a while after the fact.
      return age > state.delivery.lifetimeSeconds * 2;
    });
  }

  /// How many codes are outstanding. For tests and for a surface that shows the user whether a
  /// code is currently live.
  int get outstandingCount => _outstanding.length;
}

// ---------------------------------------------------------------------------
// The exchange
// ---------------------------------------------------------------------------

/// What happened to a pairing attempt.
///
/// A refusal names the grant rejection it came from, so the surface can show the person the prose
/// the domain already wrote rather than inventing its own.
sealed class PairingOutcome {
  const PairingOutcome();

  bool get isPaired => this is PairedOutcome;

  /// The prose to show the person, taken from the domain's own messages for a refusal.
  String get userMessage;
}

/// Both devices recorded each other.
final class PairedOutcome extends PairingOutcome {
  const PairedOutcome(this.peer);

  /// The peer now recorded here.
  final DeviceId peer;

  @override
  String get userMessage => 'Paired with $peer.';

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is PairedOutcome && other.peer == peer);

  @override
  int get hashCode => peer.hashCode;

  @override
  String toString() => 'Paired($peer)';
}

/// The code was refused. Nothing was recorded at either device.
final class RefusedOutcome extends PairingOutcome {
  const RefusedOutcome(this.reason);

  final GrantRejection reason;

  @override
  String get userMessage => reason.userMessage;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is RefusedOutcome && other.reason == reason);

  @override
  int get hashCode => reason.hashCode;

  @override
  String toString() => 'Refused(${reason.wireName})';
}

/// What the issuing device sends back.
///
/// [AcceptedResponse] carrying the issuer's identity is the confirmation both-or-neither
/// requires: it exists only if the issuer's own record was written, so the requester recording on
/// receipt of it cannot produce a one-sided pairing in the issuer-failed direction.
sealed class PairingResponse {
  const PairingResponse();
}

/// The issuer redeemed the code and recorded the requester.
final class AcceptedResponse extends PairingResponse {
  const AcceptedResponse(this.issuerIdentity);

  final PairingIdentity issuerIdentity;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is AcceptedResponse && other.issuerIdentity == issuerIdentity);

  @override
  int get hashCode => issuerIdentity.hashCode;

  @override
  String toString() => 'Accepted($issuerIdentity)';
}

/// The issuer refused the code and recorded nothing.
final class RefusedResponse extends PairingResponse {
  const RefusedResponse(this.reason);

  final GrantRejection reason;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is RefusedResponse && other.reason == reason);

  @override
  int get hashCode => reason.hashCode;

  @override
  String toString() => 'Refused(${reason.wireName})';
}

/// The issuing device's half: judge the code, and record the requester if it was good.
///
/// Returns this device's own public identity on success, which is what the requester needs in
/// order to record the other direction — and, crucially, is also the *confirmation* that this
/// side has recorded the requester. There is no way to obtain the identity without the record
/// having been written, so the requester cannot record a peer that has not reciprocated.
///
/// If the store write fails, this returns the failure and records nothing. The requester never
/// receives an identity and so never records anything either.
Future<Result<PairingResponse>> acceptPairingAtIssuer({
  required PairingCodeIssuer issuer,
  required PairingRecorder store,
  required PairingIdentity ownIdentity,
  required PairingIdentity requester,
  required PairingCode code,
  required DateTime now,
}) async {
  final validity = issuer.redeem(code: code, now: now);

  if (validity case InvalidGrant(:final reason)) {
    return Result.success(RefusedResponse(reason));
  }

  // The requester's key is recorded from what the requester presented in person, during an
  // exchange the owner started at this machine. It is never taken from a relay and never from a
  // key travelling alongside a command.
  final recorded = await store.recordPairing(
    peer: requester.deviceId,
    verifyingKey: requester.verifyingKey,
    pairedAt: now,
  );

  if (recorded.errorOrNull case final error?) {
    return Result.failure(error);
  }

  return Result.success(AcceptedResponse(ownIdentity));
}

/// The requesting device's half: present the code, then record the issuer only once the issuer
/// has confirmed it recorded us.
///
/// # The failure this function exists to prevent
///
/// The dangerous ordering is the tempting one — record the peer optimistically, then tell it
/// about us — because it leaves this device believing it is authorized while the peer has no
/// record of it. Every command it then sends is refused as coming from an unknown sender, and
/// there is nowhere in either interface where the user could see why.
///
/// So the order here is fixed: the issuer records first and says so, and only then does this side
/// write. And if this side's write fails after the issuer's succeeded, the issuer's record is
/// **withdrawn** by [undo] before the failure is returned, because the other one-sided state is
/// just as bad in the opposite direction.
///
/// [undo] is a callback rather than a direct store call because the issuer's store is not
/// reachable from here in the real system — it is on the other device, behind whatever channel
/// carried the exchange. Making the caller supply it keeps this function honest about the fact
/// that the compensating step is a real round trip that can itself fail.
Future<Result<PairingOutcome>> completePairingAtRequester({
  required PairingRecorder store,
  required PairingResponse response,
  required DateTime now,
  required Future<Result<void>> Function() undo,
}) async {
  if (response case RefusedResponse(:final reason)) {
    // Nothing was recorded at the issuer, so there is nothing to undo and nothing to write here.
    // Both sides hold no pairing, which is the correct both-or-neither outcome for a refusal.
    return Result.success(RefusedOutcome(reason));
  }

  final issuerIdentity = (response as AcceptedResponse).issuerIdentity;

  final recorded = await store.recordPairing(
    peer: issuerIdentity.deviceId,
    verifyingKey: issuerIdentity.verifyingKey,
    pairedAt: now,
  );

  if (recorded.errorOrNull case final localFailure?) {
    // The issuer has recorded us and we cannot record it. Withdraw its record rather than leaving
    // it believing a pairing exists that this device knows nothing about.
    final undone = await undo();

    if (undone.errorOrNull case final undoFailure?) {
      // Both the write and its compensation failed. This is the one state that cannot be
      // repaired from here, so it is reported in full rather than flattened into either failure
      // alone — the user needs to know a stale record may exist at the other device and can be
      // revoked there.
      return Result.failure(
        StorageError(
          message: 'pairing could not be recorded on this device '
              '($localFailure), and withdrawing the record the other device had '
              'already made also failed ($undoFailure). The other device may '
              'still list this one as paired; revoke it there.',
        ),
      );
    }

    return Result.failure(localFailure);
  }

  return Result.success(PairedOutcome(issuerIdentity.deviceId));
}

/// Withdraws a pairing recorded during an exchange that then failed.
///
/// A revocation rather than a deletion, for the reason the store's own comment gives: the store
/// never deletes, and a revoked pairing's key is absent from the map commands are checked
/// against, so the peer confers nothing. "Neither side believes it is paired" is satisfied by
/// absence of authority, which is what a revoked row is.
Future<Result<void>> withdrawPairing({
  required PairingRecorder store,
  required DeviceId peer,
  required DateTime at,
}) {
  return store.revokePairing(peer: peer, revokedAt: at);
}
