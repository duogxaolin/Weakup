## MODIFIED Requirements

### Requirement: Power-off execution is behind an injectable interface
The system SHALL express power-off execution as an injectable abstraction so that automated tests never invoke a real shutdown. On the Rust desktop implementation this SHALL be a trait object selected at construction time; the concrete OS executor SHALL be chosen by compile-time target, and the test suite SHALL bind a recording fake.

#### Scenario: Tests never invoke a real shutdown
- **WHEN** the automated test suite exercises any path that reaches power-off execution
- **THEN** the injected executor SHALL be a fake that records the invocation, and no real OS shutdown command SHALL be issued

#### Scenario: Concrete executor selected by target
- **WHEN** the application is built for Windows, macOS, or Linux
- **THEN** the executor for that OS SHALL be selected at compile time, and executors for other OSes SHALL NOT be compiled into the binary

#### Scenario: Unsupported target refuses rather than no-ops
- **WHEN** the application is built for a target with no real power-off capability
- **THEN** the executor SHALL return an explicit unsupported failure and SHALL NOT invoke any command

### Requirement: Windows, macOS, and Linux execute real power-off
On Windows, the system SHALL invoke `shutdown /s /t 0`. On macOS, the system SHALL invoke the System Events shutdown AppleScript via `osascript`. On Linux, the system SHALL invoke `systemctl poweroff`. Each invocation SHALL capture the process exit status and standard error so that failures can be classified.

#### Scenario: Windows shutdown command invoked
- **WHEN** the grace-period countdown expires on Windows
- **THEN** the system SHALL run `shutdown /s /t 0` and report success or the specific OS error

#### Scenario: macOS shutdown command invoked
- **WHEN** the grace-period countdown expires on macOS
- **THEN** the system SHALL run the AppleScript `tell application "System Events" to shut down` and report success or the specific OS error

#### Scenario: Linux shutdown command invoked
- **WHEN** the grace-period countdown expires on Linux
- **THEN** the system SHALL run `systemctl poweroff` and report success or the specific OS error

#### Scenario: Failure classification is driven by captured process output
- **WHEN** a shutdown command exits non-zero
- **THEN** the system SHALL classify the failure using the captured exit status and standard error rather than reporting a generic failure

## ADDED Requirements

### Requirement: macOS declares an Apple Events usage description and runs unsandboxed
The macOS desktop build SHALL declare an `NSAppleEventsUsageDescription` string explaining why the app controls System Events, and SHALL NOT enable the App Sandbox. Both conditions are required for the OS to present an Automation consent prompt instead of terminating the process on the Apple Events call.

#### Scenario: Consent prompt appears instead of process termination
- **WHEN** a macOS power-off is attempted for the first time
- **THEN** the operating system SHALL present the Automation consent prompt, and the application process SHALL NOT be terminated

#### Scenario: Usage description is present in the built app
- **WHEN** the macOS application bundle is built
- **THEN** its `Info.plist` SHALL contain a non-empty `NSAppleEventsUsageDescription` value

#### Scenario: Denied consent is reachable and reported
- **WHEN** the user denies the Automation consent prompt
- **THEN** the system SHALL report consent denial as a specific error with the System Settings path to re-enable it, rather than crashing or reporting a generic failure
