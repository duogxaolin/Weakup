## ADDED Requirements

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

## MODIFIED Requirements

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
