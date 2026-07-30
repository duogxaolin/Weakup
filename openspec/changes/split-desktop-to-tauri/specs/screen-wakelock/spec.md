## MODIFIED Requirements

### Requirement: Desktop keep-awake persists while app is backgrounded or in tray
On Windows, macOS, and Linux, the system SHALL keep the screen-awake assertion held while the app is minimized, hidden to the tray, or not the focused window, for as long as the job remains active. On the Rust desktop implementation the assertion SHALL be acquired through the OS-native mechanism for the build target: `SetThreadExecutionState` with `ES_DISPLAY_REQUIRED` on Windows, an `IOPMAssertion` of type `NoDisplaySleepAssertion` on macOS, and the freedesktop D-Bus idle-inhibition interface on Linux with a `systemd-inhibit` fallback.

#### Scenario: Desktop wakelock persists after hide-to-tray
- **WHEN** a `keepAwake` job is active and the user closes the main window (hiding it to the tray)
- **THEN** the screen-awake assertion SHALL remain held

#### Scenario: Assertion released when the job ends
- **WHEN** an active `keepAwake` job is cancelled, completes, or is paused
- **THEN** the system SHALL release the OS assertion, and SHALL NOT leave an assertion held after the last active `keepAwake` job ends

#### Scenario: Assertion release survives an unclean shutdown
- **WHEN** the desktop app process terminates while an assertion is held
- **THEN** the assertion SHALL not outlive the process, since it is owned by the process rather than registered globally

#### Scenario: Linux inhibition failure degrades honestly
- **WHEN** neither the D-Bus idle-inhibition interface nor `systemd-inhibit` is available on a Linux system
- **THEN** the system SHALL report keep-awake as unavailable with the reason, and SHALL NOT show a job as actively holding the screen awake when it is not
