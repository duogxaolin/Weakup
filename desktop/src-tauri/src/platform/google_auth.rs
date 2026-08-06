//! Account identity through Google, completed in the user's own browser.
//!
//! This module implements [`AuthProvider`] and nothing else. It answers "which account is
//! this device acting as", and it is deliberately incapable of answering anything about
//! authority — see the account-identity capability, and the source-text test in
//! `google_auth_tests.rs` that fails the build if account identity ever reaches the command
//! decision.
//!
//! # Why the system browser, never an embedded web view (design D3)
//!
//! An embedded web view for sign-in is what phishing looks like. The user cannot inspect the
//! address bar, cannot see the certificate, and has no way to tell a real Google page from a
//! rendered imitation — so the habit of typing a Google password into a window an application
//! drew is exactly the habit that makes phishing work. Google's own guidance for native apps
//! is the system browser for this reason, and it additionally lets the user's existing
//! session, password manager, and second factor work as they already do.
//!
//! So sign-in opens the default browser and receives the redirect on a loopback listener.
//!
//! # Why the listener binds `127.0.0.1` and not `0.0.0.0`
//!
//! `0.0.0.0` binds every interface, which would put the sign-in callback on the local network:
//! anyone on the same café Wi-Fi could reach it and deliver an authorization code of their
//! choosing. `127.0.0.1` is reachable only from this machine. The distinction is one character
//! wide and entirely invisible when reading a diff, which is why [`LOOPBACK_HOST`] is a named
//! constant asserted on by a test rather than a literal typed at the bind site.
//!
//! The port is ephemeral (`0`, assigned by the OS) because a fixed port would collide with
//! whatever else happens to hold it, and the listener accepts **exactly one request** and then
//! shuts down. It is not a service that outlives the sign-in — see [`LoopbackReceiver`].
//!
//! # Why PKCE, and why there is no client secret
//!
//! A desktop application is a *public* client: whatever secret it shipped would be present in
//! every copy of the binary, so it would not be a secret. PKCE (RFC 7636) is what replaces it.
//! The app generates a random verifier, sends only its SHA-256 hash (`S256`) with the
//! authorization request, and presents the verifier itself when redeeming the code. An
//! attacker who intercepts the authorization code — the loopback redirect is the classic place
//! this is attempted — cannot redeem it without the verifier, which never left this process.
//!
//! `plain` is not offered. It is permitted by the RFC and defeats the entire mechanism, and an
//! implementation that supports both can be downgraded to the weaker one by whatever it is
//! talking to.
//!
//! # What is UNVERIFIED here
//!
//! Sign-in has never been completed end to end, and cannot be from this repository: it needs an
//! OAuth client that does not yet exist (see [`OAuthClientConfig`]), a browser, and a human.
//! What the tests do cover is every decision this module makes on its own — the bind address,
//! the one-request shutdown, the PKCE construction, the redirect parsing, where the refresh
//! token is put, and what sign-out does and does not touch. The network round trip to Google is
//! not covered and is not claimed.

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::Mutex;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::application::transport::{AccountId, AuthProvider};
use crate::core::{AppError, AppResult};
use crate::platform::secret_store::{SecretLookup, SecretStore};

// ---------------------------------------------------------------------------
// The loopback listener
// ---------------------------------------------------------------------------

/// The only interface the sign-in callback is reachable on.
///
/// A named constant rather than a literal at the bind site, because `127.0.0.1` and `0.0.0.0`
/// differ by a few characters and mean "this machine only" versus "every machine that can
/// route to this one". A test asserts on this value; it is not something to eyeball in review.
pub const LOOPBACK_HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;

/// The OS assigns the port. A fixed one would collide with whatever already holds it, and
/// would also let another local process squat the callback address before sign-in begins.
pub const EPHEMERAL_PORT: u16 = 0;

/// How long the listener waits for the browser to come back before giving up.
///
/// Bounded so that an abandoned sign-in — the user closes the tab, or never completes the
/// consent screen — releases the socket rather than leaving a listener alive indefinitely.
pub const REDIRECT_TIMEOUT: Duration = Duration::from_secs(300);

