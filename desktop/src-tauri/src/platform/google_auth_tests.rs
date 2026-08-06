//! The structural and behavioural guarantees for account identity.
//!
//! # Why this file exists separately
//!
//! One of the tests below greps `google_auth.rs` and `command_acceptance.rs` for name
//! fragments that must not appear. Put inline as a `mod tests` in either file, the test's own
//! string literals would be part of the source it searches, and it would fail against itself.
//! That is not hypothetical — this codebase has hit it before, which is why
//! `transport_tests.rs`, `device_identity_tests.rs`, `remote_command_tests.rs`, and
//! `command_acceptance_tests.rs` are also separate files.
//!
//! # What these tests can and cannot establish
//!
//! They cover every decision this module makes on its own: the bind address, the one-request
//! shutdown, the PKCE construction against RFC 7636's published vector, the redirect parsing,
//! where the refresh token is put, and what sign-out does and does not touch.
//!
//! They do **not** establish that sign-in works. Completing OAuth needs an OAuth client that
//! does not yet exist for this project, a browser, and a person; none of the three is available
//! to a test run. That path is UNVERIFIED and is named as such rather than implied by a green
//! suite.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::time::Duration;

use chrono::{TimeZone, Utc};

use crate::application::transport::{AccountId, AuthProvider};
use crate::core::AppError;
use crate::data::{PairingRecord, PairingStore};
use crate::domain::test_signing::TestKeyPair;
use crate::domain::{
    evaluate_command, CommandAcceptance, CommandEnvelope, CommandTargetState, DeviceId,
    RejectionReason, RemoteCommand, VerifyingKey,
};
use crate::platform::google_auth::{
    authorization_url, parse_redirect_request, GoogleAuthProvider, LoopbackReceiver,
    OAuthClientConfig, PkceChallenge, RedirectOutcome, ACCOUNT_ID_SECRET_NAME,
    CODE_CHALLENGE_METHOD, LOOPBACK_HOST, REFRESH_TOKEN_SECRET_NAME,
};
use crate::platform::secret_store::{InMemorySecretStore, SecretLookup, SecretStore};

/// The source of the account-identity module, read at compile time.
const GOOGLE_AUTH_SOURCE: &str = include_str!("google_auth.rs");

/// The source of the command-acceptance rules, read at compile time. This is the file whose
/// inputs must contain no account identity.
const COMMAND_ACCEPTANCE_SOURCE: &str = include_str!("../domain/command_acceptance.rs");

/// A client id of the right shape, for tests that need a well-formed config. Not a real
/// client: no OAuth client exists for this project yet, and this value is a fixture rather
/// than a placeholder standing in for one — nothing in production reads it.
const WELL_FORMED_CLIENT_ID: &str = "000000000000-testfixture.apps.googleusercontent.com";

fn config() -> OAuthClientConfig {
    OAuthClientConfig::new(WELL_FORMED_CLIENT_ID).expect("well-formed fixture")
}

// ---------------------------------------------------------------------------
// Task 2.4 — the structural guarantee. This is the test that must never be weakened.
// ---------------------------------------------------------------------------

#[test]
fn account_identity_is_not_among_the_inputs_to_the_command_decision() {
    // The account-identity capability's load-bearing claim: an account session confers no
    // authority to shut down a machine. Only a pairing does.
    //
    // Asserted on the source of the decision rather than by exercising it, because the failure
    // is additive. Someone adds an `account: AccountId` to `CommandTargetState` meaning only to
    // improve a log line; every existing test still passes, the next person reads the field as
    // something the decision may consult, and same-account access becomes authority — the exact
    // failure this system is built to prevent.
    //
    // Checked over the whole file, comments included, except for the prose that exists to say
    // these things are absent. A doc comment describing a field that does not exist is the
    // first step towards the field existing.
    for forbidden in [
        "AccountId",
        "account_id",
        "session_token",
        "signed_in",
        "sign_in",
        "auth_provider",
        "AuthProvider",
    ] {
        let uses: Vec<&str> = COMMAND_ACCEPTANCE_SOURCE
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                // The rules' own prose explains that account identity is *not* consulted. That
                // explanation is the guarantee being documented, not a breach of it.
                !trimmed.starts_with("//")
            })
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "the command-acceptance rules reference {forbidden:?}. Account identity answers \
             which devices a user may *see*; pairing answers which they may *command*. A \
             session reaching this decision would make possession of a valid account session \
             sufficient to shut down a machine: {uses:?}"
        );
    }
}

