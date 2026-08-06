# Manual setup — the two steps no CLI can perform

Everything else in this change was provisioned from the command line. These two cannot be,
and the reason is deliberate on Google's part rather than a gap in tooling: creating OAuth
credentials automatically is exactly what a credential-harvesting attack would want to do.

Both were attempted and both failed in ways worth recording, so nobody repeats the attempt:

| Attempt | Result |
| --- | --- |
| `firebase auth:export` | `HTTP 400 CONFIGURATION_NOT_FOUND` — Authentication is not initialized |
| `firebase init` with `auth.providers.googleSignIn` | Reported `Successfully setup those features: auth`, but Auth stayed uninitialized and `oauth_client` stayed empty. **The success message is not true.** |
| `firebase apps:sdkconfig ANDROID` | `"oauth_client": []` — no client exists |
| `gcloud` | Not installed; in any case there is no API for creating OAuth 2.0 client credentials |

Until both steps below are done, sign-in cannot work. The code errors cleanly and names what
is missing rather than failing obscurely — that is the intended behaviour, not a bug.

## 1. Enable Google Sign-In

<https://console.firebase.google.com/project/weakup-remote-2026/authentication/providers>

Click **Get started** if prompted, then enable **Google**. Set the support email to
`duogxaolin@gmail.com`. This matches what `firebase.json` already declares:

```json
"auth": { "providers": { "anonymous": false, "emailPassword": false,
                         "googleSignIn": { ... } } }
```

`emailPassword: false` is intentional and required — the design is Google OAuth only, with no
password login anywhere in the product.

Enabling Google here creates the Android and iOS OAuth clients automatically. Re-running
`firebase apps:sdkconfig ANDROID ... --out mobile/android/app/google-services.json` afterwards
will populate the currently-empty `oauth_client` array. **Do that** — the file in the tree is
correct but incomplete until then.

## 2. Create a Desktop OAuth client for the Tauri app

<https://console.cloud.google.com/apis/credentials?project=weakup-remote-2026>

**Create credentials → OAuth client ID → Application type: Desktop app.**

A Desktop-type client is what the loopback redirect flow requires (design D3). It is a public
client and issues **no secret** — that is correct, not a misconfiguration. A client secret
embedded in a distributed desktop binary is not a secret, which is why the flow uses PKCE
instead.

Do not substitute the Firebase web app ID `1:683928241538:web:c30598008f0567a44bfb9b`. That is
a Firebase app ID and is not an OAuth client ID; they are different things.

Put the resulting ID where the desktop build reads it (see the path named in the sign-in
module's error message). That file is gitignored: it is per-developer configuration, and a
client ID committed to a public repository invites use of your quota by others.

## What is already done — do not redo

- Firestore API enabled, `(default)` database created in `asia-southeast1`
- `firestore.rules` compiled and released to `cloud.firestore`
- Android, iOS, and Web apps registered under `vn.delify.weakup`
- Debug keystore SHA-1 registered with the Android app
- `google-services.json` and `GoogleService-Info.plist` fetched with real values

## Still unwired, and known

`GoogleService-Info.plist` is present at `mobile/ios/Runner/` but is **not referenced by
`Runner.xcodeproj/project.pbxproj`**, so it will not be bundled and `FirebaseApp.configure()`
would fail at runtime on iOS. The fix is to open the project in Xcode and drag the file into
the Runner target, which writes the three required entries consistently. Hand-editing
`project.pbxproj` was deliberately avoided: a corrupted project file is a worse outcome than an
unwired plist, and iOS cannot be built or verified on the development machine in use.