/// What the browser is shown once the redirect has been received.
///
/// Deliberately static and self-contained: no script, no external resource, nothing that
/// depends on the network. It exists only so the user is not left staring at a blank tab.
const COMPLETION_PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>Signed in</title></head><body>\
<p>Signed in. You can close this tab and return to Weakup.</p>\
</body></html>";

/// A loopback socket that accepts exactly one request and is then finished.
///
/// The lifetime is the point. This is not a server the application runs; it is open for the
/// duration of one sign-in and closed as soon as the browser has come back. [`receive`] takes
/// `self` by value, so the listener is dropped — and the socket released — when it returns,
/// and the type system prevents a second request being accepted through the same receiver.
///
/// [`receive`]: LoopbackReceiver::receive
pub struct LoopbackReceiver {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl LoopbackReceiver {
    /// Binds a loopback socket on an OS-assigned port.
    pub fn bind() -> AppResult<Self> {
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(LOOPBACK_HOST), EPHEMERAL_PORT))
            .map_err(|e| AppError::Validation {
                message: format!("the sign-in callback socket could not be opened: {e}"),
            })?;

        let local_addr = listener.local_addr().map_err(|e| AppError::Validation {
            message: format!("the sign-in callback socket has no address: {e}"),
        })?;

        Ok(Self {
            listener,
            local_addr,
        })
    }

    /// The address the browser will be told to come back to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// The `redirect_uri` this receiver corresponds to.
    ///
    /// `http` rather than `https` is correct and is what Google's native-app flow specifies for
    /// loopback: the connection never leaves the machine, and a certificate for `127.0.0.1`
    /// could not be issued by a public CA in any case.
    pub fn redirect_uri(&self) -> String {
        format!("http://{}:{}/", LOOPBACK_HOST, self.local_addr.port())
    }

    /// Accepts **one** request, answers it, and shuts down.
    ///
    /// Consumes `self`: after this returns the socket is closed, whether the request was a
    /// successful redirect, an error redirect, or unparseable. There is no path by which this
    /// receiver accepts a second connection.
    pub fn receive(self) -> AppResult<RedirectOutcome> {
        self.listener
            .set_nonblocking(false)
            .map_err(|e| AppError::Validation {
                message: format!("the sign-in callback socket could not be configured: {e}"),
            })?;

        let (stream, _peer) = self.listener.accept().map_err(|e| AppError::Validation {
            message: format!("the sign-in callback was not received: {e}"),
        })?;

        stream
            .set_read_timeout(Some(REDIRECT_TIMEOUT))
            .map_err(|e| AppError::Validation {
                message: format!("the sign-in callback socket could not be configured: {e}"),
            })?;

        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .map_err(|e| AppError::Validation {
                message: format!("the sign-in callback could not be read: {e}"),
            })?;

        // Answered before the outcome is judged, so the user sees a completed page even when
        // the provider redirected with an error.
        let mut writer = &stream;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            COMPLETION_PAGE.len(),
            COMPLETION_PAGE
        );
        // A browser that has already gone away is not a sign-in failure: the code, if there
        // was one, was still received. So a write failure here is deliberately not fatal.
        let _ = writer.write_all(response.as_bytes());
        let _ = writer.flush();

        // `self.listener` drops here, releasing the socket. This is the "shuts down
        // immediately" half of design D3, and it happens on every return path because it is a
        // drop rather than an explicit close someone could forget to call.
        parse_redirect_request(&request_line)
    }
}

/// What came back on the redirect.
///
/// The provider signals refusal by redirecting with an `error` parameter rather than by failing
/// to redirect, so "the user declined" is an outcome to be reported, not an error to be
/// retried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectOutcome {
    /// An authorization code, still to be redeemed. Redeeming it needs the PKCE verifier.
    Code {
        code: String,
        /// Echoed back by the provider. The caller compares it with what it sent.
        state: String,
    },
    /// The provider reported a failure — most commonly the user declining consent.
    ProviderError { error: String },
}