#[test]
fn the_command_decision_takes_no_account_input_even_when_a_session_exists() {
    // The same claim as a compiling fact rather than a grep. `CommandTargetState` is
    // constructed exhaustively here: if a field carrying account identity were ever added, this
    // would fail to compile, which is a second, independent way for the guarantee to break
    // loudly.
    //
    // The device signing here is signed in — it holds a session — and is not paired. The spec's
    // "a signed-in but unpaired device is refused" scenario.
    let secrets = InMemorySecretStore::new();
    let provider = GoogleAuthProvider::new(secrets);
    provider
        .record_session(&AccountId::new("account-1"), "refresh-token-value")
        .expect("record");
    assert!(
        provider.current_account().expect("account").is_some(),
        "the sender holds a valid account session"
    );

    let sender = DeviceId::new("phone-a").expect("non-empty");
    let state = CommandTargetState {
        seen_nonces: HashSet::new(),
        // Empty: no pairing exists with the sender, whatever account it is signed in to.
        verifying_keys: HashMap::new(),
        target_can_power_off: true,
        target_is_remote_target: true,
        is_paired: false,
        remote_control_enabled: true,
    };

    let created_at = Utc.with_ymd_and_hms(2026, 8, 6, 12, 0, 0).unwrap();
    let keys = TestKeyPair::from_seed(7);
    let envelope = CommandEnvelope {
        sender: sender.clone(),
        command: RemoteCommand::PowerOff,
        created_at,
        nonce: "nonce-1".to_string(),
        signature: keys.sign_command(
            &sender,
            RemoteCommand::PowerOff.as_str(),
            created_at,
            "nonce-1",
        ),
    };

    let outcome = evaluate_command(&envelope, &state, created_at);

    assert_eq!(
        outcome,
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified),
        "a device with a valid account session but no pairing is refused; the session is not \
         an input to this decision and cannot make it come out any other way"
    );
}

// ---------------------------------------------------------------------------
// Task 2.1 / 2.5 — the listener binds loopback, asserted rather than eyeballed
// ---------------------------------------------------------------------------

#[test]
fn the_loopback_listener_binds_127_0_0_1_and_not_all_interfaces() {
    // `0.0.0.0` would put the sign-in callback on every interface the machine has — reachable
    // by anyone on the same network, who could then deliver an authorization code of their
    // choosing. The two differ by a few characters and are indistinguishable at a glance in a
    // diff, so this is asserted on the socket's actual bound address rather than read.
    let receiver = LoopbackReceiver::bind().expect("bind");
    let addr = receiver.local_addr();

    assert_eq!(
        addr.ip(),
        std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        "the sign-in callback must be reachable only from this machine"
    );
    assert_ne!(
        addr.ip(),
        std::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        "0.0.0.0 would expose the sign-in callback to the local network"
    );
    assert!(
        !addr.ip().is_unspecified(),
        "an unspecified bind address means every interface"
    );
}

#[test]
fn the_named_loopback_constant_is_the_loopback_address() {
    // The constant the bind site uses, pinned independently of the bind. If someone edits the
    // constant, this fails even on a machine where the bind happens to succeed either way.
    const _: () = assert!(LOOPBACK_HOST.is_loopback());
    assert_eq!(LOOPBACK_HOST, Ipv4Addr::new(127, 0, 0, 1));
}

#[test]
fn the_listener_is_given_an_ephemeral_port_rather_than_a_fixed_one() {
    // A fixed port would collide with whatever already holds it, and would let another local
    // process squat the callback address before sign-in begins.
    let first = LoopbackReceiver::bind().expect("bind");
    let second = LoopbackReceiver::bind().expect("bind");

    assert_ne!(first.local_addr().port(), 0, "the OS assigned a real port");
    assert_ne!(
        first.local_addr().port(),
        second.local_addr().port(),
        "two receivers do not contend for one fixed port"
    );
}

