## 1. Config and dependencies

- [x] 1.1 Add `google-services.json` at `mobile/android/app/` and `GoogleService-Info.plist` at `mobile/ios/Runner/`, fetched for project `weakup-remote-2026`, package/bundle `vn.delify.weakup`. Confirm the Android file's `package_name` matches `applicationId` exactly — a mismatch fails at runtime with an unhelpful message.
- [x] 1.2 Add Dart dependencies: `firebase_core`, `cloud_firestore`, `firebase_messaging`, `google_sign_in`. Pin majors. **Check each against `compileSdk` 36 before adding** — `flutter_secure_storage` 11 was pinned back to 10.3.1 precisely because it demanded SDK 37, which has no released platform package. If any of these demand 37, pin to the last version that does not and record why in a comment beside the pin.
- [x] 1.3 Add Rust dependencies for HTTPS and JSON. `reqwest` with `rustls-tls` (not native-tls — it avoids a system OpenSSL dependency that differs per platform and breaks CI). No Firebase SDK exists for Rust; per design D8 the REST API is spoken directly.
- [x] 1.4 Verify `capabilities/default.json` is **unchanged** and the CSP still reads `default-src 'self'`. Per design D2 this is the point: the web view gains nothing ← (verify: `cargo build` succeeds on macOS; `flutter pub get` resolves; `flutter build apk --debug` still assembles — it does today, and a dependency demanding SDK 37 would break it)

## 2. Account identity — Rust

- [x] 2.1 Create `desktop/src-tauri/src/platform/google_auth.rs` implementing the existing `AuthProvider` trait. Sign-in opens the **system browser** and receives the redirect on a loopback listener bound to `127.0.0.1` on an ephemeral port, per design D3. Do not use an embedded web view — the user must be able to see the address bar.
- [x] 2.2 The listener accepts exactly one request and shuts down immediately. It must not outlive the sign-in.
- [x] 2.3 Store the refresh token in the existing `SecretStore`, never in the database or a config file. Reuse the seam from `platform/secret_store.rs` rather than adding a second storage path.
- [x] 2.4 Add a test asserting the account identity is not among the inputs to `evaluate_command` — the spec's "sign-in state is absent from the command decision" scenario. A source-text test in the existing style is adequate; put it in a separate file, not an inline `mod tests`.
- [x] 2.5 Add tests for token refresh and for sign-out leaving pairings intact ← (verify: signing out and back in does not require re-pairing; the loopback listener is bound to 127.0.0.1 and not 0.0.0.0 — assert on the bind address, do not eyeball)

## 3. Account identity — Dart

- [x] 3.1 Create `mobile/lib/platform/google_auth.dart` implementing the Dart `AuthProvider`, backed by `google_sign_in`.
- [x] 3.2 Mirror the secret-store usage for any long-lived token.
- [x] 3.3 Mirror the tests from 2.4 and 2.5 ← (verify: `flutter analyze` clean; no password field or flow exists anywhere in the sign-in surface — grep for it, the spec forbids one)

## 4. The transport — Rust

