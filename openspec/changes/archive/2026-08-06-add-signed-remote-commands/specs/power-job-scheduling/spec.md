## MODIFIED Requirements

### Requirement: Jobs survive app restart and, where the OS permits, device reboot
The system SHALL persist all jobs to local storage such that they are recoverable after an app restart. On platforms where the OS permits background re-registration after device reboot (Windows, macOS, Linux, Android), the system SHALL re-register pending jobs after reboot. On iOS, where reboot re-registration is not possible, the system SHALL state this limitation to the user.

A job's origin — whether it was scheduled at this machine or created by an authorized remote command — SHALL be persisted with the job and SHALL be recovered unchanged. The countdown a power-off job receives is determined by its origin, so an origin that does not survive a restart silently converts a remote-initiated shutdown into one with the shorter local countdown. A job recovered from storage SHALL therefore receive exactly the countdown its original origin earns, not the countdown of a default.

Rows written before the origin was persisted SHALL be recovered as locally scheduled, which is what every such row meant when it was written.

#### Scenario: Pending job recovered after app restart
- **WHEN** the app is force-quit while a job is pending and then relaunched
- **THEN** the job SHALL appear in its prior state with its original `targetInstantUtc` intact

#### Scenario: Android job re-registers after device reboot
- **WHEN** an Android device reboots while a job is pending
- **THEN** the system SHALL re-register the job's scheduling via the boot-completed receiver without requiring the user to reopen the app first

#### Scenario: iOS states reboot limitation
- **WHEN** an iOS user views a pending job
- **THEN** the system SHALL state that the job will not survive a device reboot without reopening the app

#### Scenario: A remote-origin job keeps its origin across a restart
- **WHEN** a job created by an authorized remote command is persisted and then recovered after an app restart
- **THEN** it SHALL be recovered as remote-origin and SHALL receive the remote countdown, not the local one

#### Scenario: A job written before origin was persisted reads as local
- **WHEN** a job row written by an earlier version of the app, which stored no origin, is recovered
- **THEN** it SHALL be recovered as locally scheduled
