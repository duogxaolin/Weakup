# account-identity Specification

## Purpose
How a device establishes which account it belongs to, what that identity is for, and — the part most
easily got wrong — what it is emphatically not for.

Account identity answers "which devices might this person own", so a user can find their own machines
and address a message to one. It does not answer "may this device command that one". That question is
settled by pairing, and the authorization capability already refuses to treat same-account access as
sufficient. Keeping the two apart in the code is what keeps them apart in fact: a single component
answering both invites an implementation where a valid session is treated as authority, which is the
failure this system is built to prevent.

Enforcement state. This capability is the one place in the system where the central requirement is
**not exercised even once**, and that is stated first because the requirements below read as though
sign-in works.

**No account has ever been signed in, on either platform.** On the desktop, `sign_in_to_account`
returns an error unconditionally: the OAuth authorization-code exchange is not implemented, and there
is no OAuth client ID to implement it against. Creating one has no API — Google withholds it
deliberately, since automated creation of OAuth credentials is precisely what a credential-harvesting
attack wants — and `firebase init` with `auth.providers.googleSignIn` reports
`Successfully setup those features: auth` while leaving Authentication uninitialized and
`oauth_client` empty. That success message is false; both steps are recorded in the change's
MANUAL-SETUP.md and require a browser. On mobile, `google_sign_in` is wired but has never run: no
handset has completed the flow, and the iOS `GoogleService-Info.plist` is not referenced by
`project.pbxproj`, so it would not be bundled and `FirebaseApp.configure()` would fail before sign-in
was reached.

What is enforced today, and by construction rather than by care:

- **No password path exists anywhere.** Not merely absent from the UI — there is no password field,
  no password parameter, and no password-derived credential in either implementation.
  `firebase.json` declares `emailPassword: false`. A grep for a password field across the sign-in
  surface finds nothing, which is the check the requirement asks for.
- **Account identity is absent from the command decision.** `CommandTargetState` has no account,
  session, or token field, and a source-text test fails the build if one is introduced. This is the
  requirement that matters most and it is the one that *is* verified, because it is a structural
  property rather than a runtime one: a signed-in-but-unpaired device is refused because there is no
  input through which its session could be consulted.
- **Sign-out does not touch pairings.** The auth component holds no pairing store and has no method
  that reaches one; the Tauri command counts pairings before and after and errors if the count moved.
- **The account layer and the transport are separable.** Each is behind its own trait, and neither
  names the other.

What is therefore UNVERIFIED: the OAuth exchange itself, token refresh against a live token
endpoint, and every claim about what happens *after* a successful sign-in. The loopback listener's
bind address (`127.0.0.1`, never `0.0.0.0`), its single-request lifetime, the PKCE challenge, and the
choice of secure store for the refresh token are all asserted by tests — but they are the seam
around an exchange that has never occurred. Closing this gap requires a human with a browser to
perform the two MANUAL-SETUP.md steps; no further work on this machine can close it.
## Requirements
### Requirement: Account identity is established without the system handling a password
The system SHALL establish account identity through a federated sign-in flow operated by the identity
provider. It SHALL NOT accept, transmit, or store a password, and SHALL NOT offer a password-based
sign-in path.

A password this system never receives is one it cannot leak, cannot mishandle in a log, and cannot be
compelled to reset. The provider's flow also carries the provider's own protections — suspicious-login
checks, second factors the user has already configured — which this system could not reproduce.

#### Scenario: No password is accepted anywhere
- **WHEN** the sign-in surface is inspected
- **THEN** it SHALL offer no field or flow accepting a password, and the system SHALL hold no password-derived credential

#### Scenario: Sign-in is delegated to the provider
- **WHEN** a user signs in
- **THEN** the credential exchange SHALL occur with the identity provider, and this system SHALL receive only the resulting token

### Requirement: Account identity is used for discovery and routing, never for authority
The system SHALL use account identity only to determine which devices belong to the signed-in user
and to address messages to them. It SHALL NOT use account identity as an input to the decision of
whether a command is obeyed.

A device signed in to the account that owns a target, but not paired with it, SHALL be refused. The
system SHALL NOT provide any path by which possession of a valid account session alone results in a
command being carried out.

#### Scenario: A signed-in but unpaired device is refused
- **WHEN** a device holding a valid account session sends a command to a target of the same account with which it is not paired
- **THEN** the target SHALL refuse the command for the pairing reason

#### Scenario: Sign-in state is absent from the command decision
- **WHEN** the inputs to the command-acceptance decision are inspected
- **THEN** they SHALL contain no account identity, session token, or sign-in state

### Requirement: Losing the account session does not de-authorize a paired device
The system SHALL keep pairings independent of the account session. Signing out, a token expiring, or
the identity provider becoming unreachable SHALL NOT revoke a pairing or alter which peers a device
trusts.

Pairings are established by physical possession of both devices and are recorded locally. Tying them
to a session would mean an expired token silently de-authorized a machine the user is standing in
front of — and would make the identity provider able to withdraw authority it was never granted.

#### Scenario: Sign-out leaves pairings intact
- **WHEN** a user signs out on a device
- **THEN** the pairings that device holds SHALL remain recorded, and signing in again SHALL NOT require re-pairing

#### Scenario: An unreachable identity provider does not affect local decisions
- **WHEN** the identity provider cannot be reached
- **THEN** locally scheduled jobs, the countdown, and revocation SHALL continue to work unchanged

### Requirement: The account layer is replaceable without touching command transport
The system SHALL keep the component that establishes account identity separate from the component
that carries commands, such that either can be replaced without modifying the other.

#### Scenario: Replacing the identity provider leaves the transport untouched
- **WHEN** the account-identity implementation is replaced
- **THEN** the transport implementation SHALL require no change, and the reverse SHALL also hold

