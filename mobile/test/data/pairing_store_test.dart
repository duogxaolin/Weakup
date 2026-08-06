/// The Dart mirror of `desktop/src-tauri/src/data/pairing_store_tests.rs`.
///
/// Covers the store's behaviour, the D5 revoked-means-absent rule, the D4 re-pairing rule
/// (including the row-count assertion the rule turns on), and the version 3 to 4 migration
/// against a genuine pre-migration database.
library;

import 'dart:io';

// `isNull`/`isNotNull` are declared by both drift's query builder and matcher. Only the
// matcher ones are wanted here; drift's are hidden rather than the import being aliased, so
// the table and companion types stay directly usable.
import 'package:drift/drift.dart' hide isNull, isNotNull;
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/data/app_database.dart';
import 'package:weakup/data/pairing_store.dart';
import 'package:weakup/domain/domain.dart';

AppDatabase _openTestDb() => AppDatabase(NativeDatabase.memory());

/// A drift database with no schema of its own, used only to run raw statements against a file.
///
/// Mirrors the helper in `job_origin_and_decisions_test.dart`, and exists for the same reason:
/// the pre-migration fixture must be built without importing `package:sqlite3` directly, which
/// is a transitive dependency of drift rather than one this package declares.
class _RawExecutorDatabase extends GeneratedDatabase {
  _RawExecutorDatabase(super.executor);

  @override
  Iterable<TableInfo<Table, dynamic>> get allTables => const [];

  @override
  int get schemaVersion => 1;
}

final _phone = DeviceId('phone-a');
final _laptop = DeviceId('laptop-b');

/// The public halves of the fixed test seeds the rest of the suite uses.
VerifyingKey _key(int seed) {
  const hexes = {
    1: '8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c',
    2: '8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394',
    3: 'ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1',
  };
  final hex = hexes[seed]!;
  return VerifyingKey.fromBytes([
    for (var i = 0; i < hex.length; i += 2)
      int.parse(hex.substring(i, i + 2), radix: 16),
  ])!;
}

DateTime _at(int hour) => DateTime.utc(2026, 8, 7, hour);

