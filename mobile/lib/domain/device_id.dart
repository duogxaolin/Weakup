/// An opaque identifier for one device.
///
/// Deliberately structureless — see the class comment for why.
///
/// Mirrors `desktop/src-tauri/src/domain/device_id.rs`.
library;

/// The identity of a device, as an opaque string.
///
/// **No structure is imposed, and that is the decision rather than an omission.** The
/// authorization rule's own spec says that being signed in to the same account is not
/// sufficient to command a device, so account identity is not what any of these rules branch
/// on. Giving this type structure — an embedded account, a platform tag, a key fingerprint —
/// would bake a guess about the account model into rules that do not need one, which is the
/// same argument that keeps `PlatformCapabilities` out of [RemoteCommandContext].
///
/// The change that introduces a transport chooses what a device identifier actually looks
/// like. These rules only need to compare two of them and record one.
final class DeviceId {
  /// Throws [ArgumentError] when [value] is empty or only whitespace.
  ///
  /// Emptiness is the one property worth refusing without knowing the eventual format: an
  /// empty id compares equal to another empty id, so accepting it would let two unrelated
  /// devices be treated as the same one — and "the same one" is what pairing turns into
  /// authority to shut a machine down.
  DeviceId(this.value) {
    if (value.trim().isEmpty) {
      throw ArgumentError.value(value, 'value', 'A device id cannot be empty.');
    }
  }

  /// Null rather than a throw, for callers parsing untrusted input.
  static DeviceId? tryParse(String value) {
    if (value.trim().isEmpty) return null;
    return DeviceId(value);
  }

  final String value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is DeviceId && other.value == value);

  @override
  int get hashCode => value.hashCode;

  @override
  String toString() => value;
}
