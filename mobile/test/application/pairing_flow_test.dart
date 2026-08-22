/// The Dart mirror of `desktop/src-tauri/src/application/pairing_flow_tests.rs`.
///
/// # The four cases the spec names
///
/// A code redeemed inside its lifetime pairs both sides; an expired code pairs neither; a code
/// presented twice pairs once; and an exchange that fails partway pairs neither. The last one
/// **asserts on both stores**, which is the whole point: a test that checked only the requester's
/// store would pass just as happily against an implementation where the issuer silently recorded
/// nothing, and that is exactly the bug both-or-neither exists to prevent.
library;

import 'dart:math';

import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/application/pairing_flow.dart';
import 'package:weakup/core/app_error.dart';
import 'package:weakup/core/result.dart';
import 'package:weakup/data/app_database.dart';
import 'package:weakup/data/pairing_store.dart';
import 'package:weakup/domain/domain.dart';

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

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

PairingIdentity get _phone => PairingIdentity(
      deviceId: DeviceId('phone-a'),
      verifyingKey: _key(1),
    );

PairingIdentity get _laptop => PairingIdentity(
      deviceId: DeviceId('laptop-b'),
      verifyingKey: _key(2),
    );

DateTime _at(int seconds) =>
    DateTime.utc(2026, 8, 7, 12).add(Duration(seconds: seconds));

/// A store whose `recordPairing` fails, standing in for a device whose storage is full or whose
/// database is locked at the moment the exchange completes.
///
/// Only the write fails. Reads and revocation work, so a test can still ask what it believes —
/// which is the question case 4 has to answer for *both* devices.
class _FailingWriteStore implements PairingRecorder {
  _FailingWriteStore(this.inner);

  final PairingStore inner;

  @override
  Future<Result<void>> recordPairing({
    required DeviceId peer,
    required VerifyingKey verifyingKey,
    required DateTime pairedAt,
  }) async {
    return Result.failure(const StorageError(message: 'the database is locked'));
  }

  @override
  Future<Result<void>> revokePairing({
    required DeviceId peer,
    required DateTime revokedAt,
  }) =>
      inner.revokePairing(peer: peer, revokedAt: revokedAt);
}

/// Whether [store] holds an *authorizing* pairing with [peer] — the only sense of "believes it is
/// paired" that matters, since a revoked row confers nothing.
Future<bool> _believesPaired(PairingStore store, DeviceId peer) async {
  final keys = (await store.verifyingKeysFromStore()).valueOrNull!;
  return keys.containsKey(peer);
}