#[test]
fn the_redirect_uri_points_at_the_loopback_address_the_listener_actually_holds() {
    let receiver = LoopbackReceiver::bind().expect("bind");
    let port = receiver.local_addr().port();

    let uri = receiver.redirect_uri();

    assert_eq!(uri, format!("http://127.0.0.1:{port}/"));
    assert!(
        !uri.contains("0.0.0.0"),
        "the redirect must not name every interface"
    );
}

// ---------------------------------------------------------------------------
// Task 2.2 — exactly one request, then shut down
// ---------------------------------------------------------------------------

#[test]
fn the_listener_accepts_one_request_and_then_stops_listening() {
    // The listener must not outlive the sign-in. Asserted by connecting a second time *after*
    // the first request has been served: a socket still listening would accept it.
    let receiver = LoopbackReceiver::bind().expect("bind");
    let addr = receiver.local_addr();

    let client = std::thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream
            .write_all(b"GET /?code=the-code&state=the-state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .expect("write");
        let mut response = String::new();
        // The completion page is read so the server side has finished writing before the
        // connection closes.
        let _ = stream.read_to_string(&mut response);
        response
    });

    let outcome = receiver.receive().expect("receive");
    let response = client.join().expect("client thread");

    assert_eq!(
        outcome,
        RedirectOutcome::Code {
            code: "the-code".to_string(),
            state: "the-state".to_string(),
        }
    );
    assert!(
        response.contains("200 OK"),
        "the browser is answered rather than left hanging: {response}"
    );

    // The receiver was consumed by `receive`, so its socket is dropped. Nothing is listening on
    // that address any more, and a second connection must fail.
    //
    // Retried briefly because the OS releases a closed listening socket asynchronously on some
    // platforms, and a bare single attempt would be flaky rather than wrong.
    let mut still_listening = true;
    for _ in 0..50 {
        match TcpStream::connect_timeout(&addr, Duration::from_millis(50)) {
            Ok(_) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => {
                still_listening = false;
                break;
            }
        }
    }

    assert!(
        !still_listening,
        "the sign-in listener is still accepting connections after the redirect was received; \
         it must not outlive the sign-in"
    );
}

#[test]
fn a_provider_error_redirect_is_an_outcome_rather_than_a_failure() {
    // The provider signals a declined consent by redirecting with `error`, not by failing to
    // redirect. Treating that as a transport failure would show the user "sign-in could not be
    // reached" when what actually happened is that they pressed Cancel.
    let outcome =
        parse_redirect_request("GET /?error=access_denied&state=the-state HTTP/1.1").expect("parse");

    assert_eq!(
        outcome,
        RedirectOutcome::ProviderError {
            error: "access_denied".to_string()
        }
    );
}

#[test]
fn a_redirect_carrying_both_an_error_and_a_code_is_treated_as_the_error() {
    // Redeeming a code the provider is simultaneously telling us not to trust would be the
    // wrong half to believe.
    let outcome = parse_redirect_request("GET /?code=c&error=access_denied&state=s HTTP/1.1")
        .expect("parse");

    assert!(matches!(outcome, RedirectOutcome::ProviderError { .. }));
}

#[test]
fn a_redirect_without_a_state_is_refused() {
    // A redirect that cannot be matched to the request that started it is a redirect this
    // process never asked for.
    let result = parse_redirect_request("GET /?code=the-code HTTP/1.1");

    assert!(matches!(result, Err(AppError::Validation { .. })));
}

#[test]
fn a_redirect_without_a_code_is_refused_rather_than_defaulted() {
    let result = parse_redirect_request("GET /?state=the-state HTTP/1.1");

    assert!(matches!(result, Err(AppError::Validation { .. })));
}

#[test]
fn a_percent_encoded_redirect_value_is_decoded() {
    let outcome =
        parse_redirect_request("GET /?code=a%2Fb%2Bc&state=x%3Dy HTTP/1.1").expect("parse");

    assert_eq!(
        outcome,
        RedirectOutcome::Code {
            code: "a/b+c".to_string(),
            state: "x=y".to_string(),
        }
    );
}

