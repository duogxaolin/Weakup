//! A [`RemoteTransport`] backed by Firestore's REST API.
//!
//! # What this module is, and what it deliberately is not
//!
//! This is plumbing. It writes documents, reads documents, and turns the bytes it finds
//! into [`CommandEnvelope`]s. It makes no judgement about any of them and has no way to
//! express one — the trait it implements offers no such member, and the source-text test in
//! `firebase_transport_source_tests.rs` fails the build if a name suggesting one ever
//! appears here.
//!
//! That absence is the whole reason a hosted relay is an acceptable place to run this
//! service. The relay stores opaque bytes it cannot read; the target checks the signature
//! itself against a key the relay never holds. A compromised relay can drop a command,
//! delay it, duplicate it, reorder two, or return content it invented outright. None of
//! those change what the target does, because [`evaluate_command`] recomputes the signature
//! check from the bytes rather than believing anything carried alongside them.
//!
//! # The signed content is opaque here
//!
//! [`envelope_to_document`] writes the sender, the command name, the creation instant, the
//! nonce, and the signature bytes, and [`document_to_envelope`] reads them back. Neither
//! inspects what they mean. The signature is carried as a Firestore `bytesValue` — base64
//! on the wire — and is never parsed, truncated, or normalised, because a byte changed in
//! transit must produce a *verification failure* at the target rather than a repair here.
//!
//! # Hand-written wire types (design D8)
//!
//! There is no Firebase SDK for Rust and no generated client. The types below are written
//! by hand for the four endpoints this needs, and **every field this code depends on is
//! parsed explicitly**: a missing field is an error, never a silent default. The trade-off
//! is stated in design D8 — a Firestore API change breaks at runtime rather than at compile
//! time — and explicit parsing is what turns that break into a named error instead of a
//! command that silently reads as something it is not.
//!
//! # The network lives here, not in the web view (design D2)
//!
//! This is the only module in the crate that makes an outbound request. The web view's
//! capability set carries no `http:` permission and its CSP remains `default-src 'self'`,
//! so the frontend cannot reach the network even if script execution is achieved there. A
//! future contributor reaching for the Firebase JavaScript SDK should read design D2 first.

use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Map, Value};

use crate::application::transport::{DevicePresenceRecord, RemoteTransport};
use crate::core::{AppError, AppResult};
use crate::domain::{CommandEnvelope, DeviceId, RemoteCommand};

/// How often a running device reports that it is present.
///
/// **This is tied to [`ONLINE_THRESHOLD_SECONDS`], which is 90.** That threshold was chosen
/// to tolerate one missed report on a 60-second heartbeat plus jitter; reporting on this
/// interval is what makes it mean what its own documentation says. Changing this number
/// without editing the presence rule would silently change what "online" means for every
/// device, in a file that does not mention presence.
///
/// Quota is the other constraint and it is not close: two devices at 60 seconds is roughly
/// 2,880 writes per day against a 20,000 free-tier limit. A 10-second interval would be
/// 17,280 and would leave almost nothing for commands.
///
/// [`ONLINE_THRESHOLD_SECONDS`]: crate::domain::ONLINE_THRESHOLD_SECONDS
pub const PRESENCE_REPORT_INTERVAL_SECONDS: u64 = 60;

/// A heartbeat at or beyond the online threshold would report a device as absent between
/// two consecutive successful reports, which is a false alarm rather than a detection.
///
/// Checked at compile time rather than in a test, in the same style as the presence rule's
/// own threshold ordering: a contributor who lengthens the interval past the threshold gets
/// a build error rather than a fleet of devices flickering offline.
const _: () = assert!(
    (PRESENCE_REPORT_INTERVAL_SECONDS as i64) < crate::domain::ONLINE_THRESHOLD_SECONDS,
    "a presence interval at or past the online threshold would make a healthy device \
     appear absent between two successful reports"
);

