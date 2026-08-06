# Background Runtime Specification

## Purpose

Defines how the app stays alive to run a pending job once the user is no longer looking at it, and
what each operating system permits in that state. On desktop this means closing the window hides
it to a tray icon rather than killing the process, with an explicit Quit that actually exits and an
optional launch-at-login. On mobile it means an Android foreground service that runs only while a
job is pending, and the permission prompts and OS-imposed limits that come with it.

Where an operating system cannot deliver a guarantee, the boundary is stated rather than papered
over: Android 15 caps `dataSync` service time and iOS gives no background timing guarantee at all,
so this capability requires those cases surface to the user instead of a job silently disappearing.
Deciding when a job fires is not defined here — that belongs to power-job-scheduling.

## Requirements

### Requirement: Desktop app hides to tray instead of quitting on window close
On Windows, macOS, and Linux, the system SHALL intercept the main window's close action and hide the window rather than terminating the process, keeping any active jobs running. A tray/menu-bar icon SHALL remain visible while the app runs in this hidden state.

#### Scenario: Closing the window hides it instead of quitting
- **WHEN** the user clicks the window close button on desktop
- **THEN** the system SHALL hide the window and keep the process running with the tray icon visible

#### Scenario: Active job continues after hide-to-tray
- **WHEN** a `keepAwake` or `powerOff` job is active and the window is hidden to tray
- **THEN** the job SHALL continue running unaffected

#### Scenario: Tray icon click restores the window
- **WHEN** the user clicks the tray icon
- **THEN** the system SHALL show and focus the main window

### Requirement: Tray quit action terminates the process, bypassing the close interception
The system SHALL provide a distinct "Quit" action in the tray menu that terminates the application process, using the mechanism that bypasses the close-interception (`destroy()`), since the standard close call is intercepted and would otherwise make Quit a no-op.

#### Scenario: Quit from tray menu actually exits
- **WHEN** the user selects "Quit" from the tray menu
- **THEN** the system SHALL terminate the process via `windowManager.destroy()` and SHALL NOT merely hide the window

### Requirement: Optional launch-at-startup on desktop
On Windows, macOS, and Linux, the system SHALL offer a user-toggleable setting to launch the app automatically at OS login. The setting SHALL be off by default.

#### Scenario: Enabling autostart registers with the OS
- **WHEN** the user enables the launch-at-startup setting
- **THEN** the system SHALL register the app with the platform's autostart mechanism (registry Run key on Windows, login item on macOS, `~/.config/autostart` entry on Linux)

#### Scenario: Disabling autostart deregisters
- **WHEN** the user disables the launch-at-startup setting
- **THEN** the system SHALL remove the corresponding autostart registration

#### Scenario: Autostart is off by default
- **WHEN** the app is installed and launched for the first time
- **THEN** the launch-at-startup setting SHALL be off

### Requirement: Android foreground service runs only while a job is pending
On Android, the system SHALL start a foreground service (with a visible notification) when a `keepAwake` or `powerOff` job becomes active, and SHALL stop the service when no job is active. The service SHALL declare the `dataSync` foreground service type per Android 14+ requirements.

#### Scenario: Foreground service starts when a job becomes active
- **WHEN** a job transitions to active on Android
- **THEN** the system SHALL start the foreground service with a visible notification indicating the app is running in the background

#### Scenario: Foreground service stops when no jobs remain active
- **WHEN** the last active job completes or is cancelled
- **THEN** the system SHALL stop the foreground service

#### Scenario: Foreground service declares required type and permission
- **WHEN** the foreground service starts on Android 14 (API 34) or later
- **THEN** the manifest SHALL declare `android:foregroundServiceType="dataSync"` and the `FOREGROUND_SERVICE_DATA_SYNC` permission, and the service SHALL pass the matching `serviceTypes` at runtime

### Requirement: Android 15 dataSync 6-hour cap is handled without silent job loss
On Android 15 (API 35) and later, if the foreground service reaches the 6-hour/day `dataSync` cap while a job is still pending, the system SHALL stop the service, mark the affected job as degraded, and notify the user rather than allowing the job to silently vanish.

#### Scenario: Service reaches daily cap with a job still pending
- **WHEN** the foreground service has run for 6 hours in the current day on Android 15+ and a job remains pending
- **THEN** the system SHALL stop the service, mark the job degraded, and display a notification explaining the OS-imposed limit

### Requirement: Android exact-alarm and notification permissions are requested at point of need
On Android 13+ (API 33+), the system SHALL check `canScheduleExactAlarms()` before scheduling an exact alarm and SHALL request the `SCHEDULE_EXACT_ALARM` permission (routing the user to system settings) only when the user creates a job that needs exact timing. The system SHALL request `POST_NOTIFICATIONS` at runtime before the first notification is scheduled. Permission SHALL be requested at the point of need, not at first app launch.

#### Scenario: Exact-alarm permission requested on first scheduled job
- **WHEN** a user on Android 13+ creates their first job requiring exact-time scheduling
- **THEN** the system SHALL check `canScheduleExactAlarms()` and, if denied, route the user to the system settings screen to grant it

#### Scenario: Exact-alarm permission denial degrades precision, not functionality
- **WHEN** the user declines to grant `SCHEDULE_EXACT_ALARM`
- **THEN** the system SHALL still create the job and rely on the in-process timer while the app is running, and SHALL state that timing may drift if the app is closed

#### Scenario: Notification permission requested before first scheduled notification
- **WHEN** the app is about to schedule its first local notification on Android 13+
- **THEN** the system SHALL request `POST_NOTIFICATIONS` at that point, not at app launch

### Requirement: iOS background execution has no timing guarantee and is stated as such
On iOS, the system SHALL rely on scheduled local notifications (not `BGTaskScheduler`) as the primary mechanism for delivering job-related events when the app is not in the foreground, and SHALL state in the UI that background timing is not guaranteed and that force-quitting the app stops all background delivery.

#### Scenario: iOS job creation states the background limitation
- **WHEN** a user on iOS creates a job with a duration or absolute-time trigger
- **THEN** the system SHALL state that the app must remain open (or in the background, not force-quit) for the job to fire reliably

#### Scenario: iOS notification queue limit is respected
- **WHEN** the number of pending scheduled notifications on iOS would exceed 64
- **THEN** the system SHALL prioritize the nearest-due notifications and SHALL NOT silently drop the soonest-due job's notification
