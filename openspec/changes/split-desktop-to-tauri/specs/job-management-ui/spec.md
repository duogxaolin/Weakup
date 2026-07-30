## ADDED Requirements

### Requirement: Desktop UI is rendered in the Tauri web view over an explicit command surface
The desktop user interface SHALL be rendered in the Tauri web view and SHALL communicate with the Rust core exclusively through explicitly declared commands. The web view SHALL NOT contain scheduling, trigger-resolution, or power-off decision logic; it SHALL present state produced by the Rust core and forward user intent to it.

#### Scenario: Trigger resolution is not duplicated in the web view
- **WHEN** the user enters an absolute time in the desktop UI
- **THEN** the target instant SHALL be resolved by the Rust core, and the web view SHALL display the resolved value returned to it

#### Scenario: Countdown is derived from the persisted target instant
- **WHEN** the desktop UI displays a countdown for an active job
- **THEN** the displayed remaining time SHALL be computed from the job's absolute target instant, not from a value counted down independently in the web view

#### Scenario: Power-off cannot be initiated from the web view without the grace period
- **WHEN** the desktop UI requests a power-off job
- **THEN** the command surface SHALL NOT expose any command that executes power-off directly, and execution SHALL only occur through the core's grace-period countdown

### Requirement: Desktop UI states degraded runtime capabilities in plain language
The desktop UI SHALL display, in plain language, any capability that failed to initialize or that the OS has denied, together with the consequence for the user.

#### Scenario: Tray unavailable is stated with its consequence
- **WHEN** tray creation failed at startup
- **THEN** the UI SHALL state that the tray is unavailable and that closing the window will quit the app rather than hiding it

#### Scenario: Autostart unavailable is stated with its reason
- **WHEN** autostart initialization failed
- **THEN** the launch-at-startup control SHALL be shown as unavailable with the reason, rather than appearing operable or silently doing nothing

#### Scenario: Denied power-off consent is stated with the remedy
- **WHEN** the OS has denied a power-off request due to consent or privilege
- **THEN** the UI SHALL state the specific denial and the path to re-enable it, not a generic failure message