/// How long a single Firestore request may take before it is abandoned.
///
/// Bounded so a hung relay cannot wedge the caller indefinitely. A request that times out
/// is an ordinary failure: the command is not held for later, because a command that could
/// not be delivered now would arrive stale and be refused anyway.
const REQUEST_TIMEOUT_SECONDS: u64 = 30;

/// The collection holding one document per device in the account.
const DEVICES_COLLECTION: &str = "devices";

/// The collection holding opaque signed envelopes addressed to one device.
const COMMANDS_COLLECTION: &str = "commands";

// ---------------------------------------------------------------------------
// The HTTP seam.
// ---------------------------------------------------------------------------

/// One request to the relay, reduced to what this module actually needs.
///
/// A struct rather than a pile of parameters so the stub in the tests receives exactly what
/// the real client would, and a test asserting on a request cannot drift from what is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayRequest {
    /// `GET`, `POST`, `PATCH`, or `DELETE`.
    pub method: &'static str,
    /// The path below the project's document root, with no leading slash.
    pub path: String,
    /// The JSON body, absent for reads.
    pub body: Option<String>,
}

/// Where the bytes actually go.
///
/// Extracted as a trait for one reason: the hostile-relay tests in
/// `firebase_transport_tests.rs` need a relay that fabricates, alters, and malforms
/// responses, and they must run with no network. A stub implementing this returns whatever
/// a compromised relay could return, and the tests then assert on what the *target* does
/// with it — which is the property the spec actually claims.
pub trait RelayHttp: Send + Sync {
    /// Performs one request, returning the response body on success.
    ///
    /// A non-success status is an error rather than a body, because every caller here needs
    /// the distinction and none of them should have to remember to check.
    fn send(&self, request: &RelayRequest) -> AppResult<String>;
}

/// The real client, speaking HTTPS to Firestore.
///
/// Deliberately thin: it holds a token and a base URL and does no interpretation. Everything
/// that decides what a response *means* lives in the parsing functions below, so that the
/// meaning is exercised by tests that need no network.
///
/// # Why an async client behind a synchronous seam
///
/// [`RemoteTransport`] is synchronous, and it is not this module's place to change a shared
/// interface to suit one implementation of it. `reqwest` 0.13 gates its blocking client
/// behind a feature this build does not enable, so the async client is driven on a runtime
/// owned here. The runtime is a private detail of this struct: nothing above the seam learns
/// that a network call is asynchronous underneath, and the stub in the tests satisfies the
/// same synchronous trait with no runtime at all.
pub struct FirestoreHttp {
    client: reqwest::Client,
    /// The runtime the client's futures are driven on.
    ///
    /// Owned rather than borrowed from the caller so that this type is constructible from
    /// any thread, including one with no ambient runtime. Current-thread rather than
    /// multi-threaded: this drives at most one request at a time and a thread pool for that
    /// would be cost with no benefit.
    runtime: tokio::runtime::Runtime,
    /// `https://firestore.googleapis.com/v1/projects/{project}/databases/(default)/documents`
    base_url: String,
    /// The OAuth access token for the signed-in account.
    ///
    /// Supplied by the `AuthProvider` half of the design rather than obtained here. Account
    /// identity decides which devices a user can *see*; it is never what decides whether a
    /// command is obeyed, which is why this module takes a token and asks nothing about it.
    access_token: String,
}

impl FirestoreHttp {
    /// A client for one project, acting as one signed-in account.
    pub fn new(project_id: &str, access_token: impl Into<String>) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
            .build()
            .map_err(|error| AppError::storage(format!("relay client: {error}")))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| AppError::storage(format!("relay runtime: {error}")))?;

        Ok(Self {
            client,
            runtime,
            base_url: format!(
                "https://firestore.googleapis.com/v1/projects/{project_id}/databases/(default)/documents"
            ),
            access_token: access_token.into(),
        })
    }
}

impl RelayHttp for FirestoreHttp {
    fn send(&self, request: &RelayRequest) -> AppResult<String> {
        let url = format!("{}/{}", self.base_url, request.path);

        let mut builder = match request.method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PATCH" => self.client.patch(&url),
            "DELETE" => self.client.delete(&url),
            other => {
                return Err(AppError::storage(format!(
                    "relay request used an unsupported method {other:?}"
                )))
            }
        };

