## MODIFIED Requirements

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

## ADDED Requirements

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
