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
/// # Why [signatureVerified] is a bool and not a signature
///
/// The rules take a verification *outcome* the caller computed, not a signature they check.
/// Verifying one needs a key, a key store, an algorithm, and a platform; none of those can be
/// pure, none can be identical across Rust and Dart, and vectors carrying real key material
/// would test each language's crypto library rather than the shared decision.
///
/// The field is named for what it asserts rather than for what a caller wants, so a caller
/// hardcoding it reads as obviously wrong at the call site. That is a mitigation and not a
/// guarantee: **nothing here prevents a caller passing `true` unconditionally**, and no test
/// can catch it, because the caller does not exist yet. The change that introduces the
/// transport must verify signatures against a key held only by the paired devices and must
/// carry an end-to-end test that an invalid signature is rejected. Until then the authenticity
/// guarantee is specified, not enforced.
final class CommandEnvelope {
  const CommandEnvelope({
    required this.sender,
    required this.command,
    required this.createdAt,
    required this.nonce,
    required this.signatureVerified,
  });

  /// The device that produced this command.
  final DeviceId sender;

  /// What is being asked for.
  final RemoteCommand command;

  /// The instant the *sending* device says it created this command.
  ///
  /// A claim rather than a fact — the target cannot verify another device's clock — which is
  /// why it is bounded on both sides: too old is stale, too far ahead is future-dated.
  final DateTime createdAt;

  /// What distinguishes this command from every other one from the same device.
  ///
  /// A signature alone does not prevent replay: a captured command stays valid, and resent
  /// immediately it is still fresh. Freshness and this value are both required and neither
  /// substitutes for the other.
  final String nonce;

  /// Whether the caller verified this command's signature against a pairing the target holds.
  /// See the class comment — this is an input, not something these rules compute.
  final bool signatureVerified;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CommandEnvelope &&
          other.sender == sender &&
          other.command == command &&
          other.createdAt == createdAt &&
          other.nonce == nonce &&
          other.signatureVerified == signatureVerified);

  @override
  int get hashCode =>
      Object.hash(sender, command, createdAt, nonce, signatureVerified);

  @override
  String toString() => 'CommandEnvelope(${command.wireName} from $sender, '
      'createdAt=$createdAt, nonce=$nonce, '
      'signatureVerified=$signatureVerified)';
}