        builder = builder.bearer_auth(&self.access_token);

        if let Some(body) = &request.body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body.clone());
        }

        self.runtime.block_on(async move {
            let response = builder
                .send()
                .await
                .map_err(|error| AppError::storage(format!("relay request failed: {error}")))?;

            let status = response.status();
            let text = response.text().await.map_err(|error| {
                AppError::storage(format!("relay response unreadable: {error}"))
            })?;

            if !status.is_success() {
                // The body is included because Firestore explains a refusal there — a rules
                // denial and an expired token are different problems with different
                // remedies, and a bare status code tells the user neither.
                return Err(AppError::storage(format!(
                    "relay returned status {status}: {text}"
                )));
            }

            Ok(text)
        })
    }
}

// ---------------------------------------------------------------------------
// The transport.
// ---------------------------------------------------------------------------

/// Carries opaque envelopes between the account's devices through Firestore.
///
/// Holds no key, verifies nothing, and reports no verdict. See the module comment.
pub struct FirebaseTransport {
    http: Box<dyn RelayHttp>,
    /// Which device this instance speaks for.
    ///
    /// Presence reports are scoped to it: [`report_presence`] refuses to write a record for
    /// any other device, so this transport cannot make a peer appear present. The relay's
    /// own rules enforce the same thing; both exist because either alone would be a single
    /// point of failure for a property the spec states outright.
    ///
    /// [`report_presence`]: RemoteTransport::report_presence
    device_id: DeviceId,
    /// Envelope document names already consumed, so a listener and a poll cannot both
    /// deliver the same one twice within a session.
    ///
    /// Note what this is *not*: it is not replay defence. The nonce store in the acceptance
    /// rule is that, and it survives a restart. This only avoids pointless duplicate work,
    /// and the target would refuse a genuine duplicate regardless.
    consumed: Mutex<Vec<String>>,
}

impl FirebaseTransport {
    /// A transport speaking for `device_id` through `http`.
    pub fn new(device_id: DeviceId, http: Box<dyn RelayHttp>) -> Self {
        Self {
            http,
            device_id,
            consumed: Mutex::new(Vec::new()),
        }
    }

    /// The interval a caller should report presence on.
    ///
    /// Exposed as a function rather than left to the caller to remember, so the constant has
    /// exactly one reader and the comment tying it to the presence threshold is one hop from
    /// any call site.
    pub fn presence_interval() -> Duration {
        Duration::from_secs(PRESENCE_REPORT_INTERVAL_SECONDS)
    }

    /// Fetches the envelopes addressed to this device without polling for them.
    ///
    /// Firestore's `documents:listen` channel is what removes the need for a poll loop; this
    /// is the request that opens it. The delivered payload is the same document shape
    /// [`document_to_envelope`] already parses, so an arriving command travels the identical
    /// path as one that was read — a listener that parsed differently from a read would be a
    /// second place for the two to disagree.
    ///
    /// **NOT EXERCISED against a real relay.** No test in this repository signs in to
    /// Firestore or opens a real channel. What is tested is the request this builds and the
    /// parsing of what comes back.
    pub fn listen_request(&self) -> RelayRequest {
        RelayRequest {
            method: "POST",
            path: format!("{COMMANDS_COLLECTION}:listen"),
            body: Some(
                json!({
                    "addTarget": {
                        "query": {
                            "parent": "",
                            "structuredQuery": {
                                "from": [{ "collectionId": COMMANDS_COLLECTION }],
                                "where": {
                                    "fieldFilter": {
                                        "field": { "fieldPath": "target" },
                                        "op": "EQUAL",
                                        "value": { "stringValue": self.device_id.as_str() }
                                    }
                                }
                            }
                        }
                    }
                })
                .to_string(),
            ),
        }
    }