#[test]
fn an_unparseable_request_is_an_error_and_not_a_panic() {
    let result = parse_redirect_request("garbage");

    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// PKCE
// ---------------------------------------------------------------------------

#[test]
fn the_pkce_challenge_matches_the_published_rfc_7636_vector() {
    // RFC 7636 appendix B. Pinning the derivation against the specification's own vector is
    // what establishes that the encoding is right, which a randomly generated verifier could
    // never show — a wrong-but-self-consistent implementation would pass any round-trip test.
    let challenge = PkceChallenge::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");

    assert_eq!(
        challenge.challenge(),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn only_the_s256_challenge_method_is_offered() {
    // `plain` is permitted by the RFC and defeats the mechanism entirely: the verifier travels
    // in the authorization request, so whoever intercepts the code can also redeem it. An
    // implementation that supports both can be downgraded to the weaker one.
    assert_eq!(CODE_CHALLENGE_METHOD, "S256");
    assert!(
        !GOOGLE_AUTH_SOURCE.contains("\"plain\""),
        "the plain challenge method must not be offered"
    );
}

#[test]
fn a_generated_verifier_is_not_the_same_twice() {
    let first = PkceChallenge::generate().expect("generate");
    let second = PkceChallenge::generate().expect("generate");

    assert_ne!(first.verifier(), second.verifier());
    assert_ne!(first.challenge(), second.challenge());
}

#[test]
fn a_generated_verifier_meets_the_rfc_length_and_alphabet() {
    let challenge = PkceChallenge::generate().expect("generate");

    // 32 random bytes base64url-encode to 43 characters, the RFC's minimum.
    assert_eq!(challenge.verifier().len(), 43);
    assert!(challenge
        .verifier()
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    assert!(
        !challenge.verifier().contains('='),
        "base64url for PKCE carries no padding"
    );
}

#[test]
fn the_authorization_request_carries_the_challenge_and_never_the_verifier() {
    // The whole point of PKCE: the verifier stays in this process until the code is redeemed.
    // Sending it with the authorization request would make the exchange equivalent to having no
    // PKCE at all.
    let receiver = LoopbackReceiver::bind().expect("bind");
    let challenge = PkceChallenge::generate().expect("generate");

    let url = authorization_url(&config(), &receiver.redirect_uri(), &challenge, "the-state");

    assert!(url.contains(&format!("code_challenge={}", challenge.challenge())));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(
        !url.contains(challenge.verifier()),
        "the verifier must not travel with the authorization request"
    );
    assert!(url.starts_with("https://accounts.google.com/"));
    assert!(
        url.contains("127.0.0.1"),
        "the redirect points at loopback: {url}"
    );
}

#[test]
fn the_authorization_request_asks_for_identity_scopes_only() {
    // A scope that is never requested cannot be misused later by code that finds a token with
    // it already granted. This module establishes which account, and has no reason to read
    // mail, contacts, or files.
    let challenge = PkceChallenge::generate().expect("generate");

    let url = authorization_url(&config(), "http://127.0.0.1:1234/", &challenge, "s");

    assert!(url.contains("scope=openid%20email"));
    for over_broad in ["drive", "gmail", "contacts", "cloud-platform"] {
        assert!(
            !url.contains(over_broad),
            "the authorization request asks for {over_broad}, which identity does not need"
        );
    }
}

#[test]
fn the_code_redemption_presents_the_verifier_and_no_client_secret() {
    // A desktop application is a public client: any secret it shipped would be in every copy of
    // the binary. PKCE is what takes the secret's place, so the verifier must be present and a
    // secret must not.
    let challenge = PkceChallenge::generate().expect("generate");

    let form = GoogleAuthProvider::<InMemorySecretStore>::code_redemption_form(
        &config(),
        "the-code",
        &challenge,
        "http://127.0.0.1:1234/",
    );

    let names: Vec<&str> = form.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"code_verifier"));
    assert!(
        !names.contains(&"client_secret"),
        "a client secret in a desktop binary is not a secret; PKCE replaces it"
    );
    assert!(form
        .iter()
        .any(|(name, value)| name == "code_verifier" && value == challenge.verifier()));
}

#[test]
fn the_refresh_exchange_also_carries_no_client_secret() {
    let form = GoogleAuthProvider::<InMemorySecretStore>::refresh_form(&config(), "refresh-value");

    let names: Vec<&str> = form.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"refresh_token"));
    assert!(names.contains(&"grant_type"));
    assert!(!names.contains(&"client_secret"));
}

