/// The Dart mirror of `desktop/src-tauri/src/domain/device_identity_tests.rs`.
///
/// Every decision is exercised against [InMemorySecretStore] rather than a real Keystore or
/// Keychain, because those cannot be reached from a desktop test run. What that leaves
/// unverified is the storage mechanism, not the logic around it.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/core/core.dart';
import 'package:weakup/domain/domain.dart';
import 'package:weakup/platform/secret_store.dart';

/// An in-memory stand-in for the database's identity record.
final class FakeRecordStore implements DeviceIdentityRecordStore {
  DeviceIdentityRecord? record;

  @override
  Future<Result<DeviceIdentityRecord?>> loadIdentityRecord() async =>
      Result.success(record);

  @override
  Future<Result<void>> saveIdentityRecord(DeviceIdentityRecord r) async {
    record = r;
    return const Success(null);
  }
}

void main() {
  // ---- derivation ----

  test('the device id is sixteen bytes of lowercase hex', () {
    final id = deriveDeviceId(List.filled(32, 0));

    expect(id.value.length, deviceIdBytes * 2);
    expect(RegExp(r'^[0-9a-f]+$').hasMatch(id.value), isTrue,
        reason: 'the two implementations must agree on case as well as content');
  });

  test('the same key always derives the same id', () {
    expect(deriveDeviceId(List.filled(32, 3)), deriveDeviceId(List.filled(32, 3)));
  });

  test('different keys derive different ids', () {
    expect(deriveDeviceId(List.filled(32, 3)),
        isNot(equals(deriveDeviceId(List.filled(32, 4)))));
  });

  test('a one-byte change anywhere in the key changes the id', () {
    // The property the shared vectors also pin, asserted here across every byte position
    // rather than the three the vectors name. An implementation hashing a prefix passes the
    // vectors' first case and fails here.
    final base = deriveDeviceId(List.filled(32, 0));

    for (var position = 0; position < 32; position++) {
      final key = List.filled(32, 0);
      key[position] = 1;

      expect(
        deriveDeviceId(key),
        isNot(equals(base)),
        reason: 'changing byte $position must change the derived id; if it does not, the '
            'derivation ignores part of the key and two devices could claim one identifier',
      );
    }
  });

  // ---- load or generate ----

  test('the first run generates an identity and records both halves', () async {
    final records = FakeRecordStore();
    final secrets = InMemorySecretStore();

    final result = await loadOrGenerateIdentity(records: records, secrets: secrets);
    final identity = result.valueOrNull!;

    // The public half is recorded in ordinary storage, so the device can say who it is
    // without unlocking anything.
    expect(records.record, isNotNull);
    expect(records.record!.deviceId, identity.deviceId);
    expect(records.record!.verifyingKey, identity.verifyingKey);

    // The private half went to the secret store.
    final stored = await secrets.get(identitySecretName);
    expect(stored.valueOrNull, isA<SecretFound>());
  });

  test('a later run reuses the same identity', () async {
    // The property that makes a pairing survive a restart: the id a peer recorded must still
    // be this device's id tomorrow.
    final records = FakeRecordStore();
    final secrets = InMemorySecretStore();

    final first = await loadOrGenerateIdentity(records: records, secrets: secrets);
    final second = await loadOrGenerateIdentity(records: records, secrets: secrets);

    expect(second.valueOrNull!.deviceId, first.valueOrNull!.deviceId);
    expect(second.valueOrNull!.verifyingKey, first.valueOrNull!.verifyingKey);
  });

  test('two devices generate different identities', () async {
    // Independent installations must not collide, or one device's pairings would authorize
    // another's commands.
    final a = await loadOrGenerateIdentity(
        records: FakeRecordStore(), secrets: InMemorySecretStore());
    final b = await loadOrGenerateIdentity(
        records: FakeRecordStore(), secrets: InMemorySecretStore());

    expect(a.valueOrNull!.deviceId, isNot(equals(b.valueOrNull!.deviceId)));
  });

  test('a record without a key is a failure and generates nothing', () async {
    // The scenario the spec names explicitly. The record says an identity exists but the
    // secret store cannot produce it. Generating a replacement would give this device a new
    // id, making it a stranger to every peer it is paired with.
    final records = FakeRecordStore();
    final secrets = InMemorySecretStore();

    final original = await loadOrGenerateIdentity(records: records, secrets: secrets);
    final originalId = original.valueOrNull!.deviceId;

    // Something removed the key: a keystore reset, a migration, a user clearing credentials.
    await secrets.delete(identitySecretName);

    final result = await loadOrGenerateIdentity(records: records, secrets: secrets);

    expect(result.isFailure, isTrue,
        reason: 'a missing key under an existing record must fail, never regenerate silently');
    expect(result.errorOrNull, isA<StorageError>());

    // Nothing was generated. This is the assertion that would fail if the error were reported
    // *after* regenerating, which would be just as destructive.
    expect(records.record!.deviceId, originalId,
        reason: 'the recorded identity must be untouched');
    final stored = await secrets.get(identitySecretName);
    expect(stored.valueOrNull, isA<SecretNotFound>(),
        reason: 'no replacement key may be written');
  });

  test('an unreachable store is a failure and the identity survives it', () async {
    // The recoverable case, which must not be converted into an unrecoverable one. A locked
    // keystore comes back; a regenerated identity does not.
    final records = FakeRecordStore();
    final secrets = InMemorySecretStore();

    final original = await loadOrGenerateIdentity(records: records, secrets: secrets);
    final originalId = original.valueOrNull!.deviceId;

    secrets.failWith('the keystore is locked');
    expect((await loadOrGenerateIdentity(records: records, secrets: secrets)).isFailure,
        isTrue);

    secrets.recover();
    final recovered = await loadOrGenerateIdentity(records: records, secrets: secrets);
    expect(recovered.valueOrNull!.deviceId, originalId,
        reason: 'the identity survived the outage; regenerating would have thrown it away');
  });

  test('a key that does not match the record is a failure', () async {
    // The two halves must describe the same device. If they disagree, proceeding would sign
    // commands under an identifier peers do not associate with this key.
    final records = FakeRecordStore();
    final secrets = InMemorySecretStore();
    await loadOrGenerateIdentity(records: records, secrets: secrets);

    await secrets.set(identitySecretName, List.filled(32, 9));

    final result = await loadOrGenerateIdentity(records: records, secrets: secrets);
    expect(result.isFailure, isTrue);
  });

  test('a stored key of the wrong length is a failure rather than a crash', () async {
    // The stored bytes come from outside and may be corrupt. A throw here would take the app
    // down at startup.
    final records = FakeRecordStore();
    final secrets = InMemorySecretStore();
    await loadOrGenerateIdentity(records: records, secrets: secrets);

    await secrets.set(identitySecretName, [1, 2, 3]);

    expect((await loadOrGenerateIdentity(records: records, secrets: secrets)).isFailure,
        isTrue);
  });

  test('the public identity is readable without consulting the secret store', () async {
    // The spec scenario: a device reports who it is from ordinary storage, without needing
    // the private key.
    final records = FakeRecordStore();
    final secrets = InMemorySecretStore();
    final identity = await loadOrGenerateIdentity(records: records, secrets: secrets);
    final expectedId = identity.valueOrNull!.deviceId;

    secrets.failWith('the keystore is locked');

    final record = await records.loadIdentityRecord();
    expect(record.valueOrNull!.deviceId, expectedId);
  });

  // ---- signing ----

  test('a signature from sign verifies against the published verifying key', () async {
    // The spec scenario: what the identity signs, a peer holding its published key can check.
    final identity = (await loadOrGenerateIdentity(
      records: FakeRecordStore(),
      secrets: InMemorySecretStore(),
    ))
        .valueOrNull!;

    final payload = [1, 2, 3, 4, 5];
    final signature = await identity.sign(payload);

    expect(
      await verifySignature(
          payload: payload, signature: signature, key: identity.verifyingKey),
      isTrue,
      reason: 'a signature this device produced must verify against the key it publishes',
    );
  });

  test('a signature does not verify over different content', () async {
    final identity = (await loadOrGenerateIdentity(
      records: FakeRecordStore(),
      secrets: InMemorySecretStore(),
    ))
        .valueOrNull!;

    final signature = await identity.sign([1, 2, 3]);

    expect(
      await verifySignature(
          payload: [9, 9, 9], signature: signature, key: identity.verifyingKey),
      isFalse,
    );
  });

  test("one device's signature does not verify against another's key", () async {
    final a = (await loadOrGenerateIdentity(
            records: FakeRecordStore(), secrets: InMemorySecretStore()))
        .valueOrNull!;
    final b = (await loadOrGenerateIdentity(
            records: FakeRecordStore(), secrets: InMemorySecretStore()))
        .valueOrNull!;

    final signature = await a.sign([1, 2, 3]);

    expect(
      await verifySignature(payload: [1, 2, 3], signature: signature, key: b.verifyingKey),
      isFalse,
    );
  });

  test('the device id matches the derivation of its own verifying key', () async {
    // The self-certifying property, as an internal consistency check: the identifier this
    // device reports is the one its key derives to.
    final identity = (await loadOrGenerateIdentity(
      records: FakeRecordStore(),
      secrets: InMemorySecretStore(),
    ))
        .valueOrNull!;

    expect(identity.deviceId, deriveDeviceId(identity.verifyingKey.bytes));
  });

  test('toString does not contain key material', () async {
    // The behavioural counterpart to the source-text guard.
    final identity = (await identityFromSeed(List.filled(32, 7))).valueOrNull!;

    final rendered = identity.toString();

    expect(rendered, contains('<sealed>'));
    expect(rendered, contains(identity.deviceId.value));
    expect(rendered, isNot(contains('07070707')));
    expect(rendered, isNot(contains('7, 7, 7')));
  });

  // ---- the secret store's own contract ----

  group('the secret store', () {
    const name = 'a-secret';

    test('a stored secret round trips', () async {
      final store = InMemorySecretStore();
      await store.set(name, [1, 2, 3, 4]);

      final found = (await store.get(name)).valueOrNull;

      expect(found, isA<SecretFound>());
      expect((found as SecretFound).secret, [1, 2, 3, 4]);
    });

    test('an absent entry is not-found rather than a failure', () async {
      // The first-run answer. It must be a success, because a caller distinguishing "generate
      // one" from "something is wrong" reads exactly this.
      final store = InMemorySecretStore();

      final result = await store.get(name);

      expect(result.isSuccess, isTrue);
      expect(result.valueOrNull, isA<SecretNotFound>());
    });

    test('a deleted entry reads as not found', () async {
      final store = InMemorySecretStore();
      await store.set(name, [1]);

      await store.delete(name);

      expect((await store.get(name)).valueOrNull, isA<SecretNotFound>());
    });

    test('deleting an absent entry is not a failure', () async {
      expect((await InMemorySecretStore().delete(name)).isSuccess, isTrue);
    });

    test('the two failure modes are distinguishable without reading a message', () async {
      // The distinction the whole library exists to preserve. A store that reported not-found
      // for an outage would tell a caller "there is no identity", and the caller would
      // generate a replacement — silently destroying every pairing this device holds.
      final empty = InMemorySecretStore();
      final unavailable = InMemorySecretStore();
      unavailable.failWith('no credential service');

      final absent = await empty.get(name);
      final broken = await unavailable.get(name);

      expect(absent.isSuccess, isTrue);
      expect(absent.valueOrNull, isA<SecretNotFound>());
      expect(broken.isFailure, isTrue);
    });

    test('an outage does not destroy the stored secret', () async {
      final store = InMemorySecretStore();
      await store.set(name, [7, 7, 7]);

      store.failWith('locked');
      expect((await store.get(name)).isFailure, isTrue);

      store.recover();
      final found = (await store.get(name)).valueOrNull;
      expect((found as SecretFound).secret, [7, 7, 7],
          reason: 'the secret was intact the whole time; only the store was unreachable');
    });
  });
}
