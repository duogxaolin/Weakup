# Release pipeline — status and how to finish it

## What exists

`.github/workflows/release.yml` builds four platforms on a `v*.*.*` tag and attaches
everything to a GitHub Release:

| Job | Produces |
| --- | --- |
| `build-macos` | `.app`, `.dmg` |
| `build-windows` | `.msi`, `.exe` (NSIS) |
| `build-linux` | `.deb`, `.AppImage` |
| `build-android` | `.apk` — **new**, the previous release had no mobile artifact |

`publish-release` waits on all four and uploads their outputs.

`.github/workflows/ci.yml` gained two jobs that were missing: `flutter` (analyze with
`--fatal-infos`, plus 402 tests) and `end-to-end` (the 12 authenticity and 9
pairing-to-job tests). Before this, the mobile half of a cross-language codebase had
no CI at all, and the suites proving a remote command produces a 300-second countdown
ran only on a developer's machine.

## Tag pushed, workflow did not run — and why

`v0.2.0-rc.1` was tagged and pushed at 2026-08-06. No run started.

**This is not a configuration fault.** GitHub Actions was in a major outage from
15:22 UTC that day (githubstatus.com, incident "Incident with Actions", impact
critical, Actions and Pages both listed as major outage). GitHub was deliberately
throttling webhooks to recover, and their own status note said "many push and pull
request events are not triggering new workflow runs." A CI run queued three hours
earlier was still queued.

Evidence it is the outage and not the YAML:

- Both workflow files parse cleanly (`yaml.safe_load`), 6 and 5 jobs, no malformed steps.
- An earlier CI failure on this branch was `Failed to resolve action download info.
  Error: Service Unavailable` and `Bad Gateway` — infrastructure, not the build.
- Every command the new jobs run passes locally; see the numbers below.

### To finish once Actions recovers

```bash
# confirm the tag is still there and points at the workflow commit
git tag -l v0.2.0-rc.1
git rev-list -n1 v0.2.0-rc.1

# if no run appeared, re-push the tag to re-fire the trigger
git push --delete origin v0.2.0-rc.1
git push origin v0.2.0-rc.1

gh run list --workflow=release.yml --limit 3
gh release view v0.2.0-rc.1 --json assets --jq '.assets[].name'
```

Expect five desktop artifacts plus `weakup-0.2.0-rc.1-android-debug-key.apk`.

## Verified locally, not on a runner

Nothing below was executed by GitHub Actions. This is what passes on the development
machine, which is a weaker claim than a green pipeline:

| Check | Result |
| --- | --- |
| `cargo test --lib` | 534 |
| `cargo test --test shared_vectors` | 24 |
| `cargo test --test end_to_end_authenticity` | 12 |
| `cargo test --test end_to_end_pairing_to_job` | 9 |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `flutter analyze --fatal-infos` | clean |
| `flutter test` | 402 |
| `node --test` | 115 |
| `flutter build apk --release` | 61.4 MB, signed with APK scheme v2 |

The release APK was built and its signing block inspected: `APK Sig Block 42` is
present and scheme v2 is applied, so the artifact is genuinely signed. Which key
signed it could not be confirmed here — `apksigner` is not installed on this machine —
but with no `key.properties` in the tree, Gradle takes the debug branch, which is what
the release notes state.

## What could still fail on the first real run

Named rather than discovered later:

- **`subosito/flutter-action`** is third-party. Standard choice, not GitHub-maintained.
- **The Windows release step now uses `shell: bash`** so `${GITHUB_REF_NAME#v}` expands.
  Bash exists on Windows runners; this specific step has never executed.
- **Gradle may want an SDK component** the runner image does not ship. `ubuntu-latest`
  is Ubuntu 24.04, whose image lists `platforms;android-36` rev 2 and build-tools
  36.0.0 and 34.0.0 — checked against `actions/runner-images`, which covers
  `compileSdk 36` and the Firebase plugins that target 34. No speculative `sdkmanager`
  step was added, because one would mask which component actually went missing.
- **First-run cache misses** make the Android job slow once.

## Signing, and why the APK says `-debug-key`

Release signing activates only when four repository secrets are all present:
`ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`,
`ANDROID_KEY_PASSWORD`. Without them Gradle falls back to the Android debug key and
says so in the build log, the job summary, and the release notes, and the filename
carries `-debug-key` rather than `-upload-key`.

A debug-signed APK sideloads fine but is not publishable to the Play Store, identifies
no developer (the debug key ships with every Android SDK and is not secret), and will
not update over an install signed with a different key.

To sign for real:

```bash
keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA -keysize 2048 \
  -validity 10000 -alias upload
base64 -w0 upload-keystore.jks   # macOS: base64 -i upload-keystore.jks
```

Add the four secrets under **Settings → Secrets and variables → Actions**. Keep the
`.jks` out of the repository — `.gitignore` already covers `key.properties`, `*.jks`,
and `*.keystore`.

## No demo link, and no half-wired substitute

There is no hosted demo and no package-registry publication, because every route needs
a credential that was not available:

- `pubspec.yaml` sets `publish_to: 'none'`.
- Play Store needs an upload keystore **and** a service account.
- F-Droid needs reproducible unsigned builds.
- A hosted demo needs infrastructure, and the desktop app is a native binary that
  shuts computers down — a browser demo would not be the same program.

The GitHub Release APK is the honest distribution channel. Nothing was stubbed to make
the list look longer.

## Three version numbers still disagree

`tauri.conf.json` says 0.1.0, `Cargo.toml` 0.1.0, `pubspec.yaml` 1.0.0+1.

Release builds override all of them from the git tag (`--config '{"version":...}'` for
Tauri, `--build-name` for Flutter), so a `v0.2.0` tag produces 0.2.0 artifacts. This was
done via flags rather than by editing files, because `tauri.conf.json` had to stay
byte-identical for the Firebase change's D2 guarantee.

Consequence: a **development** build still reports 0.1.0 or 1.0.0. Reconciling the three
files is a separate change and has not been done.
