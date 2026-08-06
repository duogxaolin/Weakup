# Job Management UI Specification

## Purpose

Defines the surfaces through which a user creates, watches, and controls jobs: the creation form
that treats `keepAwake` and `powerOff` as independently selectable, the job list with its live
countdown and status labeling, and the pause, resume, and cancel controls available from both list
and detail views. Status is required to be legible on its own terms — text or icon, not inferred
from a moving countdown.

Accessibility is part of the capability rather than a later refinement: every control is reachable
and operable by keyboard alone, every element carries a screen-reader name, and contrast meets
WCAG 2.1 AA. This spec also owns the timing of honesty about platform limits, requiring that a
capability the current platform lacks is explained in plain language before the user confirms
creation, not after. What the underlying job does once created is specified elsewhere.

## Requirements

### Requirement: Job creation form supports independent selection of either or both job types
The system SHALL let the user configure a `keepAwake` job, a `powerOff` job, or both in one session, each with its own trigger configuration, and SHALL let the user activate them independently.

#### Scenario: User enables only keep-awake
- **WHEN** the user configures only a `keepAwake` job and confirms
- **THEN** the system SHALL activate the `keepAwake` job without creating a `powerOff` job

#### Scenario: User enables only power-off
- **WHEN** the user configures only a `powerOff` job and confirms
- **THEN** the system SHALL activate the `powerOff` job without creating a `keepAwake` job

#### Scenario: User enables both
- **WHEN** the user configures both a `keepAwake` job and a `powerOff` job and confirms
- **THEN** the system SHALL activate both jobs independently, each following its own trigger

### Requirement: Job list shows countdown, status, and type-specific labeling
The system SHALL display, for each job, its type, trigger description, live countdown to `targetInstantUtc` (or "indefinite" for that trigger kind), and current status (active, paused, completed, cancelled, failed, overdue). Status SHALL be conveyed through text and/or icon, not through the countdown alone.

#### Scenario: Active job shows live countdown
- **WHEN** a job is active with a resolved target instant
- **THEN** the job list SHALL display a countdown that updates at least once per second while visible

#### Scenario: Indefinite keep-awake job shows no countdown
- **WHEN** a `keepAwake` job has an `indefinite` trigger
- **THEN** the job list SHALL display "indefinite" or equivalent text instead of a countdown

#### Scenario: Status conveyed without relying on the countdown
- **WHEN** a job's status changes to failed
- **THEN** the system SHALL display a text label and/or icon indicating failure, independent of whatever the countdown shows

### Requirement: Every job control is reachable and operable by keyboard alone
All interactive elements in job creation, job list, job detail, and the power-off grace-period countdown SHALL be reachable via keyboard Tab order and operable via Enter/Space, with no functionality available only via mouse or touch.

#### Scenario: Job creation completed via keyboard only
- **WHEN** a user navigates the job creation form using only Tab, arrow keys, Enter, and Space
- **THEN** they SHALL be able to configure and confirm a job without using a mouse or touch input

#### Scenario: Power-off countdown Cancel is keyboard-operable
- **WHEN** the grace-period countdown is displayed
- **THEN** the Cancel control SHALL be focusable and activatable via keyboard, and SHALL be the initially focused element

### Requirement: All interactive elements have accessible labels and meet AA contrast
Every button, input, and status indicator SHALL expose a screen-reader-accessible name/label (e.g. Semantics on Flutter). Text and meaningful icon contrast against its background SHALL meet WCAG 2.1 AA (4.5:1 for normal text, 3:1 for large text/icons). All focusable elements SHALL have a visible focus indicator.

#### Scenario: Screen reader announces job status
- **WHEN** a screen reader user navigates to a job list item
- **THEN** the screen reader SHALL announce the job type, trigger description, and current status

#### Scenario: Focus indicator visible on all interactive elements
- **WHEN** a user tabs through the job creation form
- **THEN** each focused element SHALL show a visible focus outline meeting AA contrast against its background

### Requirement: Capability limitations are shown in plain language before and during job creation
When the current platform lacks a capability relevant to the job being configured (e.g. power-off on Android/iOS, background keep-awake on iOS), the system SHALL display a plain-language explanation before the user confirms creation, not only after the fact.

#### Scenario: Mobile power-off creation shows the limitation before confirmation
- **WHEN** a user on Android or iOS opens the power-off job creation form
- **THEN** the system SHALL display, before the Create button is used, that the OS does not allow apps to power off the device and that a reminder notification will be used instead

#### Scenario: iOS keep-awake creation shows the foreground-only limitation before confirmation
- **WHEN** a user on iOS opens the keep-awake job creation form
- **THEN** the system SHALL display, before the Create button is used, that the screen stays awake only while the app is open in the foreground

### Requirement: Job pause, resume, and cancel are available from the job list and detail views
The system SHALL let the user pause an active job (releasing the wakelock or suspending the pending shutdown without deleting the job), resume a paused job (recomputing the remaining time from now), and cancel a job (removing it) from both the job list and job detail view.

#### Scenario: Pausing a keep-awake job releases the wakelock
- **WHEN** the user pauses an active `keepAwake` job
- **THEN** the system SHALL release the screen-awake assertion and mark the job paused without deleting it

#### Scenario: Resuming a paused job recomputes remaining time
- **WHEN** the user resumes a paused `duration`-triggered job
- **THEN** the system SHALL recompute `targetInstantUtc` as now plus the originally configured duration

#### Scenario: Cancelling a job removes it and releases any held resources
- **WHEN** the user cancels a job
- **THEN** the system SHALL release any held wakelock or pending shutdown, stop the foreground service if no jobs remain, and remove the job from the active list