- [x] 4.1 Create `desktop/src-tauri/src/platform/firebase_transport.rs` implementing `RemoteTransport` over the Firestore REST API. Hand-written request/response types per design D8; every field this code depends on is parsed explicitly and a missing field is an error, never a silent default.
- [x] 4.2 Implement register-device, report-presence, list-devices, send-envelope, receive-envelopes. The envelope's signed content is carried as opaque bytes — this code must not parse, inspect, or validate it.
- [x] 4.3 Presence is reported every **60 seconds** per design D5, matching the 90-second online threshold the presence rule already uses. Put the interval in one named constant with a comment tying it to that threshold.
- [x] 4.4 Implement the Firestore listener so an arriving command is delivered without polling.
- [x] 4.5 Add a source-text test asserting `firebase_transport.rs` contains no reference that would let it assert authenticity — same forbidden-name check the transport trait already carries. Separate file, not inline.
- [x] 4.6 Add tests using a stubbed HTTP layer: a relay returning a fabricated command, a relay returning an altered command, and a relay returning malformed JSON. The first two must be refused on authenticity grounds; the third must be an error, not a panic ← (verify: the fabricated-command test routes through `evaluate_command` rather than asserting on the transport directly — the spec's claim is about the outcome, not the plumbing)

## 5. The transport — Dart

- [x] 5.1 Create `mobile/lib/platform/firebase_transport.dart` implementing the Dart `RemoteTransport` over `cloud_firestore`.
- [x] 5.2 Same operations, same opaque-bytes rule, same 60-second interval from a named constant.
- [x] 5.3 Wire `firebase_messaging` as a **wake-up signal only** per design D4 — the push carries no command content, and the Firestore listener is what delivers. A push that never arrives must degrade to higher latency, not to a missed command.
- [x] 5.4 Mirror the tests from 4.5 and 4.6 ← (verify: disabling push entirely still delivers commands through the listener — test it, this is the degradation path D4 claims)

## 6. Pairing flow

- [x] 6.1 Rust: generate a grant, present a six-character code from an alphabet excluding `0`/`O` and `1`/`I`/`l` per design D6, and accept a code presented back. Reuse the existing `evaluate_grant` — do not reimplement expiry or single-use.
- [x] 6.2 Rust: on success, record the peer in the existing pairing store. Per design D7 confirm the peer recorded us before recording them — a half-recorded pairing sends commands that are always refused with no visible reason.
- [x] 6.3 Dart: the same two halves, mirroring the Rust behaviour.
- [x] 6.4 Add tests: a code presented within its lifetime pairs both sides; an expired code pairs neither; a code presented twice pairs only once; an exchange that fails partway leaves neither side believing it is paired ← (verify: the failure-partway test asserts on **both** stores, not one — that is the whole point of D7)

## 7. Surfaces

- [x] 7.1 Desktop: new Tauri commands for sign-in, sign-out, present-pairing-code, accept-pairing-code, list-pairings, revoke-pairing, list-devices. **No new power-off command** — a remote request creates a job through the existing path, which is what keeps the countdown mandatory. The existing source-text test forbidding `pub fn power_off` must still pass.
- [x] 7.2 Desktop frontend: a pairing surface and a paired-devices list showing presence **derived by the existing presence rule** from each peer's last reported instant — never a bare online/offline flag from the relay.
- [x] 7.3 Desktop: a remote-control setting, defaulting to disabled, persisted, changeable only at the machine. Disabling must not revoke pairings.
- [x] 7.4 Mobile: pairing screen, paired-devices screen with presence, and controls to request a remote power-off or keep-awake on a paired desktop.
- [x] 7.5 Add i18n entries for every new string in both `desktop/src/i18n.js` tables — the existing test enforces exact key parity between locales.
- [x] 7.6 Extend `desktop/src/wiring.test.js`: every new control has a label pointing at it, every `data-i18n` key exists, and the remote-control setting is present in the settings surface ← (verify: `node --test` green; the i18n parity test passes, meaning no key was added to one locale only)

## 8. End to end, against the fake

- [ ] 8.1 A test that pairs two in-process devices through the **real pairing code path**, signs a command on one, routes it through `FakeTransport`, and confirms the other creates a remote-origin job with the 300-second countdown.
- [ ] 8.2 The same, with the pairing revoked first — refused with `AuthenticityUnverified`, since a revoked peer is absent from the key map.
- [ ] 8.3 The same, with remote control disabled at the target — refused with the disabled reason.
- [ ] 8.4 Confirm the hostile-transport tests still pass with the real pairing flow in place: duplicate, delay past freshness, reorder, drop ← (verify: 8.1 fails if the countdown is taken from the local constant instead of the origin — try it, confirm red, revert)

## 9. Full verification and honest reporting

- [ ] 9.1 `cd desktop/src-tauri && cargo test --lib`, `cargo test --test shared_vectors`, `cargo test --test end_to_end_authenticity`, `cargo clippy --all-targets -- -D warnings`.
- [ ] 9.2 `cd mobile && flutter analyze`, `flutter test`, and `flutter build apk --debug`.
- [ ] 9.3 `cd desktop/src && node --test`.
- [ ] 9.4 Confirm `capabilities/default.json` and the CSP are unchanged, no new Tauri power-off command exists, and no schema version changed.
- [ ] 9.5 Confirm every pre-existing shared vector file shows zero edits — this change adds no decision both implementations must share, so it should add no vectors either.
- [ ] 9.6 **Report honestly.** State plainly which paths are UNVERIFIED: OAuth completion on a real handset, FCM delivery to a sleeping desktop, secure-store behaviour outside macOS, and any real Firestore round trip. Do not write "works" for anything not run. Update each touched capability's Purpose to say what is now enforced and what still is not ← (verify: the completion report names every unverified path rather than reporting the suite as proof of the feature)