void main() {
  late AppDatabase db;
  late PairingStore store;

  setUp(() {
    db = _openTestDb();
    store = PairingStore(db);
  });

  tearDown(() async => db.close());

  // ---- recording and reading back ----

  test('a pairing round trips with its key', () async {
    // The spec requires the pairing record the peer's *key*, not only its name: a pairing
    // storing an identifier alone would authorize whoever presented that name.
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));

    final pairings = (await store.listPairings()).valueOrNull!;
    expect(pairings, hasLength(1));
    expect(pairings.first.peer, _phone);
    expect(pairings.first.verifyingKey, _key(1));
    expect(pairings.first.revoked, isFalse);
  });

  test('an active pairing key appears in the map', () async {
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));

    final keys = (await store.verifyingKeysFromStore()).valueOrNull!;

    expect(keys[_phone], _key(1));
  });

  test('a device that was never paired has no key', () async {
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));

    final keys = (await store.verifyingKeysFromStore()).valueOrNull!;

    expect(keys[_laptop], isNull);
  });

  // ---- revocation ----

  test('revoking removes the key from the map', () async {
    // Design D5: absent, not present-and-rejected-later. A revoked peer's command fails as an
    // unknown sender rather than having its signature verified first.
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));

    await store.revokePairing(peer: _phone, revokedAt: _at(13));

    final keys = (await store.verifyingKeysFromStore()).valueOrNull!;
    expect(keys[_phone], isNull,
        reason: "a revoked pairing's key must be absent from the map, not present for a "
            'later check');
  });

  test('revoking retains the row rather than deleting it', () async {
    // Design D4. The record that a pairing existed is what someone investigating an
    // unexplained shutdown needs.
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));

    await store.revokePairing(peer: _phone, revokedAt: _at(13));

    final pairings = (await store.listPairings()).valueOrNull!;
    expect(pairings, hasLength(1), reason: 'the row must be retained');
    expect(pairings.first.revoked, isTrue);
  });

  test('revocation is per pairing rather than global', () async {
    // An implementation treating any revocation as a switch that disables the store passes
    // every single-peer test above.
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));
    await store.recordPairing(
        peer: _laptop, verifyingKey: _key(2), pairedAt: _at(12));

    await store.revokePairing(peer: _phone, revokedAt: _at(13));

    final keys = (await store.verifyingKeysFromStore()).valueOrNull!;
    expect(keys[_phone], isNull);
    expect(keys[_laptop], _key(2),
        reason: 'revoking one pairing must not disturb another');
  });

  test('revoking an unknown peer is a failure', () async {
    // Silence would let a user believe they had de-authorized a lost device when no such
    // pairing was recorded.
    final result = await store.revokePairing(peer: _phone, revokedAt: _at(13));

    expect(result.isFailure, isTrue);
  });

  // ---- re-pairing: the row count is the point ----

  test('a revoked device can be paired again and regains authority', () async {
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));
    await store.revokePairing(peer: _phone, revokedAt: _at(13));

    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(14));

    final keys = (await store.verifyingKeysFromStore()).valueOrNull!;
    expect(keys[_phone], _key(1));
  });

  test('re-pairing clears the revocation on the same row rather than adding a second',
      () async {
    // **The row count is the assertion**, not merely that the key came back. A second row
    // would satisfy a key-presence check while leaving exactly the ambiguity design D4 exists
    // to prevent: "is this device authorized?" would depend on which row is read first.
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));
    await store.revokePairing(peer: _phone, revokedAt: _at(13));

    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(14));

    final pairings = (await store.listPairings()).valueOrNull!;
    expect(
      pairings,
      hasLength(1),
      reason: 're-pairing must clear the revocation on the existing row, never insert a '
          'second: two rows for one peer make authorization depend on read order',
    );
    expect(pairings.first.revoked, isFalse, reason: 'the revocation must be cleared');
  });

  test('re-pairing with a new key replaces the recorded one', () async {
    // A device that was reinstalled has a new identity key. Pairing it again must record the
    // key actually presented — and still only one row.
    await store.recordPairing(
        peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));

    await store.recordPairing(
        peer: _phone, verifyingKey: _key(2), pairedAt: _at(14));

    final pairings = (await store.listPairings()).valueOrNull!;
    expect(pairings, hasLength(1));
    expect(pairings.first.verifyingKey, _key(2));
  });

  // ---- the pure map builder ----

  test('the map builder excludes revoked pairings', () {
    final keys = verifyingKeysFromPairings([
      PairingRecord(peer: _phone, verifyingKey: _key(1), revoked: true),
      PairingRecord(peer: _laptop, verifyingKey: _key(2), revoked: false),
    ]);

    expect(keys, hasLength(1));
    expect(keys[_phone], isNull);
    expect(keys[_laptop], _key(2));
  });

  test('an empty store authorizes nobody', () {
    // The first-run state. An implementation defaulting to permissive when it holds no
    // pairings would obey every command it received.
    expect(verifyingKeysFromPairings([]), isEmpty);
  });

  // ---- this device's own identity record ----

  test('the identity record round trips and holds only public material', () async {
    final record = DeviceIdentityRecord(
      deviceId: deriveDeviceId(_key(1).bytes),
      verifyingKey: _key(1),
    );

    await store.saveIdentityRecord(record);

    final loaded = (await store.loadIdentityRecord()).valueOrNull;
    expect(loaded, record);
  });

  test('a device with no identity yet reports none', () async {
    expect((await store.loadIdentityRecord()).valueOrNull, isNull);
  });

  test('saving twice replaces rather than adding a second identity', () async {
    // Two identities would make "which key am I signing with" ambiguous.
    await store.saveIdentityRecord(DeviceIdentityRecord(
        deviceId: deriveDeviceId(_key(1).bytes), verifyingKey: _key(1)));
    await store.saveIdentityRecord(DeviceIdentityRecord(
        deviceId: deriveDeviceId(_key(2).bytes), verifyingKey: _key(2)));

    expect(await db.select(db.deviceIdentities).get(), hasLength(1));
    expect((await store.loadIdentityRecord()).valueOrNull!.verifyingKey, _key(2));
  });

  // ---- durability and migration ----

  group('the version 3 to 4 migration', () {
    test('a genuine v3 database upgrades with its existing rows intact', () async {
      // A genuine pre-migration database. The v3 shape is written to a real file and the file
      // is then *closed*; only on the second open does Drift see user_version 3 and run the
      // actual onUpgrade path. Asserting against a database that was already at version 4
      // would prove nothing about upgrading, which is the trap this test exists to avoid.
      final dir = await Directory.systemTemp.createTemp('weakup_v3_');
      final file = File('${dir.path}/v3.sqlite');

      final seed = _RawExecutorDatabase(NativeDatabase(file));
      await seed.customStatement('''
        CREATE TABLE jobs (
          id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
          type TEXT NOT NULL,
          trigger_kind TEXT NOT NULL,
          trigger_minutes INTEGER NULL,
          trigger_hour INTEGER NULL,
          trigger_minute INTEGER NULL,
          trigger_date TEXT NULL,
          status TEXT NOT NULL,
          target_instant_utc INTEGER NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          failure_message TEXT NULL,
          origin TEXT NULL
        )
      ''');
      await seed.customStatement('''
        CREATE TABLE command_decisions (
          id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
          sender_device_id TEXT NOT NULL,
          command TEXT NOT NULL,
          decision TEXT NOT NULL,
          rejection_reason TEXT NULL,
          decided_at INTEGER NOT NULL
        )
      ''');
      // Rows written by the old version, which the migration must not disturb.
      await seed.customStatement('''
        INSERT INTO jobs (type, trigger_kind, trigger_minutes, status,
                          target_instant_utc, created_at, updated_at, origin)
        VALUES ('powerOff', 'duration', 30, 'active', 1000, 1000, 1000, 'remote')
      ''');
      await seed.customStatement('''
        INSERT INTO command_decisions (sender_device_id, command, decision, decided_at)
        VALUES ('phone-a', 'powerOff', 'accepted', 1000)
      ''');
      await seed.customStatement('PRAGMA user_version = 3');
      await seed.close();

      // Opening runs onUpgrade from 3 to 4.
      final migrated = AppDatabase(NativeDatabase(file));
      addTearDown(migrated.close);

      // Nothing existing was rewritten. The migration is additive.
      final jobs = await migrated.select(migrated.jobs).get();
      expect(jobs, hasLength(1), reason: 'the pre-migration job must survive');
      expect(jobs.first.origin, 'remote',
          reason: 'the origin recorded by the previous migration must be preserved — '
              'replacing the migration strategy rather than appending to it is what would '
              'lose it');
      expect(await migrated.select(migrated.commandDecisions).get(), hasLength(1),
          reason: 'the table the previous migration created must survive');

      // And the new tables arrive with this migration, so an upgraded install can pair
      // immediately rather than on some later launch.
      final upgradedStore = PairingStore(migrated);
      await upgradedStore.recordPairing(
          peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));
      expect((await upgradedStore.listPairings()).valueOrNull, hasLength(1));

      await upgradedStore.saveIdentityRecord(DeviceIdentityRecord(
          deviceId: deriveDeviceId(_key(2).bytes), verifyingKey: _key(2)));
      expect((await upgradedStore.loadIdentityRecord()).valueOrNull, isNotNull);
    });

    test('pairings and revocations both survive a restart', () async {
      // Both halves in one test, because they fail in opposite directions and each is a spec
      // scenario: a forgotten pairing silently de-authorizes a peer, while a forgotten
      // revocation leaves a lost device in control after the owner believed otherwise.
      final dir = await Directory.systemTemp.createTemp('weakup_restart_');
      final file = File('${dir.path}/weakup.sqlite');

      final first = AppDatabase(NativeDatabase(file));
      final firstStore = PairingStore(first);
      await firstStore.recordPairing(
          peer: _phone, verifyingKey: _key(1), pairedAt: _at(12));
      await firstStore.recordPairing(
          peer: _laptop, verifyingKey: _key(2), pairedAt: _at(12));
      await firstStore.revokePairing(peer: _phone, revokedAt: _at(13));
      await first.close();

      final reopened = AppDatabase(NativeDatabase(file));
      addTearDown(reopened.close);
      final keys =
          (await PairingStore(reopened).verifyingKeysFromStore()).valueOrNull!;

      expect(keys[_phone], isNull,
          reason: 'a revocation that a restart undid would be worse than none');
      expect(keys[_laptop], _key(2),
          reason: 'a pairing forgotten on restart would silently de-authorize a peer');
    });
  });
}
