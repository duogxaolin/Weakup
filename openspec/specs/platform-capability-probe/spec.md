# Platform Capability Probe Specification

## Purpose

Defines a single `PlatformCapabilities` model, resolved once at startup, that answers what the
current platform can actually do: whether power-off is supported, whether keep-awake survives
backgrounding, whether autostart exists, and whether scheduling needs a runtime permission grant.
It exists so platform-dependent behavior branches on a named capability rather than on
`Platform.isX` checks scattered through UI and scheduling code.

The model is constructible explicitly, which makes the impossible-platform branches unit-testable
on any development machine — Android and iOS behavior can be exercised without an Android or iOS
device. This spec fixes the capability values to the verified platform matrix; it does not define
what any consumer does in response to them.
## Requirements
### Requirement: A single capability model drives all platform-dependent behavior
The system SHALL expose a `PlatformCapabilities` model, resolved once at app startup, that reports at minimum: whether power-off is supported, whether keep-awake persists in the background, whether autostart is available, whether scheduling requires a runtime permission grant, and whether the device can act as a remote-control target. All UI and scheduling logic SHALL branch on this model rather than on direct `Platform.isX` checks scattered across the codebase.

#### Scenario: Capability model resolved at startup
- **WHEN** the app starts
- **THEN** the system SHALL resolve `PlatformCapabilities` once and make it available to all consumers via a Riverpod provider

#### Scenario: UI queries capability, not raw platform identity
- **WHEN** the job creation screen decides whether to show the real power-off option or the reminder substitution
- **THEN** it SHALL query `PlatformCapabilities.supportsPowerOff`, not `Platform.isAndroid` or `Platform.isIOS` directly

#### Scenario: Remote-command authorization queries the capability model
- **WHEN** the system decides whether a device can accept a remote command
- **THEN** it SHALL branch on the capability model rather than on a direct platform check, so the rule stays in one place

### Requirement: Capability model is injectable for testing
The system SHALL allow `PlatformCapabilities` to be constructed explicitly (e.g. a fixed instance representing Android or iOS) so that platform-specific branches are unit-testable on any development machine, including one where the impossible-platform behavior cannot otherwise be exercised.

#### Scenario: Test exercises Android capability without an Android device
- **WHEN** a unit test constructs a `PlatformCapabilities` instance with `supportsPowerOff: false`
- **THEN** the job-creation validation logic under test SHALL reject a real power-off request and select the reminder substitution, matching actual Android behavior

### Requirement: Capability values match the verified platform matrix
The resolved capability values SHALL match the verified platform behavior: `supportsPowerOff` is true for Windows, macOS, and Linux and false for Android and iOS; `keepAwakePersistsInBackground` is true for Windows, macOS, and Linux, true for Android only while its foreground service is running, and false for iOS; `supportsAutostart` is true for Windows, macOS, and Linux and false for Android and iOS.

A device can act as a remote-control target only if it can both keep a background presence and
perform the actions a remote command would ask for. Accordingly the remote-target capability
SHALL be true for Windows, macOS, and Linux, and false for Android and iOS — on those platforms
the operating system suspends the app and forbids power-off, so a command sent to them could not
be received reliably nor carried out.

This asymmetry is the point of the feature rather than a limitation of it: a phone is a useful
remote *controller* precisely because it need not be a *target*.

#### Scenario: Desktop capabilities report full support
- **WHEN** `PlatformCapabilities` resolves on Windows, macOS, or Linux
- **THEN** `supportsPowerOff`, `keepAwakePersistsInBackground`, and `supportsAutostart` SHALL all be true

#### Scenario: iOS capabilities report the impossible cases as false
- **WHEN** `PlatformCapabilities` resolves on iOS
- **THEN** `supportsPowerOff` SHALL be false, `keepAwakePersistsInBackground` SHALL be false, and `supportsAutostart` SHALL be false

#### Scenario: Android capabilities report power-off false, background keep-awake conditional
- **WHEN** `PlatformCapabilities` resolves on Android
- **THEN** `supportsPowerOff` SHALL be false and `keepAwakePersistsInBackground` SHALL be true only while the foreground service is active

#### Scenario: Desktop platforms can be remote-control targets
- **WHEN** `PlatformCapabilities` resolves on Windows, macOS, or Linux
- **THEN** the remote-target capability SHALL be true

#### Scenario: Mobile platforms cannot be remote-control targets
- **WHEN** `PlatformCapabilities` resolves on Android or iOS
- **THEN** the remote-target capability SHALL be false, and a remote command addressed to such a device SHALL be refused with the platform-capability reason rather than accepted and silently dropped