/// Parses the request line of the single redirect request.
///
/// Split out from the socket handling so it can be tested against every shape the provider
/// can produce without opening a socket at all.
pub fn parse_redirect_request(request_line: &str) -> AppResult<RedirectOutcome> {
    // `GET /?code=...&state=... HTTP/1.1`
    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| AppError::Validation {
            message: "the sign-in callback was not a recognisable HTTP request".to_string(),
        })?;

    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");

    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    let mut error: Option<String> = None;

    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = match pair.split_once('=') {
            Some(parts) => parts,
            None => continue,
        };
        let value = percent_decode(value);
        match name {
            "code" => code = Some(value),
            "state" => state = Some(value),
            "error" => error = Some(value),
            _ => {}
        }
    }

    // Checked before the code, because a redirect carrying both is a refusal, and treating it
    // as a success would redeem a code the provider is telling us not to trust.
    if let Some(error) = error {
        return Ok(RedirectOutcome::ProviderError { error });
    }

    match (code, state) {
        (Some(code), Some(state)) => Ok(RedirectOutcome::Code { code, state }),
        // Each field is required explicitly rather than defaulted. A redirect without a state
        // cannot be matched to the request that started it, and accepting one would accept a
        // code from a sign-in this process never began.
        (None, _) => Err(AppError::Validation {
            message: "the sign-in callback carried no authorization code".to_string(),
        }),
        (_, None) => Err(AppError::Validation {
            message: "the sign-in callback carried no state parameter, so it cannot be matched \
                      to the sign-in that started it"
                .to_string(),
        }),
    }
}

/// Decodes the percent-encoding a redirect query uses.
///
/// Hand-written rather than pulling a crate for it: the input is one query string from one
/// provider, and the alternative was a dependency for twenty lines.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    // Not a valid escape; the `%` is literal.
                    Err(_) => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// PKCE
// ---------------------------------------------------------------------------

/// The only code challenge method this module will use.
///
/// `plain` is deliberately absent. The RFC permits it and it defeats the entire mechanism —
/// the verifier is sent in the clear in the authorization request, so anyone who can intercept
/// the code can also redeem it. An implementation offering both can be talked down to the
/// weaker one.
pub const CODE_CHALLENGE_METHOD: &str = "S256";

/// A PKCE verifier and the challenge derived from it.
///
/// The verifier stays in this process for the lifetime of one sign-in and is presented only
/// when the code is redeemed. The challenge is the only half that travels with the
/// authorization request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkceChallenge {
    verifier: String,
    challenge: String,
}

impl PkceChallenge {
    /// Derives a challenge from an existing verifier.
    ///
    /// Separate from [`generate`] so the derivation can be tested against RFC 7636's published
    /// test vector, which a randomly generated verifier could never exercise.
    ///
    /// [`generate`]: PkceChallenge::generate
    pub fn from_verifier(verifier: impl Into<String>) -> Self {
        let verifier = verifier.into();
        let digest = Sha256::digest(verifier.as_bytes());
        let challenge = base64_url_no_pad(&digest);
        Self {
            verifier,
            challenge,
        }
    }

    /// A fresh verifier from the system CSPRNG, and its challenge.
    ///
    /// 32 bytes, which base64url-encodes to 43 characters — the RFC's minimum length, and its
    /// recommended construction.
    pub fn generate() -> AppResult<Self> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| AppError::Validation {
            message: format!("the system random source could not be read: {e}"),
        })?;
        Ok(Self::from_verifier(base64_url_no_pad(&bytes)))
    }

    /// Presented only when redeeming the code, never in the authorization request.
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    /// Sent with the authorization request. Reveals nothing about the verifier.
    pub fn challenge(&self) -> &str {
        &self.challenge
    }
}

/// A random value tying the redirect to the request that started it.
///
/// Distinct from the PKCE verifier and serving a different purpose: PKCE stops an intercepted
/// code being redeemed, `state` stops a redirect this process never asked for being accepted.
pub fn generate_state() -> AppResult<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| AppError::Validation {
        message: format!("the system random source could not be read: {e}"),
    })?;
    Ok(base64_url_no_pad(&bytes))
}

