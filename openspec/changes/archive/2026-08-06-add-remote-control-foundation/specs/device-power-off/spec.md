## MODIFIED Requirements

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
