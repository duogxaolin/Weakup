// `prefer_initializing_formals` is suppressed here, and the reason is not style. Taking its
// advice means writing `A({required this._gateway})`, which needs the `private-named-parameters`
// language feature and a minimum SDK of 3.12. This package targets lower, and the analyzer
// rejects the suggested form outright with `experiment_not_enabled` — verified rather than
// assumed. Even once available, a private named parameter cannot be supplied from another
// library, so the constructor would become uncallable from both the app and its tests. The lint
// mis-models a constructor that takes public arguments and stores them privately, which is what
// keeps these two collaborators injectable for testing while unreachable from outside.
// ignore_for_file: prefer_initializing_formals

/// Account identity through Google, delegated to the platform's own sign-in flow.
///
/// This library implements [AuthProvider] and nothing else. It answers "which account is this
/// device acting as", and it is deliberately incapable of answering anything about authority —
/// see the account-identity capability, and the source-text test in
/// `test/platform/google_auth_source_test.dart` that fails the build if account identity ever
/// reaches the command decision.
///
/// Mirrors `desktop/src-tauri/src/platform/google_auth.rs`.
///
/// # No password, anywhere
///
/// There is no password field, no password parameter, and no password-based sign-in path. The
/// credential exchange happens with Google, in Google's own surface — the Android account
/// picker or the iOS Safari view controller that `google_sign_in` presents — and this system
/// receives only the resulting tokens.
///
/// A password this system never receives is one it cannot leak, cannot mishandle in a log, and
/// cannot be compelled to reset. The provider's flow also carries the provider's own
/// protections, including suspicious-login checks and whatever second factor the user has
/// already configured, none of which this system could reproduce.
///
/// The desktop takes the same position for the same reason and reaches it differently: it opens
/// the system browser rather than an embedded web view (design D3), because on desktop an
/// in-app web view is what phishing looks like. On mobile the platform's account picker *is*
/// the system surface, so `google_sign_in` is the equivalent choice rather than a departure
/// from it.
///
/// # Account identity is not authority
///
/// Being signed in says which devices a user may *see*. Which they may *command* is settled by
/// pairing, and the command-acceptance rules take no account input at all. Keeping the two
/// apart in the code is what keeps them apart in fact: one component answering both invites an
/// implementation in which a valid session becomes authority, which is the failure this system
/// exists to prevent.
///
/// # What is UNVERIFIED here
///
/// Sign-in has never been completed on a real handset from this repository, and cannot be: it
/// needs an OAuth client that does not yet exist for this project, a device, and a person. What
/// the tests cover is every decision this library makes on its own — where the token is put,
/// what sign-out clears and what it leaves alone, and the absence of any password path. The
/// round trip to Google is not covered and is not claimed.
library;

import 'dart:convert';

import 'package:google_sign_in/google_sign_in.dart';

import '../application/transport.dart';
import '../core/app_error.dart';
import '../core/result.dart';
import 'secret_store.dart';

/// The name the long-lived token is filed under in the platform credential store.
///
/// Mirrors `REFRESH_TOKEN_SECRET_NAME` in the Rust implementation. The token goes here and
/// **nowhere else** — not the database, not shared preferences, not a log line.
/// `secret_store.dart` is the single seam for this, which is what lets the decision be tested
/// against an in-memory fake rather than a real keystore.
const String refreshTokenSecretName = 'google-account-refresh-token';

/// The account identifier is cached alongside the token so a restart knows which account is
/// signed in without a network round trip. It identifies; it authorizes nothing.
const String accountIdSecretName = 'google-account-id';

/// What is asked for. Identity only.
///
/// A scope that is not requested cannot be misused later by code that finds a token lying
/// around with it already granted. This library establishes *which account*, and has no reason
/// to read mail, contacts, or files.
const List<String> identityScopes = <String>['email'];

/// The sign-in surface, as this library needs it.
///
/// An interface rather than a direct call into `GoogleSignIn.instance`, because the plugin
/// requires a real platform channel and a real device: every decision *around* sign-in — where
/// the token goes, what sign-out clears, what happens when the store is unreachable — would
/// otherwise be untestable without a handset. The same seam reasoning as `SecretStore`.
///
/// Note what is absent: no method takes a password, and none could be added without also adding
/// it to the provider's own surface, which is Google's and not this repository's.
abstract interface class GoogleSignInGateway {
  /// Prepares the platform sign-in machinery. Must complete before anything else is called.
  Future<void> initialize();

