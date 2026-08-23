## Why

The desktop client presents every surface — status, job creation, job list, settings, and
remote control — stacked on one scrolling page that the sidebar only anchor-scrolls, so the
app reads as a single crowded tab. Controls render with browser-default chrome and no shared
visual system. Separately, the macOS `.dmg` opens as a bare, unstyled Finder window, which
looks unfinished at the exact moment a new user forms their first impression of the app. Both
are presentation-layer problems: the scheduling domain and cross-language contract are correct
and must not change.

## What Changes

- Replace sidebar anchor-scrolling with real view-switching: the five primary surfaces
  (dashboard, create, jobs, settings, remote) become distinct views toggled by URL hash, with
  the active sidebar link marked `aria-current="page"`. Every view stays mounted in the DOM
  (only `hidden` toggles) so live countdown intervals keep running and source-reading tests
  still pass.
- Add the missing `#create` sidebar entry (the creation surface currently has no nav link) and
  the `nav.create` string to both `en` and `vi` i18n tables.
- Give each primary view a header with a short description (`dashboard.description`,
  `jobs.description`, `remote.description`), wrapping — never replacing — the existing pinned
  heading ids.
- Turn the dashboard into an at-a-glance overview: keep the status orb and detail, add a
  next-upcoming-job countdown, and add a quick path to the creation view.
- Introduce a v2 design-token system (spacing, radius, type scale, control sizing) and explicit
  light **and** dark palettes, then restyle the shell chrome, sidebar, cards, inputs, buttons,
  switch, segmented control, job rows, remote-device rows, status orb, dialogs, and grace
  banner against those tokens.
- Fix three latent frontend bugs surfaced during design review: the CSS input selector does not
  match `input[type=text]`/`input[type=date]` (they render browser-default); `.job-item` and
  `[data-presence]` remote rows have no styling rules at all; the `nav.create` key is absent.
- Beautify the macOS installer: add a `bundle.macOS.dmg` block (background image + window and
  icon geometry), ship an authored background PNG, and make release CI emit the *styled* DMG
  rather than silently falling back to an unstyled one.

No behavioral change to scheduling, the shutdown grace period, the Rust/Dart domain, or
`shared/testvectors/`. No new Tauri command. The CSP and `capabilities/default.json` stay
byte-identical. All three version declarations stay at `0.2.0`.

## Capabilities

### New Capabilities
- `desktop-navigation-shell`: How the desktop app separates its primary surfaces into distinct,
  hash-navigable views; how the active view is indicated to assistive technology; the invariant
  that all views remain mounted so background timers survive navigation; and that the degraded
  banner is outside the navigable set.
- `macos-installer-packaging`: What the delivered macOS `.dmg` presents to a user (styled Finder
  window with background art and positioned application icon and Applications alias) and the
  requirement that automated release builds produce the styled installer rather than an unstyled
  fallback.

### Modified Capabilities
- `job-management-ui`: The dashboard surface gains a defined role as an at-a-glance overview
  (surfacing the next upcoming job and a direct path to creation) distinct from the full job
  list, and the primary surfaces become independently navigable views rather than sections of
  one scrolling page — while preserving the existing keyboard-operability, accessible-labeling,
  and status-legibility requirements.

## Impact

- **Frontend (behavioral + presentation):** `desktop/src/index.html`, `desktop/src/styles.css`,
  `desktop/src/main.js`, `desktop/src/logic.js`, `desktop/src/i18n.js`. Test suites
  `desktop/src/wiring.test.js`, `desktop/src/i18n.test.js`, `desktop/src/logic.test.js`
  (115 tests) are the regression gate and must stay green; new views/keys/ids extend them.
- **Packaging (macOS-only):** `desktop/src-tauri/tauri.conf.json` (new `bundle.macOS.dmg`
  block), a new committed asset `desktop/src-tauri/images/dmg-background.png` plus its generator,
  and `.github/workflows/release.yml` (`TAURI_BUNDLER_DMG_IGNORE_CI` env, runner pin). Windows
  (`msi`/`nsis`) and Linux (`deb`/`appimage`) build jobs are untouched.
- **Explicitly unchanged:** Rust/Dart domain, `shared/testvectors/`, the CSP string,
  `capabilities/default.json`, the Tauri command surface, and all version declarations.
