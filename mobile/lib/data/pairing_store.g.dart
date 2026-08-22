// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'pairing_store.dart';

// ignore_for_file: type=lint
mixin _$PairingStoreMixin on DatabaseAccessor<AppDatabase> {
  $PairingsTable get pairings => attachedDatabase.pairings;
  $DeviceIdentitiesTable get deviceIdentities =>
      attachedDatabase.deviceIdentities;
  PairingStoreManager get managers => PairingStoreManager(this);
}

class PairingStoreManager {
  final _$PairingStoreMixin _db;
  PairingStoreManager(this._db);
  $$PairingsTableTableManager get pairings =>
      $$PairingsTableTableManager(_db.attachedDatabase, _db.pairings);
  $$DeviceIdentitiesTableTableManager get deviceIdentities =>
      $$DeviceIdentitiesTableTableManager(
        _db.attachedDatabase,
        _db.deviceIdentities,
      );
}