  /// Presents the provider's own account picker and returns the account chosen.
  ///
  /// Returns `null` when the user declines. Declining is an outcome to be reported, not an
  /// error to be retried.
  Future<GoogleAccountSession?> authenticate();

  /// Re-establishes a previous session without presenting any UI, if the platform can.
  Future<GoogleAccountSession?> attemptSilent();

  /// Forgets the session at the provider.
  Future<void> signOut();
}

/// What the provider returned about the signed-in account.
///
/// Deliberately thin: an identifier, an email for display, and the long-lived token. Nothing
/// here is consulted by any decision about whether a command is obeyed.
final class GoogleAccountSession {
  const GoogleAccountSession({
    required this.accountId,
    required this.email,
    required this.token,
  });

  /// The provider's stable identifier for this account.
  final String accountId;

  /// Shown to the user so they can tell which account they are signed in to. Display only.
  final String email;

  /// The long-lived credential, destined for the platform credential store and nowhere else.
  final String token;
}

/// The real gateway, backed by `google_sign_in`.
///
/// # UNVERIFIED on Android and iOS
///
/// Written against the package's 7.x API and never run on a handset. `authenticate` is
/// unsupported on some platforms and throws there rather than returning `null`; that is
/// translated below, but which platforms do which has not been observed from here.
///
/// The plugin also requires an OAuth client that does not yet exist for this project. On
/// Android the client is matched by package name and signing certificate through
/// `google-services.json` rather than passed in code; on iOS it comes from
/// `GoogleService-Info.plist`. Neither file currently contains an OAuth client, so
/// [initialize] will fail on a real device until one is created — see the desktop module's
/// `missing_client_id_error` for what to create and where.
final class GoogleSignInPluginGateway implements GoogleSignInGateway {
  GoogleSignInPluginGateway([GoogleSignIn? signIn])
      : _signIn = signIn ?? GoogleSignIn.instance;

  final GoogleSignIn _signIn;

  @override
  Future<void> initialize() => _signIn.initialize();

  @override
  Future<GoogleAccountSession?> authenticate() async {
    if (!_signIn.supportsAuthenticate()) {
      // Reported rather than silently returning null: "this platform cannot present a sign-in"
      // and "the user declined" call for different words, and only one of them is worth
      // offering a retry for.
      throw UnsupportedError(
        'this platform does not support presenting the Google account picker',
      );
    }
    final account = await _signIn.authenticate(scopeHint: identityScopes);
    return _sessionFrom(account);
  }

  @override
  Future<GoogleAccountSession?> attemptSilent() async {
    final attempt = _signIn.attemptLightweightAuthentication();
    if (attempt == null) {
      return null;
    }
    final account = await attempt;
    return account == null ? null : _sessionFrom(account);
  }

  @override
  Future<void> signOut() => _signIn.signOut();

  /// Reads only the fields this system depends on, and requires each explicitly.
  ///
  /// A missing id token is an error rather than a default. The same rule design D8 states for
  /// the wire types applies here for the same reason: a silent default would produce a session
  /// that looks established and can never be refreshed.
  GoogleAccountSession _sessionFrom(GoogleSignInAccount account) {
    final idToken = account.authentication.idToken;
    if (idToken == null) {
      throw StateError('the provider returned no id token for the signed-in account');
    }
    return GoogleAccountSession(
      accountId: account.id,
      email: account.email,
      token: idToken,
    );
  }
}

/// [AuthProvider] backed by Google, with the long-lived token in the platform credential store.
///
/// # What this type deliberately cannot do
///
/// It holds no pairing store, takes none, and has no method that touches one. Signing out
/// clears the tokens this type owns and nothing else, which is what makes the capability's
/// "sign-out leaves pairings intact" scenario true by construction rather than by remembering
/// to be careful — there is no code path from here to a pairing.
final class GoogleAuthProvider implements AuthProvider {
  GoogleAuthProvider({
    required GoogleSignInGateway gateway,
    required SecretStore secrets,
  })  : _gateway = gateway,
        _secrets = secrets;

  final GoogleSignInGateway _gateway;
  final SecretStore _secrets;

  /// The account as last established, cached in memory so [currentAccount] does not consult
  /// the credential store on every call.
  AccountId? _cachedAccount;