/// base64url without padding, as RFC 7636 requires for the challenge.
///
/// Hand-written for the same reason as [`percent_decode`]: a dependency for one small,
/// well-specified encoding that is exercised against a published test vector.
fn base64_url_no_pad(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        }
    }

    out
}

// ---------------------------------------------------------------------------
// The client configuration, which does not yet exist
// ---------------------------------------------------------------------------

/// Where the OAuth client identifier is read from.
///
/// Untracked, and listed in `.gitignore`. The file holds an identifier rather than a secret —
/// see [`OAuthClientConfig`] for why a desktop client has no secret at all — but it is
/// per-installation configuration rather than something the repository should assert, and
/// keeping it out of the tree means a fork does not inherit a client it cannot use.
pub const OAUTH_CLIENT_CONFIG_PATH: &str = "oauth_client.json";

/// The Google Cloud project the OAuth client must belong to, named in the error so the message
/// is actionable rather than merely accurate.
pub const OAUTH_PROJECT_ID: &str = "weakup-remote-2026";

/// The identifier of this application as the provider knows it.
///
/// # There is no client secret, deliberately
///
/// A desktop application is a public client. Any secret it shipped would be in every copy of
/// the binary and extractable from all of them, so it would not be a secret and treating it as
/// one would be worse than not having it — the security would be assumed rather than present.
/// PKCE is what does the job a client secret does for a confidential client. This struct has
/// therefore no secret field, and adding one would be a mistake rather than an improvement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthClientConfig {
    client_id: String,
}

impl OAuthClientConfig {
    /// Validates and wraps a client identifier.
    pub fn new(client_id: impl Into<String>) -> AppResult<Self> {
        let client_id = client_id.into().trim().to_string();

        if client_id.is_empty() {
            return Err(missing_client_id_error("it is present but empty"));
        }

        // A Firebase *app* id (`1:683928241538:web:...`) is the identifier most likely to be
        // pasted here by mistake: it is the one that is easy to find, it appears in the
        // Firebase console next to the word "app", and it is not an OAuth client. Rejected
        // with a message saying so, because the failure it otherwise produces — the provider
        // refusing the authorization request — says nothing about which of the two identifiers
        // was wrong.
        if !client_id.ends_with(".apps.googleusercontent.com") {
            return Err(missing_client_id_error(
                "the configured value is not an OAuth client id — an OAuth client id ends in \
                 `.apps.googleusercontent.com`. A Firebase app id (`1:...:web:...`) identifies \
                 an app, not an OAuth client, and cannot be used here",
            ));
        }

        Ok(Self { client_id })
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Reads the client id from `contents`, which is the untracked config file's text.
    ///
    /// Split from the file read so the parsing and every error it produces are testable
    /// without a filesystem.
    pub fn from_config_json(contents: &str) -> AppResult<Self> {
        let parsed: serde_json::Value =
            serde_json::from_str(contents).map_err(|e| missing_client_id_error(&format!(
                "the configuration file is not valid JSON ({e})"
            )))?;

        // Parsed explicitly: a missing field is an error, never a silent default. The same rule
        // design D8 states for the wire types applies here for the same reason.
        let client_id = parsed
            .get("client_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                missing_client_id_error(
                    "the configuration file has no `client_id` field. It should read: \
                     {\"client_id\": \"<id>.apps.googleusercontent.com\"}",
                )
            })?;

        Self::new(client_id)
    }
}

/// The error a caller gets when there is no usable OAuth client id.
///
/// # Why this is an error rather than a default
///
/// No OAuth client exists for this project yet, and one cannot be created from here: the
/// Firebase CLI reports `"oauth_client": []`, `firebase auth` only imports and exports users,
/// and creating an OAuth 2.0 client is a Google Cloud Console operation with no API behind it.
/// The credential genuinely does not exist.
///
/// A placeholder or invented identifier would be worse than this error in every respect. It
/// would make sign-in fail at the provider with a message about an unknown client, several
/// steps away from the actual cause, and it would look — to anyone reading the source — like
/// the feature was configured. An error naming exactly what is missing and where to obtain it
/// is the honest state of affairs.
fn missing_client_id_error(detail: &str) -> AppError {
    AppError::Validation {
        message: format!(
            "Google sign-in is not configured: {detail}. Create an OAuth 2.0 client of type \
             `Desktop app` in the Google Cloud Console for project `{OAUTH_PROJECT_ID}` \
             (APIs & Services → Credentials → Create credentials → OAuth client ID → Desktop \
             app), then write its id to `{OAUTH_CLIENT_CONFIG_PATH}` in the desktop app's \
             configuration directory as {{\"client_id\": \
             \"<id>.apps.googleusercontent.com\"}}. A desktop client has no client secret and \
             needs none; PKCE takes its place."
        ),
    }
}

