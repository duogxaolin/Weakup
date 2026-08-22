/// The structural and behavioural guarantees for account identity on mobile.
///
/// The Dart mirror of `desktop/src-tauri/src/platform/google_auth_tests.rs`, covering the same
/// two claims: account identity is absent from the command decision (task 2.4/3.3), and signing
/// out leaves pairings intact (task 2.5/3.3). It additionally enforces the spec's "no password
/// is accepted anywhere" scenario over the whole sign-in surface.
///
/// # Why this file is separate from the library it guards
///
/// Some of the tests below grep `lib/platform/google_auth.dart` and `lib/domain/
/// command_acceptance.dart` for text that must not appear. Put inside the library, or in a test
/// that also declared those names in its own strings, the test would match itself. That trap has
/// bitten this codebase before, which is why `device_identity_source_test.dart` and
/// `transport_source_test.dart` are also separate.
///
/// # What these tests can and cannot establish
///
/// They do **not** establish that sign-in works. Completing OAuth needs an OAuth client that does
/// not yet exist for this project, a handset, and a person; none of the three is available to a
/// test run. That path is UNVERIFIED and is named as such rather than implied by a green suite.
library;

import 'dart:convert';
import 'dart:io';

import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/application/transport.dart';
import 'package:weakup/core/core.dart';
import 'package:weakup/data/app_database.dart';
import 'package:weakup/data/pairing_store.dart';
import 'package:weakup/domain/domain.dart';
import 'package:weakup/platform/google_auth.dart';
import 'package:weakup/platform/secret_store.dart';

import '../domain/test_signing.dart';

/// A gateway that returns a fixed session, recording what it was asked to do.
///
/// Note what it cannot be asked: there is no password parameter anywhere on the interface, and
/// none could be added without also adding one to Google's own surface.
final class _FakeGateway implements GoogleSignInGateway {
  _FakeGateway({this.session, this.throwOnAuthenticate = false});

  GoogleAccountSession? session;
  bool throwOnAuthenticate;

  int initializeCalls = 0;
  int authenticateCalls = 0;
  int signOutCalls = 0;

  @override
  Future<void> initialize() async => initializeCalls++;

  @override
  Future<GoogleAccountSession?> authenticate() async {
    authenticateCalls++;
    if (throwOnAuthenticate) {
      throw StateError('the account picker could not be presented');
    }
    return session;
  }

  @override
  Future<GoogleAccountSession?> attemptSilent() async => session;

  @override
  Future<void> signOut() async => signOutCalls++;
}

const _session = GoogleAccountSession(
  accountId: 'account-1',
  email: 'someone@example.com',
  token: 'refresh-token-value',
);

GoogleAuthProvider _provider(SecretStore secrets, GoogleSignInGateway gateway) =>
    GoogleAuthProvider(gateway: gateway, secrets: secrets);