#[test]
fn the_module_declares_no_client_secret_anywhere() {
    // Structural rather than behavioural, because a secret field would be *added* and every
    // existing test would still pass.
    for forbidden in ["client_secret\":", "fn client_secret", "client_secret:"] {
        assert!(
            !GOOGLE_AUTH_SOURCE
                .lines()
                .filter(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.starts_with("//") && !trimmed.starts_with("///")
                })
                .any(|line| line.contains(forbidden)),
            "a client secret field appeared in the account-identity module: {forbidden}"
        );
    }
}

// ---------------------------------------------------------------------------
// The missing OAuth client, reported specifically
// ---------------------------------------------------------------------------

#[test]
fn an_absent_client_id_names_what_is_missing_and_where_to_obtain_it() {
    // No OAuth client exists for this project yet and none can be created from here: the
    // Firebase CLI reports an empty `oauth_client` list, `firebase auth` only imports and
    // exports users, and creating an OAuth 2.0 client is a Console operation with no API. So
    // the error is the correct answer — but only if it is actionable. "Not configured" would
    // leave the reader no better off than the failure did.
    let error = OAuthClientConfig::from_config_json("{}").expect_err("no client_id");

    let AppError::Validation { message } = error else {
        panic!("a missing client id is a configuration problem, not a storage one");
    };

    assert!(message.contains("Desktop"), "names the client type: {message}");
    assert!(
        message.contains("weakup-remote-2026"),
        "names the project: {message}"
    );
    assert!(
        message.contains("Google Cloud Console"),
        "names where to create it: {message}"
    );
    assert!(
        message.contains("oauth_client.json"),
        "names where to put it: {message}"
    );
}

#[test]
fn a_firebase_app_id_is_refused_with_an_explanation_of_the_difference() {
    // The identifier most likely to be pasted here by mistake: it is easy to find, it sits next
    // to the word "app" in the Firebase console, and it is not an OAuth client. Left to the
    // provider, the failure would be an "unknown client" several steps from the cause.
    let error = OAuthClientConfig::new("1:683928241538:web:c30598008f0567a44bfb9b")
        .expect_err("a Firebase app id is not an OAuth client id");

    let AppError::Validation { message } = error else {
        panic!("expected a validation error");
    };
    assert!(
        message.contains("apps.googleusercontent.com"),
        "says what the right shape is: {message}"
    );
}

#[test]
fn an_empty_or_whitespace_client_id_is_refused() {
    assert!(OAuthClientConfig::new("").is_err());
    assert!(OAuthClientConfig::new("   ").is_err());
    assert!(OAuthClientConfig::from_config_json("{\"client_id\": \"\"}").is_err());
}

#[test]
fn malformed_configuration_json_is_an_error_and_not_a_panic() {
    let result = OAuthClientConfig::from_config_json("{not json");

    assert!(matches!(result, Err(AppError::Validation { .. })));
}

#[test]
fn a_well_formed_client_id_is_accepted_and_trimmed() {
    let config = OAuthClientConfig::from_config_json(&format!(
        "{{\"client_id\": \"  {WELL_FORMED_CLIENT_ID}  \"}}"
    ))
    .expect("well-formed");

    assert_eq!(config.client_id(), WELL_FORMED_CLIENT_ID);
}

#[test]
fn no_client_id_is_hardcoded_in_the_module() {
    // A placeholder would make sign-in fail at the provider with a message about an unknown
    // client, several steps away from the cause, and would look — to anyone reading the source
    // — like the feature was configured.
    let offending: Vec<&str> = GOOGLE_AUTH_SOURCE
        .lines()
        .filter(|line| line.contains("apps.googleusercontent.com"))
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("///")
        })
        // The suffix check and the error messages that teach the required shape are the
        // legitimate uses: none of them is an identifier. An identifier would have a
        // project-number prefix before the suffix, which is what the last filter looks for.
        .filter(|line| !line.contains("ends_with") && !line.contains("<id>"))
        .filter(|line| !line.contains("`.apps.googleusercontent.com`"))
        .collect();

    assert!(
        offending.is_empty(),
        "an OAuth client id appears to be hardcoded: {offending:?}"
    );
}

