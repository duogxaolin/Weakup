/// What a command looks like when it arrives from another device.
///
/// Mirrors `desktop/src-tauri/src/domain/command_envelope.rs`, carrying the same field set
/// and the same documented omission.
library;

import 'device_id.dart';
import 'remote_command.dart';

/// A command as received, with everything the acceptance rule needs to judge it.
///
/// # What this class deliberately does not carry
///
/// There is **no `relayAttested`, no `serverVerified`, and no `trustedSource`** — no field of
/// any kind that an intermediary could populate to influence the decision. That omission is
/// the design, not an oversight.
///
/// The architecture this honours is that the *sending device* signs and the relay only
/// forwards. A relay must be able to drop or delay a command and must never be able to invent
/// one, because that is what keeps a compromised relay from being a shutdown for every user of
/// the service. A class with a field a relay could set would satisfy that requirement in prose
/// while contradicting it in shape — and a field that exists eventually gets read. Omitting it
/// makes the requirement true by construction rather than by review.
///
/// # Why this carries a signature and not a verdict
///
/// The field here is [signature] — the actual bytes — and there is deliberately **no
/// `signatureVerified` boolean**. An earlier version of this class had one, and its own comment
/// recorded the problem: nothing prevented a caller passing `true` unconditionally, so the
/// authenticity guarantee was specified rather than enforced.
///
/// Replacing the boolean rather than adding alongside it is the entire point. If both existed,
/// every call site would face a choice between doing the work and asserting the answer, and some
/// call site would eventually assert — a transport, most likely, on the day it was written
/// against a rule that accepted either. Deleting the field makes the shortcut unrepresentable.
///
/// A caller can now only present evidence. `evaluateCommand` encodes the signed content
/// canonically, looks up the verifying key the target holds for the claimed sender, and checks
/// the arithmetic itself.
///
/// What this does *not* yet establish is where the signing key lives. A verified signature proves
/// the command came from whoever holds that key; nothing here guarantees it is held only by the
/// paired devices, because key storage belongs to the change that has a device to run on.
final class CommandEnvelope {
  const CommandEnvelope({
    required this.sender,
    required this.command,
    required this.createdAt,
    required this.nonce,
    required this.signature,
  });

  /// The device that produced this command.
  final DeviceId sender;

  /// What is being asked for.
  final RemoteCommand command;

  /// The instant the *sending* device says it created this command.
  ///
  /// A claim rather than a fact — the target cannot verify another device's clock — which is
  /// why it is bounded on both sides: too old is stale, too far ahead is future-dated.
  ///
  /// Covered by the signature, so a relay cannot rewrite it to revive a stale command.
  final DateTime createdAt;

  /// What distinguishes this command from every other one from the same device.
  ///
  /// A signature alone does not prevent replay: a captured command stays valid, and resent
  /// immediately it is still fresh. Freshness and this value are both required and neither
  /// substitutes for the other.
  ///
  /// Covered by the signature, so a relay cannot change it to make a replay look new.
  final String nonce;

  /// The sending device's Ed25519 signature over the canonical encoding of this command.
  ///
  /// Evidence, not a verdict. These bytes are attacker-controlled in the threat model — a relay
  /// can put anything here — which is why verification returns `false` for malformed input
  /// rather than throwing.
  ///
  /// The bytes signed are produced by `encodeSigningPayload` and cover the sender, the command,
  /// the creation instant, and the nonce. A field outside that encoding is a field an
  /// intermediary could change undetected.
  final List<int> signature;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CommandEnvelope &&
          other.sender == sender &&
          other.command == command &&
          other.createdAt == createdAt &&
          other.nonce == nonce &&
          _sameBytes(other.signature, signature));

  @override
  int get hashCode =>
      Object.hash(sender, command, createdAt, nonce, Object.hashAll(signature));

  @override
  String toString() => 'CommandEnvelope(${command.wireName} from $sender, '
      'createdAt=$createdAt, nonce=$nonce, '
      'signature=${signature.length} bytes)';
}

bool _sameBytes(List<int> a, List<int> b) {
  if (identical(a, b)) return true;
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}
