# Screen Wakelock Specification

## Purpose

Covers the screen-awake assertion that backs a `keepAwake` job: acquiring it when the job becomes
active, releasing it on completion, cancellation, or pause, and re-acquiring it when the app is
relaunched while the job's window is still open. Overdue jobs are not re-asserted — they complete
under the reconciliation rule instead.

How long the assertion survives differs by platform, and this spec requires the difference be
stated rather than assumed. On Windows, macOS, and Linux it is held while the app is minimized or
hidden to the tray. On Android and iOS it is foreground-only, and the UI must say so instead of
claiming a background capability that does not exist. macOS carries a further limit: the assertion
prevents idle display sleep and cannot override a lid close, a user-chosen Sleep, or thermal or
low-battery sleep.
## Requirements
### Requirement: Wakelock acquired for the lifetime of an active keep-awake job
The system SHALL acquire the platform screen-awake assertion when a `keepAwake` job becomes active and SHALL release it when the job completes, is cancelled, or is paused.

#### Scenario: Wakelock acquired on job start
- **WHEN** a `keepAwake` job transitions to active
- **THEN** the system SHALL acquire the screen-awake assertion via `wakelock_plus`

#### Scenario: Wakelock released on job completion
- **WHEN** a `keepAwake` job's trigger condition is met (duration elapsed or absolute time reached) and the job has no indefinite trigger
- **THEN** the system SHALL release the screen-awake assertion

#### Scenario: Wakelock released on manual cancellation
- **WHEN** a user cancels an active `keepAwake` job
- **THEN** the system SHALL release the screen-awake assertion immediately

### Requirement: Wakelock re-asserted after app relaunch within an active window
If the app is killed and relaunched while a `keepAwake` job's window is still active (indefinite, or duration/absolute-time not yet elapsed), the system SHALL re-acquire the screen-awake assertion on relaunch without requiring the user to recreate the job.

#### Scenario: Relaunch during active duration window re-asserts wakelock
- **WHEN** the app is killed and relaunched while a `keepAwake` job with a `duration` trigger has time remaining
- **THEN** the system SHALL re-acquire the wakelock automatically on relaunch

#### Scenario: Relaunch after window has elapsed does not re-assert
- **WHEN** the app is killed and relaunched after a `keepAwake` job's target instant has passed
- **THEN** the system SHALL mark the job completed per the overdue-reconciliation rule and SHALL NOT re-acquire the wakelock

### Requirement: Mobile keep-awake is foreground-only and the UI states this limitation
On Android and iOS, the system SHALL state in the job creation and job detail UI that the screen will stay awake only while the app is in the foreground, and that backgrounding the app or locking the device ends the effect. The system SHALL NOT claim mobile background keep-awake capability that does not exist.

#### Scenario: iOS keep-awake job shows foreground-only notice
- **WHEN** a user on iOS creates or views a `keepAwake` job
- **THEN** the system SHALL display that the screen stays awake only while the app is open and in the foreground

#### Scenario: Android keep-awake job shows foreground-only notice absent foreground service
- **WHEN** a user on Android creates or views a `keepAwake` job and the foreground service is not running
- **THEN** the system SHALL display that the screen stays awake only while the app is in the foreground

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

### Requirement: macOS keep-awake cannot block user- or system-initiated sleep
The system SHALL state, on macOS, that the keep-awake assertion prevents idle display sleep only and cannot prevent user-initiated sleep (Apple menu, lid close), thermal emergency sleep, or low-battery sleep.

#### Scenario: macOS job detail shows sleep-override limitation
- **WHEN** a user on macOS views an active `keepAwake` job
- **THEN** the system SHALL display that closing the lid or choosing Sleep will still put the Mac to sleep despite the active job

