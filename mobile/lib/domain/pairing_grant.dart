/// Whether a grant offered to establish a pairing may still be redeemed.
///
/// Pairing is the only act that confers authority to command a device — not being signed in to
/// the same account, not being registered to the same owner. So this is the decision that gates
/// everything the acceptance rules later assume, and a grant that outlives its usefulness is a
/// grant someone else can use.
///
/// # Why the out-of-band lifetime is *longer* in wall-clock terms
///
/// This is the value most likely to be "corrected" by a later contributor who has read the spec
/// and not this comment, so the reasoning is recorded in full.
///
/// The spec says out-of-band evidence is weaker: possession of a mailbox proves less than
/// possession of the machine. That is true, and yet [outOfBandGrantLifetimeSeconds] is twice
/// [atMachineGrantLifetimeSeconds]. The resolution is that the two windows are not measuring the
/// same thing. A code shown on screen is typed within seconds; 300 seconds is already generous
/// and exists only to absorb a user who walks between rooms. A code sent to a mailbox must
/// survive mail delivery latency, which is under nobody's control and routinely exceeds a
/// minute. Setting out-of-band to 300 seconds would fail honest users often enough that they
/// would retry repeatedly, and a flow that habitually fails trains people to expect failure.
///
/// The weaker evidence is compensated where it actually matters: the grant is single-use, it is
/// delivered only to an address the owner has already proven control of and never to one
/// supplied in the request to pair, and it authorises *pairing* rather than a shutdown. An
/// intercepted grant still cannot power anything off without the person also completing a
/// pairing the owner can see and revoke.
///
/// Stated as a trade-off rather than hidden: if mail delivery turns out to be fast in practice,
/// the right change is to shorten the out-of-band lifetime. Both constants live here so that is
/// a two-line edit plus vector updates.
///
/// Mirrors `desktop/src-tauri/src/domain/pairing_grant.rs`, including the rejection messages
/// verbatim.
library;

/// Lifetime of a grant shown at the target machine. See the library comment.
const int atMachineGrantLifetimeSeconds = 300;

/// Lifetime of a grant delivered out of band. Longer in wall-clock terms, for the reason the
/// library comment sets out at length — it absorbs mail delivery latency, not weaker evidence.
const int outOfBandGrantLifetimeSeconds = 600;

/// How a grant reached the person redeeming it.
///
/// The delivery path is what selects the lifetime, so it is part of the grant rather than a
/// separate argument a caller could get wrong.
enum GrantDelivery {
  /// Shown at the target device itself. Possession of the machine is direct evidence of the
  /// authority being granted.
  atMachine,

  /// Delivered out of band to the account owner's established address.
  outOfBand,
}

extension GrantDeliveryX on GrantDelivery {
  /// How long a grant delivered this way stays redeemable.
  ///
  /// The single place the mapping lives, so the lifetime enforced and the lifetime documented
  /// cannot drift apart.
  int get lifetimeSeconds => switch (this) {
        GrantDelivery.atMachine => atMachineGrantLifetimeSeconds,
        GrantDelivery.outOfBand => outOfBandGrantLifetimeSeconds,
      };

  /// The name both implementations serialise.
  String get wireName => switch (this) {
        GrantDelivery.atMachine => 'atMachine',
        GrantDelivery.outOfBand => 'outOfBand',
      };
}

/// Why a presented grant was refused.
enum GrantRejection {
  /// The grant's lifetime has elapsed.
  expired,

  /// The grant has been redeemed before. Single use is what makes a grant observed in transit
  /// worthless after the fact.
  alreadyUsed,

  /// The target never issued this grant — a mistyped or invented code.
  noSuchGrant,
}

extension GrantRejectionX on GrantRejection {
  /// Prose for the person redeeming the grant.
  ///
  /// **Byte-identical to the strings in `desktop/src-tauri/src/domain/pairing_grant.rs`.**
  String get userMessage => switch (this) {
        GrantRejection.expired =>
          'That pairing code has expired. Start pairing again to get a new one.',
        GrantRejection.alreadyUsed =>
          'That pairing code has already been used. Each code works only once.',
        GrantRejection.noSuchGrant =>
          'That pairing code was not recognised. Check it and try again.',
      };

  /// The name both implementations serialise.
  String get wireName => switch (this) {
        GrantRejection.expired => 'expired',
        GrantRejection.alreadyUsed => 'alreadyUsed',
        GrantRejection.noSuchGrant => 'noSuchGrant',
      };
}

/// Valid, or refused with exactly one reason. The same shape, and the same argument, as
/// [CommandAcceptance].
sealed class GrantValidity {
  const GrantValidity();

  bool get isValid => this is ValidGrant;

  GrantRejection? get rejection =>
      this is InvalidGrant ? (this as InvalidGrant).reason : null;
}

/// The grant may be redeemed.
final class ValidGrant extends GrantValidity {
  const ValidGrant();

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is ValidGrant;

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() => 'Valid';
}

/// The grant is refused, for exactly one enumerated reason.
final class InvalidGrant extends GrantValidity {
  const InvalidGrant(this.reason);

  final GrantRejection reason;

  /// The prose shown to the person redeeming the grant.
  String get userMessage => reason.userMessage;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is InvalidGrant && other.reason == reason);

  @override
  int get hashCode => reason.hashCode;

  @override
  String toString() => 'Invalid(${reason.wireName})';
}

/// A grant as presented for redemption.
final class PairingGrant {
  const PairingGrant({
    required this.delivery,
    required this.issuedAt,
    required this.redeemed,
    required this.recognised,
  });

  /// How it reached the person redeeming it, which selects its lifetime.
  final GrantDelivery delivery;

  /// When the target issued it.
  final DateTime issuedAt;

  /// Whether it has been redeemed before.
  final bool redeemed;

  /// Whether the target recognises it as one it issued.
  final bool recognised;

  @override
  String toString() => 'PairingGrant(${delivery.wireName}, issuedAt=$issuedAt, '
      'redeemed=$redeemed, recognised=$recognised)';
}

/// Decides whether this grant may still be redeemed.
///
/// **The order of these checks is part of the contract**, and the shared vectors pin it:
/// recognition first, then alreadyUsed, then expired.
///
/// Recognition comes first because a target has no reason to trust the claimed fields of a grant
/// it never issued — computing an age from an attacker-supplied [PairingGrant.issuedAt] and
/// reporting "expired" would answer a question about data the target does not stand behind.
///
/// [GrantRejection.alreadyUsed] precedes [GrantRejection.expired] so that an already-redeemed
/// grant reports that fact whether or not it has also expired. The two indicate different
/// situations to the person holding the code: one means try again, the other means this code is
/// spent, and a user told the wrong one goes looking for a new code they already have.
GrantValidity evaluateGrant({
  required PairingGrant grant,
  required DateTime now,
}) {
  if (!grant.recognised) {
    return const InvalidGrant(GrantRejection.noSuchGrant);
  }

  if (grant.redeemed) {
    return const InvalidGrant(GrantRejection.alreadyUsed);
  }

  // Inclusive-accept: an age of exactly the lifetime is still valid, pinned from both sides in
  // the vectors so it cannot be decided by whichever operator was typed.
  final ageSeconds = now.difference(grant.issuedAt).inSeconds;
  if (ageSeconds > grant.delivery.lifetimeSeconds) {
    return const InvalidGrant(GrantRejection.expired);
  }

  return const ValidGrant();
}