// ---------------------------------------------------------------------------
// The authorization request
// ---------------------------------------------------------------------------

/// Google's authorization endpoint.
pub const AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";

/// Google's token endpoint, where a code is redeemed and a refresh token is used.
pub const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// What is asked for. Identity only.
///
/// `openid email` and nothing more: this module establishes *which account*, and has no reason
/// to read mail, contacts, or files. A scope that is not requested cannot be misused later by
/// code that finds a token lying around with it already granted.
pub const SCOPES: &str = "openid email";

/// Builds the URL the system browser is sent to.
///
/// Returned as a string rather than opened directly so that what is being asked for is
/// inspectable by a test — the scopes, the challenge method, the redirect — instead of
/// disappearing into a call to the OS.
pub fn authorization_url(
    config: &OAuthClientConfig,
    redirect_uri: &str,
    challenge: &PkceChallenge,
    state: &str,
) -> String {
    format!(
        "{AUTHORIZATION_ENDPOINT}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method={CODE_CHALLENGE_METHOD}&state={}",
        percent_encode(config.client_id()),
        percent_encode(redirect_uri),
        percent_encode(SCOPES),
        percent_encode(challenge.challenge()),
        percent_encode(state),
    )
}

/// Percent-encodes a query parameter value.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The provider
// ---------------------------------------------------------------------------

/// The name the refresh token is filed under in the platform credential store.
///
/// Namespaced alongside the device identity's own entry. The refresh token goes here and
/// **nowhere else** — not the database, not a config file, not a log line. `secret_store.rs` is
/// the single seam for this, which is what lets the decision be tested against an in-memory
/// fake rather than a real keychain.
pub const REFRESH_TOKEN_SECRET_NAME: &str = "google-account-refresh-token";

/// The account identifier is cached alongside the token so a restart knows which account is
/// signed in without a network round trip. It identifies; it authorizes nothing.
pub const ACCOUNT_ID_SECRET_NAME: &str = "google-account-id";

/// [`AuthProvider`] backed by Google, with the refresh token in the platform credential store.
///
/// # What this type deliberately cannot do
///
/// It holds no pairing store, takes none, and has no method that touches one. Signing out
/// clears the tokens this type owns and nothing else, which is what makes the capability's
/// "sign-out leaves pairings intact" scenario true by construction rather than by remembering
/// to be careful — there is no code path from here to a pairing.
pub struct GoogleAuthProvider<S: SecretStore> {
    secrets: S,
    /// The account identifier as last established, cached in memory so `current_account` does
    /// not consult the credential store on every call.
    cached_account: Mutex<Option<AccountId>>,
}

impl<S: SecretStore> GoogleAuthProvider<S> {
    pub fn new(secrets: S) -> Self {
        Self {
            secrets,
            cached_account: Mutex::new(None),
        }
    }

    /// Records a completed sign-in: the refresh token to the credential store, the account
    /// identifier alongside it.
    ///
    /// Takes the token rather than obtaining it, so that every decision about *where it is put*
    /// is testable without completing an OAuth exchange.
    pub fn record_session(&self, account: &AccountId, refresh_token: &str) -> AppResult<()> {
        self.secrets
            .set(REFRESH_TOKEN_SECRET_NAME, refresh_token.as_bytes())?;
        self.secrets
            .set(ACCOUNT_ID_SECRET_NAME, account.as_str().as_bytes())?;
        *self.cached_account.lock().expect("not poisoned") = Some(account.clone());
        Ok(())
    }

