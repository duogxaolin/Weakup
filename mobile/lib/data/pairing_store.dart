/// The pairings this device holds, and the key map the acceptance rule checks against.
///
/// `remote-device-pairing` specifies pairing as the only act that confers authority to command
/// a device, and specifies revocation. Until this library existed there was nowhere to write
/// either, so both were specified-but-unimplemented: pairing was a decision function with
/// nothing behind it.
///
/// # Revocation is a flag, not a deletion
///
/// Revoking sets `revokedAt`; it does not remove the row. Deleting would lose the record that
/// a pairing ever existed — which is exactly what someone investigating an unexplained
/// shutdown wants to see — and would make "was this device ever paired?" unanswerable, a
/// question that matters most after a device is lost.
///
/// Re-pairing a revoked device clears `revokedAt` on the **same row** rather than inserting a
/// second. Two rows for one peer would make "is this device authorized?" depend on which one
/// is read first, and that kind of ambiguity eventually resolves the wrong way. The peer id is
/// the table's primary key, so a second row is not merely discouraged but impossible.
///
/// # A revoked pairing's key is absent, not present-and-rejected
///
/// [verifyingKeysFromPairings] filters revoked rows out of the map, so a command from a
/// revoked peer fails as an *unknown sender*. Including the key and refusing at a later
/// pairing check would verify the signature of a device the user explicitly de-authorized,
/// then refuse it for a different reason — reporting the wrong thing and doing cryptographic
/// work on a revoked device's behalf. Absent is the honest representation of revoked.
///
/// Mirrors `desktop/src-tauri/src/data/pairing_store.rs`, and
/// `shared/testvectors/pairing_store.json` pins the decisions both sides must agree on.
library;

import 'package:drift/drift.dart';

import '../application/pairing_flow.dart';
import '../core/app_error.dart';
import '../core/result.dart';
import '../domain/device_id.dart';
import '../domain/device_identity.dart';
import '../domain/signature.dart';
import 'app_database.dart';
import 'tables.dart';

part 'pairing_store.g.dart';

/// One peer this device is paired with.
final class PairingRecord {
  const PairingRecord({
    required this.peer,
    required this.verifyingKey,
    required this.revoked,
  });

  final DeviceId peer;

  /// The key recorded when this peer was paired. Subsequent commands from it are checked
  /// against exactly this, never against a key supplied alongside a command.
  final VerifyingKey verifyingKey;

  /// Whether the pairing has been revoked. A revoked pairing is retained for the record but
  /// confers no authority.
  final bool revoked;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is PairingRecord &&
          other.peer == peer &&
          other.verifyingKey == verifyingKey &&
          other.revoked == revoked);

  @override
  int get hashCode => Object.hash(peer, verifyingKey, revoked);
}

/// Builds the key map `evaluateCommand` checks signatures against.
///
/// **Revoked pairings are excluded** — see the library comment for why absent rather than
/// present-and-rejected-later is the honest representation.
///
/// This is the only place in production code that assembles such a map. The rule itself still
/// takes it as data, which keeps it pure and keeps every vector case writable without a store
/// fixture; what changed is that no caller invents one.
Map<DeviceId, VerifyingKey> verifyingKeysFromPairings(
  List<PairingRecord> pairings,
) {
  return {
    for (final record in pairings)
      if (!record.revoked) record.peer: record.verifyingKey,
  };
}