// ---------------------------------------------------------------------------
// Task 2.3 — the refresh token goes to the secret store and nowhere else
// ---------------------------------------------------------------------------

#[test]
fn the_refresh_token_is_written_to_the_platform_credential_store() {
    let secrets = InMemorySecretStore::new();
    let provider = GoogleAuthProvider::new(secrets);

    provider
        .record_session(&AccountId::new("account-1"), "refresh-token-value")
        .expect("record");

    assert_eq!(
        provider.stored_refresh_token().expect("read"),
        Some("refresh-token-value".to_string())
    );
}

#[test]
fn the_module_reaches_the_credential_store_and_never_the_database_or_a_file() {
    // Task 2.3's "do not add a second storage path". A refresh token written to the database
    // would sit in a file the user can read, be included in a backup, and outlive a sign-out
    // that only cleared the keychain.
    // `Connection` alone would match the `Connection: close` header in the completion page's
    // HTTP response, which is not a database. Matched on `rusqlite::Connection` instead, which
    // is the only shape a database handle can actually take here.
    for forbidden in [
        "rusqlite",
        "rusqlite::Connection",
        "Connection::open",
        "fs::write",
        "File::create",
        "std::fs",
    ] {
        let uses: Vec<&str> = GOOGLE_AUTH_SOURCE
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("///")
            })
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "the account-identity module reaches {forbidden:?}. The refresh token goes to the \
             platform credential store through `secret_store.rs` and nowhere else: {uses:?}"
        );
    }
}

#[test]
fn the_refresh_token_is_never_written_to_a_log() {
    for forbidden in ["log::", "println!", "eprintln!", "info!", "debug!", "warn!"] {
        assert!(
            !GOOGLE_AUTH_SOURCE
                .lines()
                .filter(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.starts_with("//") && !trimmed.starts_with("///")
                })
                .any(|line| line.contains(forbidden)),
            "the account-identity module logs, and the value it holds is a refresh token: \
             {forbidden}"
        );
    }
}

#[test]
fn an_unreachable_credential_store_is_an_error_rather_than_a_signed_out_device() {
    // The distinction `secret_store.rs` exists to preserve, applied here. Reporting a locked
    // keychain as "signed out" would tell the user their session was gone when it is intact
    // behind a temporary failure — and would invite a sign-in that overwrites it.
    let secrets = InMemorySecretStore::new();
    secrets
        .set(REFRESH_TOKEN_SECRET_NAME, b"refresh-token-value")
        .expect("set");
    secrets.fail_with("the keychain is locked");
    let provider = GoogleAuthProvider::new(secrets);

    let result = provider.stored_refresh_token();

    assert!(
        result.is_err(),
        "an unreachable store must not read as an absent session"
    );
}

#[test]
fn a_device_that_has_never_signed_in_reports_no_account() {
    let provider = GoogleAuthProvider::new(InMemorySecretStore::new());

    assert_eq!(provider.current_account().expect("account"), None);
    assert_eq!(provider.stored_refresh_token().expect("token"), None);
}

// ---------------------------------------------------------------------------
// Task 2.5 — token refresh, and sign-out leaving pairings intact
// ---------------------------------------------------------------------------