    /// Turns one `documents:listen` payload into the envelopes it carries.
    ///
    /// Separate from the request so the parsing is testable without a channel. A payload
    /// carrying no document change is not an error — the channel also emits keep-alives and
    /// target acknowledgements, and treating those as failures would make a healthy
    /// connection look broken.
    pub fn envelopes_from_listen_payload(&self, payload: &str) -> AppResult<Vec<CommandEnvelope>> {
        let value: Value = serde_json::from_str(payload)
            .map_err(|error| AppError::storage(format!("relay sent malformed JSON: {error}")))?;

        let Some(change) = value.get("documentChange") else {
            return Ok(Vec::new());
        };

        let Some(document) = change.get("document") else {
            return Ok(Vec::new());
        };

        let envelope = document_to_envelope(document)?;
        Ok(vec![envelope])
    }
}

impl RemoteTransport for FirebaseTransport {
    fn register_device(&self, device_id: &DeviceId, display_name: &str) -> AppResult<()> {
        // A device writes its own record. Being listed confers nothing — pairing is a
        // separate act — so this is a directory entry, not a grant.
        let body = json!({
            "fields": {
                "deviceId": { "stringValue": device_id.as_str() },
                "displayName": { "stringValue": display_name },
            }
        });

        self.http.send(&RelayRequest {
            method: "PATCH",
            path: format!(
                "{DEVICES_COLLECTION}/{}?updateMask.fieldPaths=deviceId\
                 &updateMask.fieldPaths=displayName",
                device_id.as_str()
            ),
            body: Some(body.to_string()),
        })?;

        Ok(())
    }

    fn report_presence(&self, device_id: &DeviceId, at: DateTime<Utc>) -> AppResult<()> {
        // A device reports only its own presence. Refusing here rather than trusting the
        // relay's rules to refuse means the property holds even against a relay whose rules
        // failed entirely — the same argument the acceptance rule makes about signatures.
        if device_id != &self.device_id {
            return Err(AppError::validation(
                "A device can report only its own presence.",
            ));
        }

        // An instant, never a state. Whether this counts as online is decided from this
        // value by the presence rule, at whichever device is asking.
        let body = json!({
            "fields": {
                "lastReportedAt": {
                    "timestampValue": at.to_rfc3339_opts(SecondsFormat::Millis, true)
                },
            }
        });

        self.http.send(&RelayRequest {
            method: "PATCH",
            path: format!(
                "{DEVICES_COLLECTION}/{}?updateMask.fieldPaths=lastReportedAt",
                device_id.as_str()
            ),
            body: Some(body.to_string()),
        })?;

        Ok(())
    }

    fn list_devices(&self) -> AppResult<Vec<DevicePresenceRecord>> {
        let response = self.http.send(&RelayRequest {
            method: "GET",
            path: DEVICES_COLLECTION.to_string(),
            body: None,
        })?;

        parse_device_list(&response)
    }

    fn send_envelope(&self, target: &DeviceId, envelope: &CommandEnvelope) -> AppResult<()> {
        let document = envelope_to_document(target, envelope);

        self.http.send(&RelayRequest {
            method: "POST",
            path: COMMANDS_COLLECTION.to_string(),
            body: Some(document.to_string()),
        })?;

        Ok(())
    }

