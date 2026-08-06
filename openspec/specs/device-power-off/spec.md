# Device Power Off Specification

## Purpose

Covers actually turning the machine off: the platform shutdown command behind an injectable
`PowerOffExecutor`, the mandatory 60-second cancellable countdown that precedes every execution,
and the specific error reported when the operating system refuses. The interface exists so the
schedule-to-execute path can be tested with a fake that records the call instead of shutting down
the developer's machine.

The capability's boundary is the set of platforms that can honour it. Windows, macOS, and Linux
perform a real power-off; Android and iOS cannot, and there the requirement is a labeled reminder
notification plus an upfront statement of the limitation, never a silent no-op. Every failure path
marks the job failed and shows the user why, rather than leaving it ambiguously pending.
## Requirements
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

#### Scenario: Test suite exercises the schedule-to-execute path without shutting down
- **WHEN** the job scheduler test suite runs a `powerOff` job to completion
- **THEN** it SHALL use a fake `PowerOffExecutor` that records the call instead of invoking the real OS shutdown command

### Requirement: Mandatory non-skippable grace-period countdown before execution
When a `powerOff` job's trigger condition is met, the system SHALL display a countdown with a
visible, always-enabled Cancel control before invoking `PowerOffExecutor`. The countdown duration
SHALL NOT be configurable to zero or disabled.

The countdown length depends on where the job came from, and on nothing else:

- A job scheduled at the machine SHALL use a 60-second countdown. This is the existing behavior
  and is unchanged.
- A job created by an authorized remote command SHALL use a longer countdown, fixed by the
  implementation and strictly greater than 60 seconds.

The remote case is longer because the 60-second figure assumes the person who scheduled the
power-off is at the machine and expecting it. For a remote request that assumption does not hold:
the person who will lose their work is not the person who asked, and may be in the middle of
something. The longer countdown is the compensation for that, and it SHALL apply to the person at
the machine regardless of who issued the command.

No caller SHALL be able to shorten, skip, or bypass either countdown. In particular a remote
command SHALL NOT be able to reduce the countdown, and the system SHALL expose no parameter by
which any caller could.

#### Scenario: Countdown displayed before shutdown command runs
- **WHEN** a `powerOff` job's target instant is reached
- **THEN** the system SHALL display a countdown with a Cancel control before calling the platform shutdown command

#### Scenario: Cancel during countdown aborts power-off
- **WHEN** the user activates Cancel during the countdown
- **THEN** the system SHALL abort the power-off, mark the job cancelled, and SHALL NOT invoke the platform shutdown command

#### Scenario: Countdown expires and executes
- **WHEN** the countdown elapses without cancellation
- **THEN** the system SHALL invoke `PowerOffExecutor.powerOff()`

#### Scenario: A locally scheduled power-off keeps the 60-second countdown
- **WHEN** a `powerOff` job scheduled at the machine reaches its target instant
- **THEN** the countdown SHALL be 60 seconds, unchanged from before remote control existed

#### Scenario: A remotely created power-off gets a longer countdown
- **WHEN** a `powerOff` job created by an authorized remote command reaches its target instant
- **THEN** the countdown SHALL be longer than 60 seconds, giving the person at the machine more time to refuse a shutdown they did not schedule

#### Scenario: Cancel at the machine overrides the remote requester
- **WHEN** the user at the target machine activates Cancel during the countdown of a remotely created power-off
- **THEN** the system SHALL abort the power-off and SHALL NOT invoke the platform shutdown command, regardless of the requesting device's authorization

#### Scenario: No caller can shorten the countdown
- **WHEN** the power-off countdown is invoked from any path, local or remote
- **THEN** the system SHALL expose no parameter that sets, shortens, or skips the duration, and the test suite SHALL fail if such a parameter is introduced

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

### Requirement: Android and iOS substitute a labeled reminder notification, never a silent no-op
On Android and iOS, the system SHALL NOT attempt to power off the device. Instead, when a `powerOff` job's trigger condition is met, the system SHALL fire a local notification explicitly labeled as a reminder (e.g. "Time to power off your device") and SHALL state, at job creation time, that the operating system does not allow apps to power off the device automatically.

#### Scenario: Android power-off job creation shows the limitation upfront
- **WHEN** a user on Android creates a `powerOff` job
- **THEN** the system SHALL state before creation that Android does not permit apps to power off the device and that a reminder notification will be sent instead

#### Scenario: iOS power-off job fires a reminder, not a shutdown attempt
- **WHEN** an iOS `powerOff` job's target instant is reached
- **THEN** the system SHALL fire a labeled reminder notification and SHALL NOT attempt any shutdown API call