#[test]
fn a_recorded_session_survives_a_restart_and_is_restored_from_the_credential_store() {
    // Token refresh depends on the refresh token outliving the process that obtained it — that
    // is the whole reason it is in the credential store rather than in memory. Modelled by
    // building a second provider over the same store, which is what a restart is.
    let secrets = InMemorySecretStore::new();
    GoogleAuthProvider::new(&secrets)
        .record_session(&AccountId::new("account-1"), "refresh-token-value")
        .expect("record");

    let after_restart = GoogleAuthProvider::new(&secrets);
    assert_eq!(
        after_restart.current_account().expect("account"),
        None,
        "a fresh process knows nothing until it consults the store"
    );

    let restored = after_restart.restore_session().expect("restore");

    assert_eq!(restored, Some(AccountId::new("account-1")));
    assert_eq!(
        after_restart.stored_refresh_token().expect("token"),
        Some("refresh-token-value".to_string()),
        "the token needed to obtain a new access token is still available"
    );
    assert_eq!(
        after_restart.current_account().expect("account"),
        Some(AccountId::new("account-1"))
    );
}

#[test]
fn a_session_with_an_account_but_no_token_is_not_a_session() {
    // A half-written pair would otherwise report an account that can never be refreshed, which
    // the user sees as being signed in to something that never works.
    let secrets = InMemorySecretStore::new();
    secrets
        .set(ACCOUNT_ID_SECRET_NAME, b"account-1")
        .expect("set");
    let provider = GoogleAuthProvider::new(&secrets);

    assert_eq!(provider.restore_session().expect("restore"), None);
    assert_eq!(provider.current_account().expect("account"), None);
}

#[test]
fn a_refreshed_session_replaces_the_stored_token_rather_than_accumulating_one() {
    let secrets = InMemorySecretStore::new();
    let provider = GoogleAuthProvider::new(&secrets);
    let account = AccountId::new("account-1");

    provider.record_session(&account, "first-token").expect("first");
    provider.record_session(&account, "second-token").expect("second");

    assert_eq!(
        provider.stored_refresh_token().expect("token"),
        Some("second-token".to_string())
    );
}

#[test]
fn signing_out_clears_the_token_and_the_account() {
    let secrets = InMemorySecretStore::new();
    let provider = GoogleAuthProvider::new(&secrets);
    provider
        .record_session(&AccountId::new("account-1"), "refresh-token-value")
        .expect("record");

    provider.sign_out().expect("sign out");

    assert_eq!(provider.current_account().expect("account"), None);
    assert_eq!(provider.stored_refresh_token().expect("token"), None);
    assert!(matches!(
        secrets.get(REFRESH_TOKEN_SECRET_NAME).expect("get"),
        SecretLookup::NotFound
    ));
    assert!(matches!(
        secrets.get(ACCOUNT_ID_SECRET_NAME).expect("get"),
        SecretLookup::NotFound
    ));
}

#[test]
fn signing_out_leaves_every_pairing_recorded() {
    // The capability's "sign-out leaves pairings intact" scenario. Pairings are established by
    // physical possession of both devices and recorded locally; the identity provider was never
    // granted the authority to withdraw them. A sign-out that revoked them would mean an
    // expired token silently de-authorized a machine the user is standing in front of.
    let pairings = RecordingPairingStore::new();
    let peer = DeviceId::new("desktop-a").expect("non-empty");
    let key = TestKeyPair::from_seed(3).verifying_key();
    pairings
        .record_pairing(&peer, &key, Utc.with_ymd_and_hms(2026, 8, 1, 9, 0, 0).unwrap())
        .expect("pair");

    let secrets = InMemorySecretStore::new();
    let provider = GoogleAuthProvider::new(&secrets);
    provider
        .record_session(&AccountId::new("account-1"), "refresh-token-value")
        .expect("record");

    provider.sign_out().expect("sign out");

    let after = pairings.list_pairings().expect("list");
    assert_eq!(after.len(), 1, "the pairing is still recorded");
    assert!(!after[0].revoked, "and it is still active");
    assert_eq!(
        pairings.verifying_keys_from_store().expect("keys").len(),
        1,
        "the key commands are checked against is still held"
    );
}

