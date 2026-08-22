/// The seam between the app and the platform's protected credential store.
///
/// Keystore on Android, Keychain on iOS, and the desktop backings
/// `flutter_secure_storage` provides. This library is the only place any of them is named, so
/// every decision that *depends* on the store can be tested against [InMemorySecretStore]
/// instead of against an OS facility that behaves differently on every target.
///
/// # Why "not found" and "unavailable" are different outcomes
///
/// This is the distinction the whole library exists to preserve, and conflating the two is
/// what would cause the worst failure this change can produce.
///
/// A device records in its database that it holds an identity. If the secret store then
/// cannot produce the private key, there are two possible reasons, and they call for opposite
/// responses:
///
/// - **Not found** — there is genuinely no entry. On a first run this is normal and the caller
///   generates one.
/// - **Unavailable** — the store exists but could not be reached: the keystore is locked, the
///   platform channel failed, the credential service is not running. The entry may be
///   perfectly intact behind that failure.
///
/// An implementation returning "not found" for both would make the second case look like a
/// first run. The caller would generate a replacement identity, and every pairing this device
/// holds would be silently destroyed — peers would keep trusting a key it no longer has, and
/// it would appear to all of them as a stranger. A locked keystore is recoverable; a
/// regenerated identity is not. So the store reports which happened and lets the caller refuse
/// to guess.
///
/// Mirrors `desktop/src-tauri/src/platform/secret_store.rs`.
library;

import 'dart:convert';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import '../core/app_error.dart';
import '../core/result.dart';

/// The outcome of looking a secret up: found with its bytes, or genuinely absent.
///
/// A sealed hierarchy rather than a nullable value, so a caller must distinguish "there is no
/// entry" from "the lookup failed" — the latter arrives as a [Result] failure instead. See
/// the library comment for why that distinction is load-bearing.
sealed class SecretLookup {
  const SecretLookup();
}

/// The entry exists.
final class SecretFound extends SecretLookup {
  const SecretFound(this.secret);

  final List<int> secret;
}

/// There is genuinely no entry under this name.
///
/// On a first run this is the expected answer, and the only case in which generating a new
/// secret is correct.
final class SecretNotFound extends SecretLookup {
  const SecretNotFound();
}

/// The platform's protected credential store, as the app sees it.
///
/// Deliberately small: get, set, delete over a namespaced name. Anything richer would be a
/// surface the fake has to reproduce faithfully, and the fake is what every other test in this
/// change depends on.
///
/// A *store unavailable* condition is a [Result] failure, while a genuinely absent entry is a
/// success carrying [SecretNotFound]. That split is the library's reason for existing.
abstract interface class SecretStore {
  /// The secret stored under [name], or [SecretNotFound] if there is none.
  ///
  /// Fails only when the store itself could not be consulted. A caller must not treat that as
  /// an absent entry — see the library comment.
  Future<Result<SecretLookup>> get(String name);

  /// Writes [secret] under [name], replacing any existing value.
  Future<Result<void>> set(String name, List<int> secret);

  /// Removes the entry under [name]. Removing an absent entry is not a failure: the caller's
  /// intent is that nothing be stored there, and that is already true.
  Future<Result<void>> delete(String name);
}

/// The real store, backed by `flutter_secure_storage`.
///
/// # UNVERIFIED on Android and iOS
///
/// Written against the package's documented behaviour and exercised only on the development
/// machine. Keystore and Keychain differ in their failure modes — key invalidation after a
/// biometric change, backup-and-restore behaviour, availability before first unlock — and
/// neither can be exercised from a desktop test run. Every *decision* that consumes this
/// interface is covered against [InMemorySecretStore], so what is unverified is the storage
/// mechanism, not the logic around it.
final class FlutterSecureStorageSecretStore implements SecretStore {
  FlutterSecureStorageSecretStore([FlutterSecureStorage? storage])
      : _storage = storage ??
            const FlutterSecureStorage(
              // Survives backup/restore only on the same device, and is unavailable until
              // first unlock. A key that restored onto a *different* device would give two
              // machines the same identity, which pairing treats as one device.
              iOptions: IOSOptions(
                accessibility: KeychainAccessibility.first_unlock_this_device,
              ),
              // The v11 default is already Keystore-backed: AES-GCM for the data with an RSA
              // OAEP-wrapped key. Left explicit rather than omitted so a future version
              // changing the default is a visible diff here rather than a silent weakening.
              aOptions: AndroidOptions(),
            );

  final FlutterSecureStorage _storage;

  /// Bytes are stored base64-encoded because the platform APIs are string-valued.
  @override
  Future<Result<SecretLookup>> get(String name) async {
    try {
      final raw = await _storage.read(key: name);
      if (raw == null) {
        // The one outcome that means "there is nothing here" rather than "I could not look".
        return Result.success(const SecretNotFound());
      }
      return Result.success(SecretFound(base64Decode(raw)));
    } catch (e) {
      // Everything else is a failure to consult the store, reported as such so the caller
      // does not mistake it for a first run and regenerate.
      return Result.failure(
        StorageError(message: 'the platform credential store could not be read: $e'),
      );
    }
  }

  @override
  Future<Result<void>> set(String name, List<int> secret) async {
    try {
      await _storage.write(key: name, value: base64Encode(secret));
      return const Success(null);
    } catch (e) {
      return Result.failure(
        StorageError(message: 'the platform credential store could not be written: $e'),
      );
    }
  }

  @override
  Future<Result<void>> delete(String name) async {
    try {
      await _storage.delete(key: name);
      return const Success(null);
    } catch (e) {
      return Result.failure(
        StorageError(message: 'the platform credential store could not be cleared: $e'),
      );
    }
  }
}

/// A non-persistent store for tests.
///
/// Exists because the platform stores cannot be exercised on every target from a developer
/// machine, and because a test that wrote to a real Keychain would leave state behind on the
/// machine running it. Every decision in this change — generate once, reuse, error rather than
/// regenerate, sign but never read — is tested against this.
///
/// [failWith] makes the store report itself unavailable, which is how the "an unreadable
/// identity is an error" scenario is reached without locking a real keystore.
final class InMemorySecretStore implements SecretStore {
  final Map<String, List<int>> _entries = {};

  /// When set, every operation fails with this message rather than touching [_entries].
  /// Models a locked keystore or a failed platform channel — the case that must never be
  /// mistaken for an empty store.
  String? _unavailable;

  /// Makes every subsequent operation report the store as unavailable.
  ///
  /// Note what this is *not*: it does not remove the entries. That is the whole point — the
  /// secret is still there, the store simply cannot be consulted, and a caller that responded
  /// by generating a replacement would destroy a recoverable situation.
  void failWith(String message) => _unavailable = message;

  /// Restores normal operation, so a test can prove the secret survived the outage.
  void recover() => _unavailable = null;

  @override
  Future<Result<SecretLookup>> get(String name) async {
    final failure = _unavailable;
    if (failure != null) {
      return Result.failure(StorageError(message: failure));
    }
    final secret = _entries[name];
    return Result.success(
      secret == null ? const SecretNotFound() : SecretFound(List.unmodifiable(secret)),
    );
  }

  @override
  Future<Result<void>> set(String name, List<int> secret) async {
    final failure = _unavailable;
    if (failure != null) {
      return Result.failure(StorageError(message: failure));
    }
    _entries[name] = List<int>.from(secret);
    return const Success(null);
  }

  @override
  Future<Result<void>> delete(String name) async {
    final failure = _unavailable;
    if (failure != null) {
      return Result.failure(StorageError(message: failure));
    }
    _entries.remove(name);
    return const Success(null);
  }
}