  @override
  Future<AccountId?> currentAccount() async => _cachedAccount;

  /// Presents the provider's sign-in surface and records the resulting session.
  ///
  /// Returns `null` on the success path when the user declined — an outcome, not a failure.
  Future<Result<AccountId?>> signIn() async {
    final GoogleAccountSession? session;
    try {
      await _gateway.initialize();
      session = await _gateway.authenticate();
    } on Object catch (error) {
      return Failure(ValidationError(message: 'Google sign-in did not complete: $error'));
    }

    if (session == null) {
      return const Success(null);
    }

    return _record(session);
  }

  /// Re-establishes a previous session without presenting anything to the user.
  Future<Result<AccountId?>> restoreSession() async {
    // The credential store is the authority on whether a session exists here, because it is
    // what holds the token that makes the session usable. The provider is consulted only when
    // the store has nothing.
    final stored = await storedToken();
    switch (stored) {
      case Failure(:final error):
        // An unreachable store is *not* a signed-out device. Reporting it as one would tell the
        // user their session was gone while it sits intact behind a locked keystore, and would
        // invite a sign-in that overwrites it.
        return Failure(error);
      case Success(value: final token):
        if (token != null) {
          final account = await _storedAccountId();
          switch (account) {
            case Failure(:final error):
              return Failure(error);
            case Success(value: final id):
              if (id != null) {
                _cachedAccount = AccountId(id);
                return Success(_cachedAccount);
              }
          }
        }
    }

    final GoogleAccountSession? session;
    try {
      await _gateway.initialize();
      session = await _gateway.attemptSilent();
    } on Object catch (error) {
      return Failure(ValidationError(message: 'the previous session could not be restored: $error'));
    }

    if (session == null) {
      _cachedAccount = null;
      return const Success(null);
    }

    return _record(session);
  }

  /// Forgets the account session.
  ///
  /// Removes the token and the cached account identifier, and **touches nothing else**.
  /// Pairings are established by physical possession of both devices and recorded locally; the
  /// identity provider was never granted the authority to withdraw them, and a sign-out that
  /// revoked them would mean an expired token silently de-authorized a machine the user is
  /// standing in front of.
  Future<Result<void>> signOut() async {
    try {
      await _gateway.signOut();
    } on Object catch (error) {
      return Failure(ValidationError(message: 'the provider sign-out failed: $error'));
    }

    final removedToken = await _secrets.delete(refreshTokenSecretName);
    if (removedToken case Failure(:final error)) {
      // Reported rather than swallowed: a sign-out that claimed success while the token was
      // still in the keystore would leave the user believing they had signed out of a device
      // that is still signed in.
      return Failure(error);
    }

    final removedAccount = await _secrets.delete(accountIdSecretName);
    if (removedAccount case Failure(:final error)) {
      return Failure(error);
    }

    _cachedAccount = null;
    return const Success(null);
  }

  /// The stored long-lived token, used to obtain a fresh one.
  ///
  /// Succeeds with `null` when there is genuinely no session, and fails when the store could
  /// not be consulted — the distinction `secret_store.dart` exists to preserve.
  Future<Result<String?>> storedToken() async {
    final lookup = await _secrets.get(refreshTokenSecretName);
    return lookup.map((found) => switch (found) {
          SecretFound(:final secret) => utf8.decode(secret),
          SecretNotFound() => null,
        });
  }

  Future<Result<String?>> _storedAccountId() async {
    final lookup = await _secrets.get(accountIdSecretName);
    return lookup.map((found) => switch (found) {
          SecretFound(:final secret) => utf8.decode(secret),
          SecretNotFound() => null,
        });
  }

  /// Writes a completed session to the credential store, token first.
  Future<Result<AccountId?>> _record(GoogleAccountSession session) async {
    final wroteToken = await _secrets.set(
      refreshTokenSecretName,
      utf8.encode(session.token),
    );
    if (wroteToken case Failure(:final error)) {
      return Failure(error);
    }

    final wroteAccount = await _secrets.set(
      accountIdSecretName,
      utf8.encode(session.accountId),
    );
    if (wroteAccount case Failure(:final error)) {
      return Failure(error);
    }

    _cachedAccount = AccountId(session.accountId);
    return Success(_cachedAccount);
  }
}
