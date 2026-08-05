## ADDED Requirements

### Requirement: Reported power-off availability accounts for permission, not only platform support
Where the host can report whether this application is *permitted* to power the machine off,
the capability report for power-off SHALL reflect that permission and not merely whether the
platform supports shutdown at all.

This distinction is load-bearing. A platform-support check answers "can any app shut this
machine down", which is a constant per OS, and reporting it alone caused a machine whose
Automation consent had been refused to be reported as fully capable — the user's first
indication of the problem was a shutdown that silently did not happen.

An outright permission denial SHALL degrade the power-off capability with a reason written for
a person. An undetermined permission SHALL NOT degrade it, because a warning shown on every
machine that has simply never been asked would train the user to ignore the banner that
matters.

Determining permission at startup SHALL NOT raise a consent dialog. Startup reports; the
prompt belongs to the scheduling interaction, where the user has context for it.

#### Scenario: A refused permission degrades the reported capability
- **WHEN** the host reports that this app is not permitted to power the machine off
- **THEN** the capability report SHALL mark power-off unavailable, with a reason, and the UI
  SHALL show it as a degradation of an essential capability

#### Scenario: An undetermined permission leaves the capability available
- **WHEN** permission has not yet been decided, or could not be checked
- **THEN** the capability report SHALL continue to report power-off as available, so that no
  warning is shown for a machine that has merely never been asked

#### Scenario: Startup does not prompt
- **WHEN** the app starts and probes power-off permission
- **THEN** it SHALL request a report only, and SHALL NOT raise a consent dialog before the user
  has expressed any intent to schedule a shutdown

#### Scenario: A platform-impossible power-off is still reported as unsupported
- **WHEN** the host has no way to power the machine off at all
- **THEN** the capability report SHALL mark power-off unavailable on that basis, without
  consulting permission, since permission is meaningless for an action that cannot occur