void main() {
  // ---------------------------------------------------------------------------
  // Task 3.3 mirroring 2.4 — account identity is absent from the command decision
  // ---------------------------------------------------------------------------

  group('account identity is not an input to the command decision', () {
    final acceptanceSource =
        File('lib/domain/command_acceptance.dart').readAsStringSync();

    test('the command-acceptance rules name no account, session, or sign-in state', () {
      // The account-identity capability's load-bearing claim: an account session confers no
      // authority to shut down a machine. Only a pairing does.
      //
      // Asserted on the source of the decision rather than by exercising it, because the failure
      // is additive. Someone adds an `accountId` to `CommandTargetState` meaning only to improve
      // a log line; every existing test still passes, the next person reads the field as
      // something the decision may consult, and same-account access becomes authority — the
      // exact failure this system is built to prevent.
      for (final forbidden in [
        'AccountId',
        'accountId',
        'sessionToken',
        'signedIn',
        'signIn',
        'AuthProvider',
        'authProvider',
      ]) {
        final offending = acceptanceSource
            .split('\n')
            .where((line) {
              final trimmed = line.trimLeft();
              // The rules' own prose explains that account identity is *not* consulted. That
              // explanation is the guarantee being documented, not a breach of it.
              return !trimmed.startsWith('//') && !trimmed.startsWith('///');
            })
            .where((line) => line.contains(forbidden))
            .toList();

        expect(
          offending,
          isEmpty,
          reason: 'the command-acceptance rules reference "$forbidden". Account identity '
              'answers which devices a user may *see*; pairing answers which they may '
              '*command*. A session reaching this decision would make possession of a valid '
              'account session sufficient to shut down a machine.',
        );
      }
    });

    test('a signed-in but unpaired device is refused', () async {
      // The same claim as behaviour rather than a grep, and the spec's own scenario.
      // `CommandTargetState` is constructed exhaustively here: a field carrying account
      // identity would break this at compile time, which is a second independent way for the
      // guarantee to fail loudly.
      final secrets = InMemorySecretStore();
      final provider = _provider(secrets, _FakeGateway(session: _session));
      await provider.signIn();
      expect(
        await provider.currentAccount(),
        isNotNull,
        reason: 'the sender holds a valid account session',
      );

      final sender = DeviceId('phone-a');
      final keys = await TestKeyPair.fromSeed(1);
      final createdAt = DateTime.utc(2026, 8, 7, 12);
      final envelope = CommandEnvelope(
        sender: sender,
        command: RemoteCommand.powerOff,
        createdAt: createdAt,
        nonce: 'nonce-1',
        signature: await keys.signCommand(
          sender: sender,
          command: RemoteCommand.powerOff.wireName,
          createdAt: createdAt,
          nonce: 'nonce-1',
        ),
      );

      final outcome = await evaluateCommand(
        envelope: envelope,
        targetState: const CommandTargetState(
          seenNonces: {},
          // Empty: no pairing exists with the sender, whatever account it is signed in to.
          verifyingKeys: {},
          targetCanPowerOff: true,
          targetIsRemoteTarget: true,
          isPaired: false,
          remoteControlEnabled: true,
        ),
        now: createdAt,
      );

      expect(
        outcome,
        const RejectedCommand(RejectionReason.authenticityUnverified),
        reason: 'a device with a valid account session but no pairing is refused; the session '
            'is not an input to this decision and cannot make it come out any other way',
      );
    });
  });

  // ---------------------------------------------------------------------------
  // Task 3.3 — the spec forbids a password anywhere in the sign-in surface
  // ---------------------------------------------------------------------------

  group('no password is accepted anywhere', () {
    test('the sign-in surface declares no password field, parameter, or flow', () {
      // The capability's "no password is accepted anywhere" scenario. A password this system
      // never receives is one it cannot leak, cannot mishandle in a log, and cannot be
      // compelled to reset.
      //
      // Checked across every file that could plausibly present a sign-in, not only the auth
      // library: the failure this guards against is a password field appearing on a screen,
      // which would live in the UI rather than here.
      final surfaces = <File>[
        File('lib/platform/google_auth.dart'),
        ...Directory('lib/ui')
            .listSync(recursive: true)
            .whereType<File>()
            .where((file) => file.path.endsWith('.dart')),
      ];

      for (final file in surfaces) {
        final source = file.readAsStringSync();
        for (final forbidden in [
          'password',
          'passphrase',
          'obscureText',
          'signInWithEmailAndPassword',
          'createUserWithEmailAndPassword',
        ]) {
          final offending = source
              .split('\n')
              .where((line) {
                final trimmed = line.trimLeft();
                // Prose explaining that no password is handled is the guarantee being
                // documented, not a breach of it.
                return !trimmed.startsWith('//') && !trimmed.startsWith('///');
              })
              .where((line) => line.toLowerCase().contains(forbidden.toLowerCase()))
              .toList();

          expect(
            offending,
            isEmpty,
            reason: '${file.path} contains "$forbidden". Sign-in is delegated to Google '
                'entirely; this system accepts, transmits, and stores no password, and offers '
                'no password-based path.',
          );
        }
      }
    });

    test('the gateway interface has no member that could carry a password', () {
      // Structural rather than textual: a password parameter would have to be *added* to this
      // interface, and every existing test would still pass.
      final source = File('lib/platform/google_auth.dart').readAsStringSync();

      expect(source.contains('abstract interface class GoogleSignInGateway'), isTrue);
      for (final forbidden in ['String password', 'required password', 'credentials(']) {
        expect(source.contains(forbidden), isFalse, reason: 'found "$forbidden"');
      }
    });
  });

  // ---------------------------------------------------------------------------
  // The token goes to the credential store and nowhere else
  // ---------------------------------------------------------------------------

  group('the long-lived token is kept in the platform credential store', () {
    test('a completed sign-in writes the token to the secret store', () async {
      final secrets = InMemorySecretStore();
      final provider = _provider(secrets, _FakeGateway(session: _session));

      final result = await provider.signIn();

      expect(result.valueOrNull, const AccountId('account-1'));
      final stored = await secrets.get(refreshTokenSecretName);
      expect(
        (stored.valueOrNull as SecretFound).secret,
        utf8.encode('refresh-token-value'),
      );
    });

    test('the library reaches the credential store and never the database or a file', () {
      // A token written to the database would sit in a file the user can read, be included in a
      // backup, and outlive a sign-out that only cleared the keystore.
      final source = File('lib/platform/google_auth.dart').readAsStringSync();

      for (final forbidden in [
        'AppDatabase',
        'drift',
        'SharedPreferences',
        'File(',
        'writeAsString',
      ]) {
        final offending = source
            .split('\n')
            .where((line) {
              final trimmed = line.trimLeft();
              return !trimmed.startsWith('//') && !trimmed.startsWith('///');
            })
            .where((line) => line.contains(forbidden))
            .toList();

        expect(
          offending,
          isEmpty,
          reason: 'the account-identity library reaches "$forbidden". The token goes to the '
              'platform credential store through `secret_store.dart` and nowhere else.',
        );
      }
    });

    test('the token is never written to a log', () {
      final source = File('lib/platform/google_auth.dart').readAsStringSync();

      for (final forbidden in ['print(', 'debugPrint', 'log(']) {
        final offending = source
            .split('\n')
            .where((line) {
              final trimmed = line.trimLeft();
              return !trimmed.startsWith('//') && !trimmed.startsWith('///');
            })
            .where((line) => line.contains(forbidden))
            .toList();

        expect(offending, isEmpty, reason: 'found "$forbidden"');
      }
    });

    test('an unreachable store is a failure rather than a signed-out device', () async {
      // The distinction `secret_store.dart` exists to preserve. Reporting a locked keystore as
      // "signed out" would tell the user their session was gone when it is intact behind a
      // temporary failure — and would invite a sign-in that overwrites it.
      final secrets = InMemorySecretStore();
      await secrets.set(refreshTokenSecretName, utf8.encode('refresh-token-value'));
      secrets.failWith('the keystore is locked');
      final provider = _provider(secrets, _FakeGateway(session: _session));

      final result = await provider.storedToken();

      expect(result.isFailure, isTrue);
    });

    test('a device that has never signed in reports no account', () async {
      final provider = _provider(InMemorySecretStore(), _FakeGateway());

      expect(await provider.currentAccount(), isNull);
      expect((await provider.storedToken()).valueOrNull, isNull);
    });

    test('a declined sign-in is an outcome rather than a failure', () async {
      // The user pressing Cancel is not an error to be retried, and must not be shown as one.
      final provider = _provider(InMemorySecretStore(), _FakeGateway(session: null));

      final result = await provider.signIn();

      expect(result.isSuccess, isTrue);
      expect(result.valueOrNull, isNull);
    });

    test('a gateway that throws is reported as a failure and not a panic', () async {
      final provider = _provider(
        InMemorySecretStore(),
        _FakeGateway(throwOnAuthenticate: true),
      );

      final result = await provider.signIn();

      expect(result.isFailure, isTrue);
      expect(result.errorOrNull, isA<ValidationError>());
    });
  });

  // ---------------------------------------------------------------------------
  // Task 3.3 mirroring 2.5 — refresh, and sign-out leaving pairings intact
  // ---------------------------------------------------------------------------

  group('a session survives a restart', () {
    test('a recorded session is restored from the credential store', () async {
      // Token refresh depends on the token outliving the process that obtained it — that is the
      // whole reason it is in the credential store rather than in memory. Modelled by building a
      // second provider over the same store, which is what a restart is.
      final secrets = InMemorySecretStore();
      await _provider(secrets, _FakeGateway(session: _session)).signIn();

      // The gateway returns nothing, so anything restored came from the store rather than from
      // the provider being asked again.
      final afterRestart = _provider(secrets, _FakeGateway(session: null));
      expect(
        await afterRestart.currentAccount(),
        isNull,
        reason: 'a fresh process knows nothing until it consults the store',
      );

      final restored = await afterRestart.restoreSession();

      expect(restored.valueOrNull, const AccountId('account-1'));
      expect((await afterRestart.storedToken()).valueOrNull, 'refresh-token-value');
      expect(await afterRestart.currentAccount(), const AccountId('account-1'));
    });

    test('a session with an account but no token is not a session', () async {
      // A half-written pair would otherwise report an account that can never be refreshed, which
      // the user sees as being signed in to something that never works.
      final secrets = InMemorySecretStore();
      await secrets.set(accountIdSecretName, utf8.encode('account-1'));
      final provider = _provider(secrets, _FakeGateway(session: null));

      final restored = await provider.restoreSession();

      expect(restored.valueOrNull, isNull);
      expect(await provider.currentAccount(), isNull);
    });

    test('signing in again replaces the stored token rather than accumulating one', () async {
      final secrets = InMemorySecretStore();
      final gateway = _FakeGateway(session: _session);
      final provider = _provider(secrets, gateway);

      await provider.signIn();
      gateway.session = const GoogleAccountSession(
        accountId: 'account-1',
        email: 'someone@example.com',
        token: 'second-token',
      );
      await provider.signIn();

      expect((await provider.storedToken()).valueOrNull, 'second-token');
    });
  });

  group('sign-out', () {
    late AppDatabase db;
    late PairingStore pairings;

    setUp(() {
      db = AppDatabase(NativeDatabase.memory());
      pairings = PairingStore(db);
    });

    tearDown(() async => db.close());

    test('signing out clears the token and the account', () async {
      final secrets = InMemorySecretStore();
      final gateway = _FakeGateway(session: _session);
      final provider = _provider(secrets, gateway);
      await provider.signIn();

      final result = await provider.signOut();

      expect(result.isSuccess, isTrue);
      expect(gateway.signOutCalls, 1);
      expect(await provider.currentAccount(), isNull);
      expect((await provider.storedToken()).valueOrNull, isNull);
      expect((await secrets.get(accountIdSecretName)).valueOrNull, isA<SecretNotFound>());
    });

    test('signing out leaves every pairing recorded', () async {
      // The capability's "sign-out leaves pairings intact" scenario, asserted against the real
      // pairing store rather than a fake. Pairings are established by physical possession of both
      // devices and recorded locally; the identity provider was never granted the authority to
      // withdraw them. A sign-out that revoked them would mean an expired token silently
      // de-authorized a machine the user is standing in front of.
      final peer = DeviceId('desktop-a');
      final key = (await TestKeyPair.fromSeed(3)).verifyingKey;
      await pairings.recordPairing(
        peer: peer,
        verifyingKey: key,
        pairedAt: DateTime.utc(2026, 8, 1, 9),
      );

      final provider = _provider(InMemorySecretStore(), _FakeGateway(session: _session));
      await provider.signIn();

      await provider.signOut();

      final after = (await pairings.listPairings()).valueOrNull!;
      expect(after, hasLength(1), reason: 'the pairing is still recorded');
      expect(after.single.revoked, isFalse, reason: 'and it is still active');
      expect(
        (await pairings.verifyingKeysFromStore()).valueOrNull,
        hasLength(1),
        reason: 'the key commands are checked against is still held',
      );
    });

    test('signing back in does not require re-pairing', () async {
      // The half a user would actually notice: sign out, sign in again, and the desktop is still
      // commandable without walking back to it.
      final peer = DeviceId('desktop-a');
      final key = (await TestKeyPair.fromSeed(3)).verifyingKey;
      await pairings.recordPairing(
        peer: peer,
        verifyingKey: key,
        pairedAt: DateTime.utc(2026, 8, 1, 9),
      );

      final provider = _provider(InMemorySecretStore(), _FakeGateway(session: _session));
      await provider.signIn();
      await provider.signOut();
      await provider.signIn();

      final keys = (await pairings.verifyingKeysFromStore()).valueOrNull!;
      expect(
        keys[peer],
        key,
        reason: 'the pairing that existed before the sign-out is the one still in force; '
            'signing out and back in required no new pairing',
      );
      expect(keys, hasLength(1));
    });

    test('the account provider holds no path to a pairing store', () {
      // Why the scenario above is true by construction rather than by remembering to be careful:
      // there is no code path from this library to a pairing at all.
      final source = File('lib/platform/google_auth.dart').readAsStringSync();

      for (final forbidden in ['PairingStore', 'pairing', 'revoke']) {
        final offending = source
            .split('\n')
            .where((line) {
              final trimmed = line.trimLeft();
              return !trimmed.startsWith('//') && !trimmed.startsWith('///');
            })
            .where((line) => line.toLowerCase().contains(forbidden.toLowerCase()))
            .toList();

        expect(
          offending,
          isEmpty,
          reason: 'the account-identity library references "$forbidden". The account and the '
              'pairing are separate authorities: an account session confers no ability to '
              'command, and losing one must not withdraw the other.',
        );
      }
    });

    test('a sign-out on an unreachable store is reported rather than silently succeeding',
        () async {
      // A sign-out that reported success while the token was still in the keystore would leave
      // the user believing they had signed out of a device that is still signed in.
      final secrets = InMemorySecretStore();
      final provider = _provider(secrets, _FakeGateway(session: _session));
      await provider.signIn();
      secrets.failWith('the keystore is locked');

      final result = await provider.signOut();

      expect(result.isFailure, isTrue);
    });
  });
}