    fn receive_envelopes(&self, device_id: &DeviceId) -> AppResult<Vec<CommandEnvelope>> {
        let query = json!({
            "structuredQuery": {
                "from": [{ "collectionId": COMMANDS_COLLECTION }],
                "where": {
                    "fieldFilter": {
                        "field": { "fieldPath": "target" },
                        "op": "EQUAL",
                        "value": { "stringValue": device_id.as_str() }
                    }
                }
            }
        });

        let response = self.http.send(&RelayRequest {
            method: "POST",
            path: ":runQuery".to_string(),
            body: Some(query.to_string()),
        })?;

        let value: Value = serde_json::from_str(&response)
            .map_err(|error| AppError::storage(format!("relay sent malformed JSON: {error}")))?;

        let rows = value
            .as_array()
            .ok_or_else(|| AppError::storage("relay returned a query result that is not an array"))?;

        let mut envelopes = Vec::new();
        let mut consumed = self.consumed.lock().expect("firebase transport lock");

        for row in rows {
            // A row without a document is Firestore reporting "no more results", not a
            // fault. Parsed explicitly so it is distinguishable from a document whose shape
            // is wrong, which *is* a fault.
            let Some(document) = row.get("document") else {
                continue;
            };

            let name = document
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            if !name.is_empty() && consumed.contains(&name) {
                continue;
            }

            // A malformed envelope is an error rather than a skip. Silently dropping one
            // would make a relay that corrupts a single command indistinguishable from one
            // that delivered nothing, and the user would see a command vanish with no
            // reason given.
            let envelope = document_to_envelope(document)?;

            if !name.is_empty() {
                consumed.push(name);
            }
            envelopes.push(envelope);
        }

        Ok(envelopes)
    }
}

// ---------------------------------------------------------------------------
// Wire types, hand-written (design D8).
// ---------------------------------------------------------------------------

/// Encodes one envelope as a Firestore document.
///
/// The signature travels as a `bytesValue` — base64 on the wire, exactly the bytes given.
/// Nothing here interprets it, and nothing repairs it: a byte altered in transit must fail
/// verification at the target rather than be normalised into something that passes.
pub fn envelope_to_document(target: &DeviceId, envelope: &CommandEnvelope) -> Value {
    json!({
        "fields": {
            "target": { "stringValue": target.as_str() },
            "sender": { "stringValue": envelope.sender.as_str() },
            "command": { "stringValue": envelope.command.as_str() },
            "createdAt": {
                "timestampValue": envelope.created_at.to_rfc3339_opts(SecondsFormat::Millis, true)
            },
            "nonce": { "stringValue": envelope.nonce },
            "signature": { "bytesValue": base64_encode(&envelope.signature) },
        }
    })
}

/// Reads one Firestore document back into an envelope.
///
/// **Every field is required.** A missing one is an error, never a default — design D8's
/// whole argument. A defaulted `createdAt` would make a stale command look fresh, a
/// defaulted nonce would collide with every other defaulted nonce, and a defaulted empty
/// signature would fail verification for a reason the user could not act on.
pub fn document_to_envelope(document: &Value) -> AppResult<CommandEnvelope> {
    let fields = document
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| missing("fields"))?;

    let sender_value = string_field(fields, "sender")?;
    let sender = DeviceId::new(sender_value)?;

    let command_value = string_field(fields, "command")?;
    let command = RemoteCommand::from_str_value(&command_value).ok_or_else(|| {
        // An unknown command name is refused rather than mapped to something plausible.
        // Guessing here would let a relay steer a command towards a different action.
        AppError::storage(format!(
            "relay sent an unrecognised command name {command_value:?}"
        ))
    })?;

    let created_at_value = fields
        .get("createdAt")
        .and_then(|value| value.get("timestampValue"))
        .and_then(Value::as_str)
        .ok_or_else(|| missing("createdAt"))?;

    let created_at = DateTime::parse_from_rfc3339(created_at_value)
        .map_err(|error| {
            AppError::storage(format!(
                "relay sent an unparseable createdAt {created_at_value:?}: {error}"
            ))
        })?
        .with_timezone(&Utc);

    let nonce = string_field(fields, "nonce")?;

    let signature_value = fields
        .get("signature")
        .and_then(|value| value.get("bytesValue"))
        .and_then(Value::as_str)
        .ok_or_else(|| missing("signature"))?;

    let signature = base64_decode(signature_value).ok_or_else(|| {
        AppError::storage("relay sent a signature that is not valid base64".to_string())
    })?;

    Ok(CommandEnvelope {
        sender,
        command,
        created_at,
        nonce,
        signature,
    })
}

