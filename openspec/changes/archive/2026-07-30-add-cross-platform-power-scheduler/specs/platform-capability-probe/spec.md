## ADDED Requirements

### Requirement: A single capability model drives all platform-dependent behavior
The system SHALL expose a `PlatformCapabilities` model, resolved once at app startup, that reports at minimum: whether power-off is supported, whether keep-awake persists in the background, whether autostart is available, and whether scheduling requires a runtime permission grant. All UI and scheduling logic SHALL branch on this model rather than on direct `Platform.isX` checks scattered across the codebase.

#### Scenario: Capability model resolved at startup
- **WHEN** the app starts
- **THEN** the system SHALL resolve `PlatformCapabilities` once and make it available to all consumers via a Riverpod provider

#### Scenario: UI queries capability, not raw platform identity
- **WHEN** the job creation screen decides whether to show the real power-off option or the reminder substitution
- **THEN** it SHALL query `PlatformCapabilities.supportsPowerOff`, not `Platform.isAndroid` or `Platform.isIOS` directly

### Requirement: Capability model is injectable for testing
The system SHALL allow `PlatformCapabilities` to be constructed explicitly (e.g. a fixed instance representing Android or iOS) so that platform-specific branches are unit-testable on any development machine, including one where the impossible-platform behavior cannot otherwise be exercised.

#### Scenario: Test exercises Android capability without an Android device
- **WHEN** a unit test constructs a `PlatformCapabilities` instance with `supportsPowerOff: false`
- **THEN** the job-creation validation logic under test SHALL reject a real power-off request and select the reminder substitution, matching actual Android behavior

### Requirement: Capability values match the verified platform matrix
The resolved capability values SHALL match the verified platform behavior: `supportsPowerOff` is true for Windows, macOS, and Linux and false for Android and iOS; `keepAwakePersistsInBackground` is true for Windows, macOS, and Linux, true for Android only while its foreground service is running, and false for iOS; `supportsAutostart` is true for Windows, macOS, and Linux and false for Android and iOS.

#### Scenario: Desktop capabilities report full support
- **WHEN** `PlatformCapabilities` resolves on Windows, macOS, or Linux
- **THEN** `supportsPowerOff`, `keepAwakePersistsInBackground`, and `supportsAutostart` SHALL all be true

#### Scenario: iOS capabilities report the impossible cases as false
- **WHEN** `PlatformCapabilities` resolves on iOS
- **THEN** `supportsPowerOff` SHALL be false, `keepAwakePersistsInBackground` SHALL be false, and `supportsAutostart` SHALL be false

#### Scenario: Android capabilities report power-off false, background keep-awake conditional
- **WHEN** `PlatformCapabilities` resolves on Android
- **THEN** `supportsPowerOff` SHALL be false and `keepAwakePersistsInBackground` SHALL be true only while the foreground service is active