    /// The stored refresh token, used to obtain a fresh access token.
    ///
    /// Returns `Ok(None)` when there is genuinely no session, and `Err` when the store could
    /// not be consulted — the distinction `secret_store.rs` exists to preserve. A caller must
    /// not treat an unreachable keychain as a signed-out device: it would report the user
    /// signed out while their session is intact behind a locked store.
    pub fn stored_refresh_token(&self) -> AppResult<Option<String>> {
        match self.secrets.get(REFRESH_TOKEN_SECRET_NAME)? {
            SecretLookup::Found(bytes) => {
                let token = String::from_utf8(bytes).map_err(|_| AppError::Storage {
                    message: "the stored refresh token is not valid UTF-8".to_string(),
                })?;
                Ok(Some(token))
            }
            SecretLookup::NotFound => Ok(None),
        }
    }

    /// Loads the signed-in account from the credential store into the in-memory cache.
    ///
    /// A session is only considered present when **both** the account identifier and the
    /// refresh token are there. A half-written pair would otherwise report an account that
    /// cannot be refreshed, which the user would see as being signed in to something that
    /// never works.
    pub fn restore_session(&self) -> AppResult<Option<AccountId>> {
        let token = self.stored_refresh_token()?;
        let account = match self.secrets.get(ACCOUNT_ID_SECRET_NAME)? {
            SecretLookup::Found(bytes) => {
                String::from_utf8(bytes).map_err(|_| AppError::Storage {
                    message: "the stored account identifier is not valid UTF-8".to_string(),
                })?
            }
            SecretLookup::NotFound => {
                *self.cached_account.lock().expect("not poisoned") = None;
                return Ok(None);
            }
        };

        if token.is_none() {
            *self.cached_account.lock().expect("not poisoned") = None;
            return Ok(None);
        }

        let account = AccountId::new(account);
        *self.cached_account.lock().expect("not poisoned") = Some(account.clone());
        Ok(Some(account))
    }

    /// Forgets the account session.
    ///
    /// Removes the refresh token and the cached account identifier, and **touches nothing
    /// else**. Pairings are established by physical possession of both devices and recorded
    /// locally; the identity provider was never granted the authority to withdraw them, and a
    /// sign-out that revoked them would mean an expired token silently de-authorized a machine
    /// the user is standing in front of.
    pub fn sign_out(&self) -> AppResult<()> {
        self.secrets.delete(REFRESH_TOKEN_SECRET_NAME)?;
        self.secrets.delete(ACCOUNT_ID_SECRET_NAME)?;
        *self.cached_account.lock().expect("not poisoned") = None;
        Ok(())
    }

    /// The form fields for redeeming an authorization code.
    ///
    /// Built as data rather than sent, so the PKCE verifier's presence and the absence of any
    /// client secret are both assertable in a test. The HTTP call itself is the caller's, and
    /// is the part this repository cannot exercise.
    pub fn code_redemption_form(
        config: &OAuthClientConfig,
        code: &str,
        verifier: &PkceChallenge,
        redirect_uri: &str,
    ) -> Vec<(String, String)> {
        vec![
            ("client_id".to_string(), config.client_id().to_string()),
            ("code".to_string(), code.to_string()),
            // The half that never travelled with the authorization request. This is what an
            // attacker holding an intercepted code does not have.
            (
                "code_verifier".to_string(),
                verifier.verifier().to_string(),
            ),
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("redirect_uri".to_string(), redirect_uri.to_string()),
        ]
    }

    /// The form fields for exchanging a refresh token for a new access token.
    ///
    /// Also secret-free, for the same reason: a public client has none.
    pub fn refresh_form(config: &OAuthClientConfig, refresh_token: &str) -> Vec<(String, String)> {
        vec![
            ("client_id".to_string(), config.client_id().to_string()),
            ("grant_type".to_string(), "refresh_token".to_string()),
            ("refresh_token".to_string(), refresh_token.to_string()),
        ]
    }
}

impl<S: SecretStore> AuthProvider for GoogleAuthProvider<S> {
    fn current_account(&self) -> AppResult<Option<AccountId>> {
        Ok(self.cached_account.lock().expect("not poisoned").clone())
    }
}