/// Reads a `documents.list` response into presence records.
///
/// The `lastReportedAt` field is the one field here that may legitimately be absent: a
/// device that has registered but never reported has no instant, and `None` says exactly
/// that. Substituting a distant past instant would make a brand-new device look stale, and
/// substituting `now` would make one that has never spoken look present.
pub fn parse_device_list(response: &str) -> AppResult<Vec<DevicePresenceRecord>> {
    let value: Value = serde_json::from_str(response)
        .map_err(|error| AppError::storage(format!("relay sent malformed JSON: {error}")))?;

    // An empty collection omits `documents` entirely rather than sending an empty array.
    let Some(documents) = value.get("documents") else {
        return Ok(Vec::new());
    };

    let documents = documents
        .as_array()
        .ok_or_else(|| AppError::storage("relay returned a device list that is not an array"))?;

    let mut records = Vec::new();

    for document in documents {
        let fields = document
            .get("fields")
            .and_then(Value::as_object)
            .ok_or_else(|| missing("fields"))?;

        let device_id = DeviceId::new(string_field(fields, "deviceId")?)?;
        let display_name = string_field(fields, "displayName")?;

        let last_reported_at = match fields
            .get("lastReportedAt")
            .and_then(|value| value.get("timestampValue"))
        {
            None => None,
            Some(raw) => {
                let text = raw.as_str().ok_or_else(|| {
                    AppError::storage("relay sent a lastReportedAt that is not a string")
                })?;
                let parsed = DateTime::parse_from_rfc3339(text)
                    .map_err(|error| {
                        AppError::storage(format!(
                            "relay sent an unparseable lastReportedAt {text:?}: {error}"
                        ))
                    })?
                    .with_timezone(&Utc);
                Some(parsed)
            }
        };

        records.push(DevicePresenceRecord {
            device_id,
            display_name,
            last_reported_at,
        });
    }

    Ok(records)
}

/// Reads a required `stringValue` field.
fn string_field(fields: &Map<String, Value>, name: &str) -> AppResult<String> {
    fields
        .get(name)
        .and_then(|value| value.get("stringValue"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| missing(name))
}

/// The error for a field the relay did not send.
///
/// Named rather than inline so every absence reads the same way in a log, and so the
/// "missing is an error" rule is one function rather than a habit.
fn missing(field: &str) -> AppError {
    AppError::storage(format!(
        "relay sent a document with no {field}; every field this code depends on is \
         required, because a defaulted one would be indistinguishable from a real value"
    ))
}

// ---------------------------------------------------------------------------
// Base64, for the signature's `bytesValue`.
// ---------------------------------------------------------------------------

/// The standard alphabet, with padding, as Firestore emits and accepts.
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes bytes for a Firestore `bytesValue`.
///
/// Hand-written rather than pulling in a crate for forty lines, consistent with D8's stance
/// on this module's dependencies.
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(BASE64_ALPHABET[(triple >> 18) as usize & 0x3F] as char);
        out.push(BASE64_ALPHABET[(triple >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALPHABET[(triple >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALPHABET[triple as usize & 0x3F] as char
        } else {
            '='
        });
    }

    out
}

/// Decodes a Firestore `bytesValue`, or `None` if it is not valid base64.
///
/// `None` rather than a panic or a partial result: these bytes are attacker-controlled in
/// the threat model — a relay chooses them — and a panic on attacker-controlled input is a
/// denial of service against the machine being defended.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    fn sextet(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some((byte - b'A') as u32),
            b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
            b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let bytes = text.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }

    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);

    for chunk in bytes.chunks(4) {
        let padding = chunk.iter().filter(|byte| **byte == b'=').count();
        if padding > 2 {
            return None;
        }

        let mut triple = 0u32;
        for (index, byte) in chunk.iter().enumerate() {
            let value = if *byte == b'=' {
                // Padding is only legal at the end of the final chunk.
                if index < 4 - padding {
                    return None;
                }
                0
            } else {
                sextet(*byte)?
            };
            triple = (triple << 6) | value;
        }

        out.push((triple >> 16) as u8);
        if padding < 2 {
            out.push((triple >> 8) as u8);
        }
        if padding < 1 {
            out.push(triple as u8);
        }
    }

    Some(out)
}