/// Pairings and this device's own identity, in the app database.
///
/// Shares the database with jobs rather than opening a second one, for the same reason the
/// job DAO does.
///
/// Implements [PairingRecorder] so the pairing exchange can be written against the two
/// operations it actually needs rather than against this whole class. The exchange must be
/// testable against a store whose write fails — that is the only way to establish the
/// both-or-neither guarantee — and a concrete drift accessor cannot be made to fail on demand.
@DriftAccessor(tables: [Pairings, DeviceIdentities])
class PairingStore extends DatabaseAccessor<AppDatabase>
    with _$PairingStoreMixin
    implements DeviceIdentityRecordStore, PairingRecorder {
  PairingStore(super.db);

  /// Records a pairing with [peer], or re-establishes a revoked one.
  ///
  /// Re-pairing **clears the revocation on the existing row** rather than adding another.
  /// Revoking withdraws the authority previously granted; it does not blacklist the device,
  /// and a user who revokes a phone after mislaying it must be able to pair it again when it
  /// turns up.
  @override
  Future<Result<void>> recordPairing({
    required DeviceId peer,
    required VerifyingKey verifyingKey,
    required DateTime pairedAt,
  }) async {
    try {
      // An upsert on the peer id rather than an insert: a second row for the same peer would
      // make authorization depend on read order. The primary key makes that impossible; this
      // clause makes re-pairing work rather than fail.
      await into(pairings).insertOnConflictUpdate(
        PairingsCompanion.insert(
          peerDeviceId: peer.value,
          verifyingKey: Uint8List.fromList(verifyingKey.bytes),
          pairedAt: pairedAt,
          revokedAt: const Value(null),
        ),
      );
      return const Success(null);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  /// Every pairing, revoked ones included. For showing the user what this device has ever
  /// been paired with.
  Future<Result<List<PairingRecord>>> listPairings() async {
    try {
      final rows = await select(pairings).get();
      final records = <PairingRecord>[];

      for (final row in rows) {
        final key = VerifyingKey.fromBytes(row.verifyingKey);
        if (key == null) {
          return Result.failure(
            StorageError(
              message: 'the pairing with ${row.peerDeviceId} holds an unusable '
                  'verifying key',
            ),
          );
        }
        records.add(
          PairingRecord(
            peer: DeviceId(row.peerDeviceId),
            verifyingKey: key,
            // Null means active. The instant itself is not needed to decide authority — only
            // whether it is set — but it is stored so the record can say *when* authority
            // was withdrawn.
            revoked: row.revokedAt != null,
          ),
        );
      }

      return Result.success(records);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  /// Withdraws [peer]'s authority, with effect for every command evaluated afterwards.
  ///
  /// A local act: it takes effect without reference to any relay or network, because a device
  /// must be de-authorizable when nothing is reachable — which is exactly when someone is
  /// most likely to be doing it.
  @override
  Future<Result<void>> revokePairing({
    required DeviceId peer,
    required DateTime revokedAt,
  }) async {
    try {
      // Sets the flag; never deletes.
      final affected = await (update(pairings)
            ..where((p) => p.peerDeviceId.equals(peer.value)))
          .write(PairingsCompanion(revokedAt: Value(revokedAt)));

      // Silence here would let a user believe they had de-authorized a lost device when no
      // such pairing was ever recorded.
      if (affected == 0) {
        return Result.failure(
          StorageError(message: 'no pairing with $peer to revoke'),
        );
      }
      return const Success(null);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  /// The keys commands are actually checked against — active pairings only.
  Future<Result<Map<DeviceId, VerifyingKey>>> verifyingKeysFromStore() async {
    final listed = await listPairings();
    return listed.map(verifyingKeysFromPairings);
  }

  // ---- this device's own identity: public material only ----

  @override
  Future<Result<DeviceIdentityRecord?>> loadIdentityRecord() async {
    try {
      final row = await (select(deviceIdentities)
            ..where((d) => d.id.equals(1)))
          .getSingleOrNull();
      if (row == null) return Result.success(null);

      final key = VerifyingKey.fromBytes(row.verifyingKey);
      if (key == null) {
        return Result.failure(
          const StorageError(
            message: 'the identity record holds an unusable verifying key',
          ),
        );
      }

      return Result.success(
        DeviceIdentityRecord(deviceId: DeviceId(row.deviceId), verifyingKey: key),
      );
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  @override
  Future<Result<void>> saveIdentityRecord(DeviceIdentityRecord record) async {
    try {
      await into(deviceIdentities).insertOnConflictUpdate(
        DeviceIdentitiesCompanion.insert(
          id: const Value(1),
          deviceId: record.deviceId.value,
          verifyingKey: Uint8List.fromList(record.verifyingKey.bytes),
          createdAt: DateTime.now().toUtc(),
        ),
      );
      return const Success(null);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }
}
