## Purpose

Defines what the delivered macOS disk image presents to a user opening it, and the requirement
that automated release builds produce that styled installer rather than silently falling back to
an unstyled one.

## ADDED Requirements

### Requirement: The macOS disk image presents a styled installation window
The macOS `.dmg` SHALL open a Finder window that displays a background image behind the
application icon and an Applications-folder alias, with the two icons positioned so the intended
drag-to-install gesture is visually obvious. The window SHALL be sized to show the full
background without clipping or an unintended scrollbar, and the background art SHALL be legible on
both standard and high-resolution (Retina) displays.

#### Scenario: Opening the disk image shows the styled layout
- **WHEN** a user opens the delivered `.dmg`
- **THEN** the mounted volume window SHALL show the background image with the application icon and
  the Applications alias positioned for a drag-to-install gesture, at the configured window size

#### Scenario: No stray default volume icon is shown
- **WHEN** the disk image is configured with a background image
- **THEN** a background asset SHALL be shipped inside the volume so no default volume icon appears
  in place of the intended art

#### Scenario: Background is authored for high-resolution displays
- **WHEN** the disk image window is viewed on a Retina display
- **THEN** the background SHALL render sharp and fit within the window rather than appearing
  blurred or overflowing the window bounds

### Requirement: Release builds produce the styled disk image
Automated macOS release builds SHALL produce the styled disk image rather than an unstyled
fallback. The release workflow SHALL run on a macOS environment capable of applying the styling
and SHALL be configured so continuous-integration detection does not skip the styling step.

#### Scenario: CI does not silently ship an unstyled installer
- **WHEN** the macOS release job builds the `.dmg` in continuous integration
- **THEN** the produced disk image SHALL contain the configured background and icon positioning,
  not the unstyled default layout

### Requirement: Disk-image styling is confined to macOS
The disk-image styling SHALL affect only the macOS installer and SHALL NOT alter the Windows or
Linux installer artifacts.

#### Scenario: Windows and Linux artifacts are unaffected
- **WHEN** the release workflow builds Windows and Linux installers
- **THEN** those artifacts SHALL be produced exactly as before, with no dependency on the macOS
  disk-image configuration
