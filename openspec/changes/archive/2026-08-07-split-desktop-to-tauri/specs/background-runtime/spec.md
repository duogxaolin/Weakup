## MODIFIED Requirements

### Requirement: Tray quit action terminates the process, bypassing the close interception
The system SHALL provide a distinct "Quit" action in the tray menu that terminates the application process, using the mechanism that bypasses the close-interception, since the standard close call is intercepted and would otherwise make Quit a no-op. On the Rust desktop implementation this SHALL be `AppHandle::exit`, invoked from the tray menu event handler.

#### Scenario: Quit from tray menu actually exits
- **WHEN** the user selects "Quit" from the tray menu
- **THEN** the system SHALL terminate the process rather than merely hiding the window

#### Scenario: Quit warns while a job is active
- **WHEN** the user selects "Quit" from the tray menu while a `keepAwake` or `powerOff` job is active
- **THEN** the system SHALL state that the active job will stop before terminating, and SHALL require confirmation

### Requirement: Optional launch-at-startup on desktop
On Windows, macOS, and Linux, the system SHALL offer a user-toggleable setting to launch the app automatically at OS login. The setting SHALL be off by default. The autostart mechanism SHALL be registered with the OS through a single initialization performed before any query or mutation of autostart state, and querying autostart state SHALL NOT fail when the feature has never been enabled.

#### Scenario: Enabling autostart registers with the OS
- **WHEN** the user enables the launch-at-startup setting
- **THEN** the system SHALL register the app with the platform's autostart mechanism (registry Run key on Windows, login item on macOS, `~/.config/autostart` entry on Linux)

#### Scenario: Disabling autostart deregisters
- **WHEN** the user disables the launch-at-startup setting
- **THEN** the system SHALL remove the corresponding autostart registration

#### Scenario: Autostart is off by default
- **WHEN** the app is installed and launched for the first time
- **THEN** the launch-at-startup setting SHALL be off

#### Scenario: Querying autostart state on a fresh install does not error
- **WHEN** the settings surface reads the current autostart state on a first launch, before autostart has ever been enabled
- **THEN** the system SHALL report the state as disabled and SHALL NOT raise an unsupported-operation error or leave the settings control in a permanently disabled loading state

### Requirement: Desktop app hides to tray instead of quitting on window close
On Windows, macOS, and Linux, the system SHALL intercept the main window's close action and hide the window rather than terminating the process, keeping any active jobs running. A tray/menu-bar icon SHALL remain visible while the app runs in this hidden state. On the Rust desktop implementation the tray icon SHALL be supplied as image data through the application handle rather than as a filesystem path, so that no per-OS icon file-format branching is required.

#### Scenario: Closing the window hides it instead of quitting
- **WHEN** the user clicks the window close button on desktop
- **THEN** the system SHALL hide the window and keep the process running with the tray icon visible

#### Scenario: Active job continues after hide-to-tray
- **WHEN** a `keepAwake` or `powerOff` job is active and the window is hidden to tray
- **THEN** the job SHALL continue running unaffected

#### Scenario: Tray icon click restores the window
- **WHEN** the user left-clicks the tray icon
- **THEN** the system SHALL unminimize, show, and focus the main window

#### Scenario: Tray icon renders on every desktop OS without per-OS icon paths
- **WHEN** the app starts on Windows, macOS, or Linux
- **THEN** the tray icon SHALL render from the same embedded icon source, and the system SHALL NOT select an icon file by extension per OS

## ADDED Requirements

### Requirement: Desktop runtime initialization failure never prevents the window from opening
The system SHALL treat tray creation, autostart registration, and notification initialization as independently failable. A failure in any of them SHALL be logged, SHALL mark the corresponding capability unavailable, and SHALL NOT abort application startup or prevent the main window from being shown.

#### Scenario: Tray creation fails but the app still opens
- **WHEN** tray icon creation fails at startup
- **THEN** the main window SHALL still open, and the UI SHALL state that the tray is unavailable and that closing the window will therefore quit the app

#### Scenario: Autostart registration fails but the app still opens
- **WHEN** autostart plugin initialization fails at startup
- **THEN** the main window SHALL still open, and the launch-at-startup control SHALL be shown as unavailable with the reason

#### Scenario: Startup does not abort on a recoverable init error
- **WHEN** any desktop runtime component fails during application setup
- **THEN** the setup routine SHALL complete successfully rather than returning an error that terminates the process before the window is created