void main() {
  late AppDatabase issuerDb;
  late AppDatabase requesterDb;
  late PairingStore issuerStore;
  late PairingStore requesterStore;

  setUp(() {
    issuerDb = AppDatabase(NativeDatabase.memory());
    requesterDb = AppDatabase(NativeDatabase.memory());
    issuerStore = PairingStore(issuerDb);
    requesterStore = PairingStore(requesterDb);
  });

  tearDown(() async {
    await issuerDb.close();
    await requesterDb.close();
  });

  // -------------------------------------------------------------------------
  // The code
  // -------------------------------------------------------------------------

  group('the pairing code', () {
    test('is six characters from the alphabet', () {
      // Drawn repeatedly rather than once: a single draw could satisfy both properties by luck.
      for (var i = 0; i < 200; i++) {
        final code = generatePairingCode();
        expect(code.value.length, pairingCodeLength);
        for (final glyph in code.value.split('')) {
          expect(pairingCodeAlphabet.contains(glyph), isTrue,
              reason: '$code contains $glyph, which is not in the alphabet');
        }
      }
    });

    test('never contains a confusable glyph', () {
      // A user who mistypes sees noSuchGrant, which is indistinguishable to them from expired, so
      // they retry until the grant really has expired. Asserted over many draws rather than over
      // the alphabet constant alone, so a generator ignoring the alphabet is still caught.
      for (var i = 0; i < 200; i++) {
        final code = generatePairingCode();
        for (final confusable in confusableGlyphs) {
          expect(code.value.contains(confusable), isFalse,
              reason: '$code contains the confusable glyph $confusable');
        }
      }
    });

    test('the alphabet itself excludes every confusable glyph', () {
      // The other half of the check above. The generator could stop using the constant; the
      // constant could grow a bad glyph. Both are asserted.
      for (final confusable in confusableGlyphs) {
        expect(pairingCodeAlphabet.contains(confusable), isFalse,
            reason: 'the alphabet contains $confusable');
      }
    });

    test('two generated codes differ', () {
      // A generator returning a constant would pass every shape assertion above. 31^6 makes a
      // genuine collision vanishingly unlikely across this many draws.
      final codes = {for (var i = 0; i < 50; i++) generatePairingCode().value};
      expect(codes.length, greaterThan(45),
          reason: '50 draws produced only ${codes.length} distinct codes, which suggests the '
              'generator is not actually random');
    });

    test('the alphabet matches the Rust one exactly', () {
      // The two implementations must show the same glyph set, or a code generated on one device
      // could contain a character the other refuses at parse.
      expect(pairingCodeAlphabet, '23456789ABCDEFGHJKMNPQRSTUVWXYZ');
      expect(pairingCodeLength, 6);
    });

    test('a typed code is accepted regardless of case and spacing', () {
      // What a person actually types. None of this changes which code was meant, so none of it is
      // a refusal.
      for (final entered in [
        'ABC234',
        'abc234',
        ' ABC234 ',
        'ABC 234',
        'abc-234',
        'AbC 2 3 4',
      ]) {
        expect(PairingCode.parse(entered).valueOrNull?.value, 'ABC234',
            reason: '$entered should normalise to ABC234');
      }
    });

    test('a code of the wrong length or with a foreign glyph is refused at parse', () {
      // Refused here rather than looked up and reported as unrecognised. A genuinely mistyped
      // character must not consume the single use the grant has.
      for (final entered in ['ABC23', 'ABC2345', '']) {
        expect(PairingCode.parse(entered).isFailure, isTrue,
            reason: '$entered is the wrong length and should be refused');
      }

      // A confusable glyph is a foreign glyph: not silently mapped to its lookalike, because
      // accepting a code as something other than what was typed is harder to reason about.
      for (final entered in ['ABC23O', 'ABC23I', 'ABC23l', 'ABC2!4']) {
        expect(PairingCode.parse(entered).isFailure, isTrue,
            reason: '$entered contains a glyph outside the alphabet and should be refused');
      }
    });
  });

  // -------------------------------------------------------------------------
  // Case 1: a code presented within its lifetime pairs both sides
  // -------------------------------------------------------------------------

  test('a code presented within its lifetime pairs both sides', () async {
    // The spec's first scenario. Both stores are asserted, because "the pairing succeeded" means
    // each device recorded the *other* — a single-sided success is the failure this design exists
    // to prevent.
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    final response = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: _at(30),
    ))
        .valueOrNull!;

    final outcome = (await completePairingAtRequester(
      store: requesterStore,
      response: response,
      now: _at(30),
      undo: () async => fail('undo must not run when nothing failed'),
    ))
        .valueOrNull!;

    expect(outcome.isPaired, isTrue);
    expect(outcome, PairedOutcome(_laptop.deviceId));

    // The issuer recorded the requester...
    expect(await _believesPaired(issuerStore, _phone.deviceId), isTrue,
        reason: 'the issuer must hold an authorizing pairing with the requester');
    // ...and the requester recorded the issuer. Both, or the exchange did not complete.
    expect(await _believesPaired(requesterStore, _laptop.deviceId), isTrue,
        reason: 'the requester must hold an authorizing pairing with the issuer');

    // And each recorded the peer's *key*, not merely its name.
    final issuerKeys = (await issuerStore.verifyingKeysFromStore()).valueOrNull!;
    final requesterKeys =
        (await requesterStore.verifyingKeysFromStore()).valueOrNull!;
    expect(issuerKeys[_phone.deviceId], _key(1));
    expect(requesterKeys[_laptop.deviceId], _key(2));
  });

  test('a code presented at exactly its lifetime still pairs', () async {
    // The boundary the domain rule pins as inclusive-accept. Asserted through the flow so the
    // flow cannot introduce a stricter comparison of its own.
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));
    final boundary = _at(atMachineGrantLifetimeSeconds);

    final response = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: boundary,
    ))
        .valueOrNull!;

    final outcome = (await completePairingAtRequester(
      store: requesterStore,
      response: response,
      now: boundary,
      undo: () async => const Success(null),
    ))
        .valueOrNull!;

    expect(outcome.isPaired, isTrue);
    expect(await _believesPaired(issuerStore, _phone.deviceId), isTrue);
    expect(await _believesPaired(requesterStore, _laptop.deviceId), isTrue);
  });

  // -------------------------------------------------------------------------
  // Case 2: an expired code pairs neither
  // -------------------------------------------------------------------------

  test('an expired code pairs neither side', () async {
    // The spec's second scenario. Both stores again: an implementation that refused at the
    // requester but had already written at the issuer would pass a one-sided check.
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));
    final expired = _at(atMachineGrantLifetimeSeconds + 1);

    final response = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: expired,
    ))
        .valueOrNull!;

    expect(response, const RefusedResponse(GrantRejection.expired));

    final outcome = (await completePairingAtRequester(
      store: requesterStore,
      response: response,
      now: expired,
      undo: () async => fail('there is nothing to undo when the issuer recorded nothing'),
    ))
        .valueOrNull!;

    expect(outcome, const RefusedOutcome(GrantRejection.expired));
    expect(outcome.isPaired, isFalse);

    expect(await _believesPaired(issuerStore, _phone.deviceId), isFalse,
        reason: 'an expired code must leave the issuer with no pairing');
    expect(await _believesPaired(requesterStore, _laptop.deviceId), isFalse,
        reason: 'an expired code must leave the requester with no pairing');
    // Not merely unauthorized — nothing was written at all.
    expect((await issuerStore.listPairings()).valueOrNull, isEmpty);
    expect((await requesterStore.listPairings()).valueOrNull, isEmpty);
  });

  test('a code that was never issued is refused as unrecognised rather than expired',
      () async {
    // The distinction the alphabet decision exists to preserve. A mistyped code reporting
    // "expired" would send the user looking for a new code they already have.
    final issuer = PairingCodeIssuer();
    issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    final invented = PairingCode.parse('ABC234').valueOrNull!;
    final response = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: invented,
      now: _at(30),
    ))
        .valueOrNull!;

    expect(response, const RefusedResponse(GrantRejection.noSuchGrant));
    expect((await issuerStore.listPairings()).valueOrNull, isEmpty);
  });

  test('a code is only accepted by the device that issued it', () async {
    // One of the three properties the ~33-bit code length depends on. A second device asked about
    // a code it did not issue does not recognise it, because codes live in the issuing device's
    // memory and are never sent anywhere.
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    final otherDevice = PairingCodeIssuer();
    final response = (await acceptPairingAtIssuer(
      issuer: otherDevice,
      store: requesterStore,
      ownIdentity: _phone,
      requester: _laptop,
      code: code,
      now: _at(30),
    ))
        .valueOrNull!;

    expect(response, const RefusedResponse(GrantRejection.noSuchGrant),
        reason: 'a device that did not issue a code must not accept it');
    expect((await requesterStore.listPairings()).valueOrNull, isEmpty);
  });

  // -------------------------------------------------------------------------
  // Case 3: a code presented twice pairs only once
  // -------------------------------------------------------------------------

  test('a code presented twice pairs only once', () async {
    // Single use is what makes a code observed in transit worthless after the fact — so the second
    // presentation is refused even though it is well within the lifetime.
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    final first = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: _at(10),
    ))
        .valueOrNull!;
    expect(first, isA<AcceptedResponse>());

    // A different device presenting the same observed code, still inside the lifetime.
    final tablet = PairingIdentity(
      deviceId: DeviceId('tablet-c'),
      verifyingKey: _key(3),
    );
    final second = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: tablet,
      code: code,
      now: _at(20),
    ))
        .valueOrNull!;

    expect(second, const RefusedResponse(GrantRejection.alreadyUsed),
        reason: 'the second presentation must be refused as already used, not accepted and '
            'not reported as expired');

    // Exactly one pairing exists: the first requester's. The second device gained nothing.
    final keys = (await issuerStore.verifyingKeysFromStore()).valueOrNull!;
    expect(keys.containsKey(_phone.deviceId), isTrue);
    expect(keys.containsKey(tablet.deviceId), isFalse,
        reason: 'a code presented twice must pair only once');
    expect((await issuerStore.listPairings()).valueOrNull, hasLength(1));
  });

  test('a spent code reports already used rather than unrecognised', () async {
    // The reason a redeemed code is retained rather than dropped the instant it is used. The two
    // refusals tell the person different things.
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: _at(10),
    );

    final again = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: _at(11),
    ))
        .valueOrNull!;

    expect(again, const RefusedResponse(GrantRejection.alreadyUsed));
  });

  // -------------------------------------------------------------------------
  // Case 4: an exchange that fails partway leaves NEITHER side paired
  // -------------------------------------------------------------------------

  test('an exchange that fails at the requester leaves neither side paired', () async {
    // **The load-bearing test of this change.**
    //
    // The issuer has recorded the requester and confirmed it. The requester's own write then
    // fails. The wrong behaviour is to leave the issuer's record standing: the issuer would list
    // a peer that holds no pairing with it, the user would see an apparently healthy pairing, and
    // — in the mirror-image case — commands would be refused with no visible reason.
    //
    // **Both stores are asserted.** A version of this test that checked only the requester's
    // store would pass against an implementation that never withdrew the issuer's record, which
    // is precisely the bug this exists to catch.
    final failingRequester = _FailingWriteStore(requesterStore);
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    final response = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: _at(30),
    ))
        .valueOrNull!;

    // The issuer has written. This is the half-recorded state, mid-exchange.
    expect(await _believesPaired(issuerStore, _phone.deviceId), isTrue,
        reason: 'precondition: the issuer recorded the requester before the requester '
            'failed');

    final result = await completePairingAtRequester(
      store: failingRequester,
      response: response,
      now: _at(30),
      // The compensating round trip back to the issuer, which in the real system is a request
      // the issuer serves.
      undo: () => withdrawPairing(
        store: issuerStore,
        peer: _phone.deviceId,
        at: _at(31),
      ),
    );

    expect(result.isFailure, isTrue,
        reason: 'a failed exchange must be reported as a failure, never as a pairing');

    // NEITHER side believes it is paired.
    expect(await _believesPaired(issuerStore, _phone.deviceId), isFalse,
        reason: "the issuer's record must be withdrawn when the requester could not "
            'reciprocate: otherwise the issuer lists a peer that holds no pairing with it');
    expect(await _believesPaired(requesterStore, _laptop.deviceId), isFalse,
        reason: 'the requester must hold no pairing after its own write failed');
  });

  test('a failed exchange leaves the withdrawn record visible but unauthorizing', () async {
    // What "withdrawn" means concretely, and why it is a revocation rather than a deletion. The
    // store never deletes, so the record that an exchange was attempted survives for anyone
    // investigating, while conferring nothing.
    final failingRequester = _FailingWriteStore(requesterStore);
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    final response = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: _at(30),
    ))
        .valueOrNull!;

    await completePairingAtRequester(
      store: failingRequester,
      response: response,
      now: _at(30),
      undo: () => withdrawPairing(
        store: issuerStore,
        peer: _phone.deviceId,
        at: _at(31),
      ),
    );

    final pairings = (await issuerStore.listPairings()).valueOrNull!;
    expect(pairings, hasLength(1), reason: 'the row is retained for the record');
    expect(pairings.first.revoked, isTrue,
        reason: 'the retained row must confer no authority');
  });

  test('a failure of both the write and its withdrawal is reported in full', () async {
    // The one state that cannot be repaired from the requesting device. Reported rather than
    // flattened into either failure alone, because the user needs to know a stale record may
    // exist at the other device and can be revoked there.
    final failingRequester = _FailingWriteStore(requesterStore);
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    final response = (await acceptPairingAtIssuer(
      issuer: issuer,
      store: issuerStore,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: _at(30),
    ))
        .valueOrNull!;

    final result = await completePairingAtRequester(
      store: failingRequester,
      response: response,
      now: _at(30),
      undo: () async => Result.failure(
        const StorageError(message: 'the other device is unreachable'),
      ),
    );

    final error = result.errorOrNull! as StorageError;
    expect(error.message, contains('revoke it there'),
        reason: 'the message must tell the user where the stale record is and what to do');
    expect(error.message, contains('the other device is unreachable'),
        reason: 'the withdrawal failure must be named, not swallowed');
  });

  test('a refusal never runs the withdrawal', () async {
    // A refusal already leaves both sides with nothing, so compensating would be a revocation of
    // a pairing that does not exist — which the store reports as a failure, turning a clean
    // refusal into one the user cannot act on.
    final outcome = (await completePairingAtRequester(
      store: requesterStore,
      response: const RefusedResponse(GrantRejection.noSuchGrant),
      now: _at(30),
      undo: () async => fail('withdrawal must not run for a refusal'),
    ))
        .valueOrNull!;

    expect(outcome, const RefusedOutcome(GrantRejection.noSuchGrant));
    expect((await requesterStore.listPairings()).valueOrNull, isEmpty);
  });

  test('an issuer whose own write fails never confirms to the requester', () async {
    // The mirror-image direction. If the issuer cannot record, it must not return an identity —
    // because the identity *is* the confirmation, and a requester that received one would record
    // a peer holding no pairing with it.
    final failingIssuer = _FailingWriteStore(issuerStore);
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    final result = await acceptPairingAtIssuer(
      issuer: issuer,
      store: failingIssuer,
      ownIdentity: _laptop,
      requester: _phone,
      code: code,
      now: _at(30),
    );

    expect(result.isFailure, isTrue,
        reason: 'an issuer that could not record must report a failure rather than confirming');

    // And so nothing reaches the requester's store either.
    expect(await _believesPaired(issuerStore, _phone.deviceId), isFalse);
    expect(await _believesPaired(requesterStore, _laptop.deviceId), isFalse);
  });

  // -------------------------------------------------------------------------
  // Housekeeping and reuse
  // -------------------------------------------------------------------------

  test('the sweep forgets codes long past their lifetime and keeps recent ones', () {
    // Housekeeping, but it must not sweep so eagerly that alreadyUsed becomes noSuchGrant while
    // the user is still looking at the screen.
    final issuer = PairingCodeIssuer();
    issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));
    expect(issuer.outstandingCount, 1);

    issuer.forgetStale(_at(atMachineGrantLifetimeSeconds + 1));
    expect(issuer.outstandingCount, 1,
        reason: 'a just-expired code must stay reportable');

    issuer.forgetStale(_at(atMachineGrantLifetimeSeconds * 2 + 1));
    expect(issuer.outstandingCount, 0, reason: 'a long-stale code must be forgotten');
  });

  test('the flow reports the same rejections the domain rule decides', () async {
    // Expiry and single use are not reimplemented here: the flow asks `evaluateGrant`. This
    // asserts the flow's verdicts agree with the rule's for the same inputs, so a divergence
    // shows up as a test failure rather than as a behaviour difference between languages.
    final issuer = PairingCodeIssuer();
    final code = issuer.issue(delivery: GrantDelivery.atMachine, now: _at(0));

    // Inside the lifetime the rule says valid, and the flow accepts.
    expect(issuer.redeem(code: code, now: _at(10)), const ValidGrant());
    // Having accepted, the rule now says alreadyUsed, and so does the flow.
    expect(issuer.redeem(code: code, now: _at(11)),
        const InvalidGrant(GrantRejection.alreadyUsed));
  });

  test('a refusal carries the domain own prose', () {
    // The surface shows what the domain already wrote, so the two cannot disagree about what an
    // expired code means.
    const outcome = RefusedOutcome(GrantRejection.expired);
    expect(outcome.userMessage, GrantRejection.expired.userMessage);
  });

  test('a seeded generator still only emits alphabet glyphs', () {
    // The generator's alphabet handling, isolated from the entropy source. `Random.secure` is
    // what production uses — this only proves the fold is correct.
    final code = generatePairingCode(random: Random(1));
    expect(code.value.length, pairingCodeLength);
    for (final glyph in code.value.split('')) {
      expect(pairingCodeAlphabet.contains(glyph), isTrue);
    }
  });
}
