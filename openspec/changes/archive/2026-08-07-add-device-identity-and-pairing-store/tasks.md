## 1. Shared vectors first

Two things both implementations must agree on: how a device id follows from a key, and what a
pairing store decides. Both must fail red before any implementation exists.

- [x] 1.1 Create `shared/testvectors/device_id_derivation.json`: cases mapping a 32-byte verifying key (hex) to the expected `DeviceId` per design D3 (first 16 bytes of SHA-256 over the key bytes, hex-encoded). Include the all-zeroes key, a key of all `0xff`, and at least three keys differing in only one byte — those last prove the derivation actually depends on the whole key rather than a prefix.
- [x] 1.2 Create `shared/testvectors/pairing_store.json`: decision cases for `(pairings, sender) -> key or absent`. Must cover: a paired peer yields its key; an unpaired peer yields absent; a **revoked** peer yields absent (per design D5, not "present but rejected later"); a re-paired formerly-revoked peer yields its key again; and two peers where one is revoked and one is not, proving revocation is per-pairing rather than global.
- [x] 1.3 Add README sections for both files: the derivation formula and why it is derived rather than assigned (D3's self-certifying argument), and the store's absent-means-revoked rule with the D5 reason.
- [x] 1.4 Add runners for both to `desktop/src-tauri/tests/shared_vectors.rs`, with required-id guards listing the one-byte-difference cases and the revoked/re-paired pair.
- [x] 1.5 Mirror both runners in `mobile/test/shared/shared_vectors_test.dart` with the same guards ← (verify: both suites fail for missing implementation, not parse errors; the one-byte-difference cases produce genuinely different expected ids — if any two match, the derivation is broken and the vectors are wrong)

## 2. Rust — the secret store seam

- [x] 2.1 Add `keyring = "4.1"` to `desktop/src-tauri/Cargo.toml`.
- [x] 2.2 Create `desktop/src-tauri/src/platform/secret_store.rs`: a `SecretStore` trait with get / set / delete over a namespaced key, returning `AppResult`. Distinguish "not found" from "store unavailable" — per design D7 and the generate-once rule, conflating them is what would cause a silent regeneration.
- [x] 2.3 Implement `KeyringSecretStore` backed by `keyring`, and `InMemorySecretStore` for tests.
- [x] 2.4 Add tests against the in-memory fake covering round-trip, not-found, and delete ← (verify: the not-found and store-unavailable paths return distinguishable errors — assert on the variant, not on a string)

## 3. Rust — device identity

- [x] 3.1 Add the `rand_core` feature to `ed25519-dalek` in `Cargo.toml`. Scope its use to the identity module only — no other code path generates keys.
- [x] 3.2 Create `desktop/src-tauri/src/domain/device_identity.rs`: `DeviceIdentity` exposing `device_id()`, `verifying_key()`, and `sign(payload) -> Signature`. Per design D2 it must expose **no** way to read the private key — no `private_key()`, no `export()`, no `Serialize`, and a **hand-written** `Debug` printing the device id and `<sealed>` rather than a derived one.
- [x] 3.3 Implement `derive_device_id(verifying_key) -> DeviceId` per D3, satisfying the vectors from 1.1.
- [x] 3.4 Implement load-or-generate: read the identity record from the database; if absent, generate a keypair, write the private half to the `SecretStore` and the public half to the database. If the database says an identity exists but the secret store cannot produce it, return an error — **do not generate a replacement** (design D7 and the spec's explicit scenario).
- [x] 3.5 Add a source-text test in a **separate** file (not inline `mod tests` — self-match trap) asserting `device_identity.rs` contains no member returning private key material: fail on `fn private_key`, `fn export`, `fn secret_key`, `derive(Debug`, and `Serialize` ← (verify: adding `pub fn private_key()` turns the test red — do it, confirm, revert; and adding `#[derive(Debug)]` to the struct also turns it red)
- [x] 3.6 Add tests: first run generates; second run reuses the same id; a secret-store failure after the record exists is an error and generates nothing; a signature from `sign()` verifies against `verifying_key()`.

## 4. Rust — the pairing store

- [x] 4.1 Bump `SCHEMA_VERSION` 4 → 5 and add `CREATE TABLE IF NOT EXISTS pairings` (peer device id, verifying key, paired_at, revoked_at nullable) and `device_identity` (device id, verifying key, created_at). Additive only — alter no existing table.
- [x] 4.2 Create `desktop/src-tauri/src/data/pairing_store.rs` with: record a pairing; list active pairings; revoke by peer id; re-pair a revoked peer. Per design D4, revoking sets `revoked_at` rather than deleting, and re-pairing **clears `revoked_at` on the same row** rather than inserting a second — two rows for one peer would make authorization depend on read order.
- [x] 4.3 Implement `verifying_keys_from_store()` building the map `evaluate_command` needs, **excluding revoked pairings** per design D5 so a revoked peer is absent rather than present-and-rejected.
- [x] 4.4 Add tests: round-trip a pairing; revoke and confirm the key vanishes from the map; re-pair and confirm it returns; confirm exactly one row exists after revoke-then-repair; a migration test opening a v4 database and confirming it upgrades with existing rows intact ← (verify: the re-pair test asserts row count, not just that the key came back — a second row would pass a key-presence assertion while leaving the ambiguity D4 exists to prevent)

## 5. Rust — close the caller-supplied-key route

- [x] 5.1 Wire production code so the only place a `verifying_keys` map is constructed is `verifying_keys_from_store()`. Tests may still build maps directly.
- [x] 5.2 Add a source-text test asserting no production file outside the store module constructs a `verifying_keys` map, per design D1. Acknowledge in a comment that this is a composition-layer guard rather than a type-level one, and why (the rules must stay pure for the vectors).
- [x] 5.3 Add an end-to-end test: pair a peer, sign a command with that peer's key, route it through `FakeTransport`, confirm accepted. Then revoke, replay the same flow, confirm refused with `AuthenticityUnverified` ← (verify: the revoke half fails if 4.3 stops filtering revoked rows — try it, confirm red, revert)

## 6. Dart — mirror everything

- [x] 6.1 Add `flutter_secure_storage: ^11.0.0` to `mobile/pubspec.yaml`.
- [x] 6.2 Create `mobile/lib/platform/secret_store.dart` mirroring the trait shape, with a `flutter_secure_storage` implementation and an in-memory fake.
- [x] 6.3 Create `mobile/lib/domain/device_identity.dart` mirroring `device_identity.rs`, including the no-private-key-access property and a hand-written `toString` that does not print key bytes.
- [x] 6.4 Implement `deriveDeviceId` satisfying the same vectors — byte-identical to Rust.
- [x] 6.5 Add the two tables to `mobile/lib/data/tables.dart`, bump `schemaVersion` 3 → 4, and **append** to the existing `MigrationStrategy.onUpgrade` — do not replace it, or the `trigger_date` and `origin` migrations are lost.
- [x] 6.6 Run `dart run build_runner build --delete-conflicting-outputs`.
- [x] 6.7 Create `mobile/lib/data/pairing_store.dart` mirroring the Rust store, same revoke and re-pair semantics.
- [x] 6.8 Mirror the source-text test for private-key access, and every test from groups 3, 4, and 5 ← (verify: `flutter analyze` clean; the Dart migration test opens a genuine v3 database rather than a fresh one; a device id derived in Dart matches one derived in Rust for the same key — use the shared vectors, they are the proof)

## 7. Full verification

- [x] 7.1 `cd desktop/src-tauri && cargo test --lib`, `cargo test --test shared_vectors`, `cargo test --test end_to_end_authenticity`, `cargo clippy --all-targets -- -D warnings`.
- [x] 7.2 `cd mobile && flutter analyze` and `flutter test`.
- [x] 7.3 `cd desktop/src && node --test`.
- [x] 7.4 `git diff` every pre-existing vector file and confirm zero edits — this change adds two files and edits none.
- [x] 7.5 Confirm both schemas bumped exactly once, `capabilities/default.json` untouched, no new Tauri command, and no network dependency in either manifest.
- [x] 7.6 Grep the whole tree for accidental private-key exposure: no `println!`/`print`/`log` line takes a signing key, and no struct holding one derives `Debug` or `Serialize` ← (verify: all six commands green; exactly three dependencies added — `keyring`, `flutter_secure_storage`, and the `rand_core` feature; the platform secret-store implementations are **honestly reported as UNVERIFIED** on Windows, Linux, Android, and iOS in the completion report, per design D7 — do not claim otherwise)
