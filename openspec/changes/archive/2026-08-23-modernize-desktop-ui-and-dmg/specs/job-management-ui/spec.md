## ADDED Requirements

### Requirement: The dashboard presents an at-a-glance overview distinct from the job list
The dashboard view SHALL present a concise overview of current state — the keep-awake status
indicator, a short status detail, the next upcoming job with its live countdown when one exists,
and a direct path to the job-creation surface — without duplicating the full job list or the
creation form's controls. When no upcoming job exists, the dashboard SHALL say so in plain
language rather than showing an empty or stale countdown.

#### Scenario: Dashboard surfaces the next upcoming job
- **WHEN** at least one active job has a resolved future target instant and the user views the
  dashboard
- **THEN** the dashboard SHALL display that job's countdown, updating at least once per second
  while the dashboard is shown

#### Scenario: Dashboard offers a direct path to creation
- **WHEN** the user views the dashboard
- **THEN** the dashboard SHALL present a control that navigates to the job-creation view

#### Scenario: Dashboard states when nothing is scheduled
- **WHEN** no active job has a resolved future target instant and the user views the dashboard
- **THEN** the dashboard SHALL indicate in plain language that nothing is scheduled instead of
  showing a countdown
