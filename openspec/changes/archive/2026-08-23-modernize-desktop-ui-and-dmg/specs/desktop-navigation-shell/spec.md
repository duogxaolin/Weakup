## Purpose

Defines how the desktop client separates its primary surfaces into distinct, independently
navigable views instead of one long scrolling page, how the currently shown view is chosen and
announced to assistive technology, and the invariants that keep background timers and
source-level test contracts intact across navigation.

## ADDED Requirements

### Requirement: Primary surfaces are distinct hash-navigable views
The desktop client SHALL present its primary surfaces — dashboard, create, jobs, settings, and
remote — as distinct views, of which exactly one is shown at a time. The shown view SHALL be
determined by the URL fragment (`location.hash`), and navigating between views SHALL NOT reload
the document. When the fragment is empty or does not name a known view, the client SHALL show the
dashboard view.

#### Scenario: Selecting a sidebar destination shows only that view
- **WHEN** the user activates the sidebar link whose target is `#jobs`
- **THEN** the client SHALL show the jobs view and hide the dashboard, create, settings, and
  remote views

#### Scenario: Empty or unknown fragment falls back to dashboard
- **WHEN** the client loads with no fragment, or with a fragment that names no known view
- **THEN** the client SHALL show the dashboard view

#### Scenario: The creation surface is reachable from the sidebar
- **WHEN** the user views the sidebar
- **THEN** a navigation link whose target is the create view SHALL be present alongside the other
  primary destinations

### Requirement: The active view is indicated to assistive technology
The client SHALL mark the sidebar link corresponding to the currently shown view as the current
page in a way exposed to assistive technology, and SHALL NOT mark any other sidebar link as the
current page.

#### Scenario: Active link carries the current-page indication
- **WHEN** the jobs view is shown
- **THEN** the sidebar link targeting the jobs view SHALL expose an `aria-current="page"`
  indication and no other sidebar link SHALL expose it

#### Scenario: Indication moves on navigation
- **WHEN** the user navigates from the jobs view to the settings view
- **THEN** the current-page indication SHALL move to the settings link and be removed from the
  jobs link

### Requirement: All views remain mounted across navigation
The client SHALL keep every primary view present in the document at all times, hiding inactive
views rather than removing them, so that live countdowns and other per-second timers continue to
run for jobs that are not in the currently shown view.

#### Scenario: A countdown in a hidden view keeps advancing
- **WHEN** a job with a live countdown is shown in the jobs view and the user navigates to the
  settings view
- **THEN** that job's countdown SHALL continue updating so that returning to the jobs view shows
  a current value, not a stale one

### Requirement: The degraded-capability banner is outside the navigable views
The client SHALL present the degraded-capability banner independently of the view set, so it is
never a navigation destination and its visibility does not depend on which view is shown.

#### Scenario: Degraded banner is not a sidebar destination
- **WHEN** the sidebar is rendered
- **THEN** no sidebar navigation link SHALL target the degraded banner

#### Scenario: Degraded banner visibility is independent of the active view
- **WHEN** the degraded banner is shown and the user navigates between views
- **THEN** the banner SHALL remain shown regardless of the active view