#### Scenario: Mobile job list labels power-off jobs as reminders
- **WHEN** a user views their job list on Android or iOS
- **THEN** any `powerOff` job SHALL be visually and textually labeled as a reminder, distinct from an actual power-off action

### Requirement: Platform-denied power-off surfaces a specific, actionable error
When the platform shutdown command fails, the system SHALL report a specific, actionable error
rather than a generic failure message, distinguishing at minimum: Windows privilege denial
(`ERROR_PRIVILEGE_NOT_HELD`), macOS Automation consent denial, and Linux polkit denial.

Where a permission denial is detectable before the shutdown runs, the system SHALL report it at
scheduling time as well, using the same wording, so that the user is not first told at the
moment the shutdown fails.

#### Scenario: Windows privilege stripped by domain policy
- **WHEN** `shutdown /s /t 0` fails with `ERROR_PRIVILEGE_NOT_HELD`
- **THEN** the system SHALL display that the organization's policy prevents this app from
  shutting down the PC, not a generic error

#### Scenario: macOS Automation consent denied
- **WHEN** the user has denied the System Events Automation permission
- **THEN** the system SHALL display that permission was denied and SHALL show the System
  Settings > Privacy & Security > Automation path to re-enable it

#### Scenario: macOS Automation consent requested before first use
- **WHEN** a user on macOS creates their first `powerOff` job
- **THEN** the system SHALL explain, before the system consent dialog appears, that macOS will
  ask for permission to control System Events

#### Scenario: The pre-consent explanation cannot be raced by the dialog
- **WHEN** the user selects Power Off, which is what triggers the explanation
- **THEN** the interaction that reveals the explanation SHALL determine permission without
  prompting, and only a subsequent deliberate submit SHALL be permitted to raise the dialog,
  so the explanation cannot be preceded by the prompt it describes

#### Scenario: Linux polkit denies the request
- **WHEN** `systemctl poweroff` fails due to a polkit policy denial
- **THEN** the system SHALL display that the system's power policy blocked the shutdown request

### Requirement: Power-off failure never fails silently
For every `PowerOffExecutor` failure path, the system SHALL surface a visible error to the user and SHALL mark the job as failed rather than leaving it in an ambiguous pending or completed state.

#### Scenario: Failed shutdown updates job status
- **WHEN** the platform shutdown command returns a failure
- **THEN** the system SHALL set the job status to failed and display the specific error immediately, without requiring the user to check logs

### Requirement: Shutdown permission is requested when the schedule is created
Where the host requires a user-granted permission for an app to power the machine off, the
system SHALL determine that permission when the user creates a `powerOff` job, and SHALL NOT
defer the request to the moment the shutdown executes.

The determination SHALL NOT be made by sending a real shutdown or probe command. On macOS the
system SHALL use `AEDeterminePermissionToAutomateTarget`, which reports the consent that would
apply to an Apple Event without sending one.

The system SHALL distinguish three outcomes — granted, denied, and undetermined — and SHALL
block job creation only on an outright denial. An undetermined outcome SHALL allow the job to
be created, because "consent has not been decided yet" and "System Events is not currently
running" are not evidence that a shutdown will fail, and refusing to schedule on either would
deny a machine that works.

#### Scenario: Overnight schedule does not defer its consent prompt
- **WHEN** a macOS user schedules a power-off for a time hours in the future
- **THEN** the system SHALL determine Automation consent during the scheduling interaction,
  while the user is present to answer any dialog, rather than when the shutdown runs

#### Scenario: A refused consent blocks the schedule with an actionable reason
- **WHEN** the preflight reports that Automation consent is denied
- **THEN** the system SHALL refuse to create the `powerOff` job and SHALL show the System
  Settings > Privacy & Security > Automation path, rather than creating a job that cannot run

#### Scenario: An undetermined consent still allows scheduling
- **WHEN** the preflight reports that consent has not been decided, or that System Events is
  not running
- **THEN** the system SHALL allow the `powerOff` job to be created and SHALL surface the
  reason, rather than blocking the user

#### Scenario: The preflight cannot itself power the machine off
- **WHEN** any permission determination runs
- **THEN** it SHALL NOT hold a `PowerOffExecutor`, spawn a shutdown command, or send an Apple
  Event, so that no permission check can bypass the mandatory grace period

#### Scenario: A platform with no permission dialog reports undetermined
- **WHEN** the preflight runs on Windows or Linux, where shutdown permission is either held or
  not and no dialog exists to raise
- **THEN** the system SHALL report the outcome as undetermined and SHALL leave the
  authoritative answer to the classification of the actual shutdown command's result

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