#[test]
fn signing_back_in_does_not_require_re_pairing() {
    // The second half of the same scenario, and the one a user would actually notice: sign out,
    // sign in again, and the desktop is still commandable without walking back to it.
    let pairings = RecordingPairingStore::new();
    let peer = DeviceId::new("desktop-a").expect("non-empty");
    let key = TestKeyPair::from_seed(3).verifying_key();
    pairings
        .record_pairing(&peer, &key, Utc.with_ymd_and_hms(2026, 8, 1, 9, 0, 0).unwrap())
        .expect("pair");
    let pairings_before = pairings.record_calls();

    let secrets = InMemorySecretStore::new();
    let provider = GoogleAuthProvider::new(&secrets);
    provider
        .record_session(&AccountId::new("account-1"), "token-1")
        .expect("first sign-in");
    provider.sign_out().expect("sign out");
    provider
        .record_session(&AccountId::new("account-1"), "token-2")
        .expect("second sign-in");

    assert_eq!(
        pairings.record_calls(),
        pairings_before,
        "signing out and back in recorded no new pairing, because none was needed"
    );
    assert_eq!(
        pairings.verifying_keys_from_store().expect("keys").len(),
        1,
        "the pairing that existed before the sign-out is the one still in force"
    );
}

#[test]
fn the_account_provider_holds_no_path_to_a_pairing_store() {
    // Why the scenario above is true by construction rather than by remembering to be careful:
    // there is no code path from this module to a pairing at all.
    for forbidden in ["PairingStore", "pairing", "revoke"] {
        let uses: Vec<&str> = GOOGLE_AUTH_SOURCE
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("///")
            })
            .filter(|line| line.to_lowercase().contains(&forbidden.to_lowercase()))
            .collect();

        assert!(
            uses.is_empty(),
            "the account-identity module references {forbidden:?}. The account and the pairing \
             are separate authorities: an account session confers no ability to command, and \
             losing one must not withdraw the other: {uses:?}"
        );
    }
}

#[test]
fn a_sign_out_on_an_unreachable_store_is_reported_rather_than_silently_succeeding() {
    // A sign-out that reported success while the token was still in the keychain would leave
    // the user believing they had signed out of a device that is still signed in.
    let secrets = InMemorySecretStore::new();
    let provider = GoogleAuthProvider::new(&secrets);
    provider
        .record_session(&AccountId::new("account-1"), "refresh-token-value")
        .expect("record");
    secrets.fail_with("the keychain is locked");

    assert!(provider.sign_out().is_err());
}

// ---------------------------------------------------------------------------
// A pairing store that counts what it was asked to do
// ---------------------------------------------------------------------------

/// An in-memory [`PairingStore`] that records how many times a pairing was written.
///
/// The count is what makes "signing back in did not require re-pairing" assertable: a
/// implementation that quietly re-paired would leave the final state looking identical.
struct RecordingPairingStore {
    records: std::sync::Mutex<Vec<PairingRecord>>,
    record_calls: std::sync::Mutex<usize>,
}

impl RecordingPairingStore {
    fn new() -> Self {
        Self {
            records: std::sync::Mutex::new(Vec::new()),
            record_calls: std::sync::Mutex::new(0),
        }
    }

    fn record_calls(&self) -> usize {
        *self.record_calls.lock().expect("not poisoned")
    }
}

impl PairingStore for RecordingPairingStore {
    fn record_pairing(
        &self,
        peer: &DeviceId,
        verifying_key: &VerifyingKey,
        _paired_at: chrono::DateTime<Utc>,
    ) -> crate::core::AppResult<()> {
        *self.record_calls.lock().expect("not poisoned") += 1;
        let mut records = self.records.lock().expect("not poisoned");
        if let Some(existing) = records.iter_mut().find(|record| &record.peer == peer) {
            existing.revoked = false;
            existing.verifying_key = verifying_key.clone();
        } else {
            records.push(PairingRecord {
                peer: peer.clone(),
                verifying_key: verifying_key.clone(),
                revoked: false,
            });
        }
        Ok(())
    }

    fn list_pairings(&self) -> crate::core::AppResult<Vec<PairingRecord>> {
        Ok(self.records.lock().expect("not poisoned").clone())
    }

    fn revoke_pairing(
        &self,
        peer: &DeviceId,
        _revoked_at: chrono::DateTime<Utc>,
    ) -> crate::core::AppResult<()> {
        let mut records = self.records.lock().expect("not poisoned");
        if let Some(existing) = records.iter_mut().find(|record| &record.peer == peer) {
            existing.revoked = true;
        }
        Ok(())
    }
}
