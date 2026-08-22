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
The system SHALL expose a `PlatformCapabilities` model, resolved once at app startup, that reports at minimum: whether power-off is supported, whether keep-awake persists in the background, whether autostart is available, and whether scheduling requires a runtime permission grant. All UI and scheduling logic SHALL branch on this model rather than on direct per-platform identity checks scattered across the codebase. The model SHALL additionally distinguish a capability that is *possible on this target* from one that is *currently permitted*, so that consent- and privilege-gated operations are reported accurately.

#### Scenario: Capability model resolved at startup
- **WHEN** the app starts
- **THEN** the system SHALL resolve `PlatformCapabilities` once and make it available to all consumers

#### Scenario: UI queries capability, not raw platform identity
- **WHEN** the job creation surface decides whether to show the real power-off option or the reminder substitution
- **THEN** it SHALL query the capability model's power-off support flag, not a direct platform identity check

#### Scenario: Possible-but-not-yet-permitted is distinguishable from impossible
- **WHEN** the desktop build reports power-off support before any consent or privilege check has occurred
- **THEN** the model SHALL report power-off as possible on this target, distinctly from a target where power-off is impossible, and SHALL NOT claim it is already permitted

#### Scenario: Degraded runtime capability is reflected in the model
- **WHEN** a desktop runtime component such as the tray or autostart fails to initialize
- **THEN** the capability model SHALL report that capability as unavailable, and the UI SHALL state the reason

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

### Requirement: Reported power-off availability accounts for permission, not only platform support
Where the host can report whether this application is *permitted* to power the machine off,
the capability report for power-off SHALL reflect that permission and not merely whether the
platform supports shutdown at all.

This distinction is load-bearing. A platform-support check answers "can any app shut this
machine down", which is a constant per OS, and reporting it alone caused a machine whose
Automation consent had been refused to be reported as fully capable — the user's first
indication of the problem was a shutdown that silently did not happen.

An outright permission denial SHALL degrade the power-off capability with a reason written for
a person. An undetermined permission SHALL NOT degrade it, because a warning shown on every
machine that has simply never been asked would train the user to ignore the banner that
matters.

Determining permission at startup SHALL NOT raise a consent dialog. Startup reports; the
prompt belongs to the scheduling interaction, where the user has context for it.

#### Scenario: A refused permission degrades the reported capability
- **WHEN** the host reports that this app is not permitted to power the machine off
- **THEN** the capability report SHALL mark power-off unavailable, with a reason, and the UI
  SHALL show it as a degradation of an essential capability

#### Scenario: An undetermined permission leaves the capability available
- **WHEN** permission has not yet been decided, or could not be checked
- **THEN** the capability report SHALL continue to report power-off as available, so that no
  warning is shown for a machine that has merely never been asked

#### Scenario: Startup does not prompt
- **WHEN** the app starts and probes power-off permission
- **THEN** it SHALL request a report only, and SHALL NOT raise a consent dialog before the user
  has expressed any intent to schedule a shutdown

#### Scenario: A platform-impossible power-off is still reported as unsupported
- **WHEN** the host has no way to power the machine off at all
- **THEN** the capability report SHALL mark power-off unavailable on that basis, without
  consulting permission, since permission is meaningless for an action that cannot occur

### Requirement: Desktop capability possibility is resolved at compile time
On the Rust desktop implementation, whether an operation is possible for the build target SHALL be determined at compile time by target OS, and code paths for other target OSes SHALL NOT be compiled into the binary. Whether a possible operation is permitted SHALL be determined at runtime from the operation's failure classification, not predicted in advance.

#### Scenario: Only the current OS's executor is compiled
- **WHEN** the desktop app is built for one target OS
- **THEN** only that OS's power-off, autostart, and keep-awake implementations SHALL be present in the binary

#### Scenario: Permission state is learned from failure, not predicted
- **WHEN** power-off is possible on the target but the OS denies the request at execution time
- **THEN** the system SHALL classify the denial from the actual failure and update what it reports to the user, rather than having pre-declared the operation permitted or forbidden

#### Scenario: Capability values match the verified desktop matrix
- **WHEN** capabilities resolve on Windows, macOS, or Linux
- **THEN** power-off SHALL be reported possible, keep-awake SHALL be reported as persisting in the background, and autostart SHALL be reported available

