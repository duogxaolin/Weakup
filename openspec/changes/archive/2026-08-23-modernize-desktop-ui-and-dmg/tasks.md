## 1. Navigation shell (view-switching)

- [x] 1.1 In `desktop/src/index.html`, add the 5th sidebar link `<a href="#create" class="sidebar-link">` (href-before-class order preserved) with an inline-SVG/unicode icon and `<span data-i18n="nav.create">`, placed among the existing primary destinations; do not make `#degraded` a nav destination.
- [x] 1.2 Add key `nav.create` to BOTH the `en` and `vi` i18n tables in `desktop/src/i18n.js` (exact key parity, non-blank values).
- [x] 1.3 In `desktop/src/main.js`, add `const VIEWS = ["dashboard","create","jobs","settings","remote"]` and a `syncViews()` that reads `location.hash`, toggles each view's `hidden` attribute so exactly one shows (empty/unknown hash → dashboard), and sets `aria-current="page"` on the matching sidebar link while removing it from all others.
- [x] 1.4 Wire `syncViews()` to `window.addEventListener("hashchange", …)` and call it once at boot AFTER `applyTheme()`/`applyLanguage()`. ← (verify: every view stays mounted in the DOM — only `hidden` toggles — so countdown intervals on non-active views keep running; degraded banner is outside the view set and its visibility is independent of active view)

## 2. Per-view headers and dashboard overview

- [x] 2.1 In `index.html`, wrap (never replace) each pinned heading id (`dashboard-status`, `create-heading`, `jobs-heading`, `settings-heading`, `remote-heading`) in a `<header class="view-header">` and add a `.view-description` element per view.
- [x] 2.2 Add keys `dashboard.description`, `jobs.description`, `remote.description` to BOTH i18n tables (create/settings reuse existing heading strings or add matching description keys as needed, kept in parity).
- [x] 2.3 On the dashboard view keep `status-orb`/`dashboard-status`/`dashboard-detail`; add a next-upcoming-job countdown reusing the `p.job-countdown[data-countdown-for]` pattern, and a quick-create anchor `<a class="button button-secondary" href="#create">`. When nothing is scheduled, show plain-language text instead of a stale countdown. ← (verify: dashboard surfaces the next upcoming job's live countdown, offers a direct path to #create, and states in plain language when nothing is scheduled — matches job-management-ui spec scenarios)

## 3. Design tokens v2, component CSS, and latent-bug fixes

- [x] 3.1 In `desktop/src/styles.css` add tokens: spacing `--space-1..6` (4/8/12/16/24/32), radius (sm 7 / 12 / lg 16 / full 999), type scale (xs 11/sm 12/md 13/lg 15/xl 20), `--control-height 36`, `--button-height 34`, transitions, `--sidebar-width 232px`, `--content-max 860px`.
- [x] 3.2 Write explicit light AND dark palettes under `[data-theme="light"]` and `[data-theme="dark"]` (accent, danger, warning, success families + canvas/surface/border ramps + `--focus-ring`). Do NOT use `color-mix()` or `light-dark()`.
- [x] 3.3 De-box panels (`.status-panel`, `.composer`, `.jobs-section`, `.settings-panel`): transparent, no border, no shadow, `max-width: var(--content-max); margin: 0 auto`; move surface/separation to inner cards.
- [x] 3.4 Restyle components against tokens: sidebar-link with `[aria-current="page"]` soft-accent pill; `.feature-card` selection via border + inset ring (power variant danger); `.button`/`.button-primary`/`.button-danger`/`.button-quiet`/`.button-small`; switch 42×26; segmented control as raised chip; `.job`/`.job-item` grid rows with dot marker; status-orb 52px ring; dialogs surface-raised + shadow-lg; grace banner big mono tabular count.
- [x] 3.5 Fix bug: broaden the single input selector so it ALSO matches `input[type=text]` and `input[type=date]` (currently only `select, input[type=number], input[type=time]`).
- [x] 3.6 Fix bug: add CSS rules for `.job-item`, `.job-item-revoked`, and `[data-presence="online|stale|offline"]` (currently unstyled).
- [x] 3.7 Express focus ONLY as `:focus-visible { outline: 2px solid var(--focus-ring); outline-offset: 2px; }`. The literal `outline: none` / `outline: 0` must appear nowhere in the stylesheet. ← (verify: after stripping comments, no `outline:\s*(none|0)` match anywhere; no `color-mix(`/`light-dark(`; both palettes present)

## 4. Extend frontend tests to lock new behavior

- [x] 4.1 Extend `desktop/src/wiring.test.js` / `i18n.test.js` so the new `#create` link, `nav.create`, and the three `*.description` keys are covered by the existing "every element main.js reaches for exists", "every data-i18n key exists in both tables", and href-before-class contracts.
- [x] 4.2 Run `cd desktop/src && node --test` and confirm all pre-existing 115 tests plus the new assertions pass; no test was weakened. ← (verify: full node:test suite green; landmarks/grace-banner-order/theme-radios/labelled-inputs/no-positive-tabindex invariants intact; logic.js still passes bounds through raw with no client-side validation added)

## 5. macOS DMG styling

- [x] 5.1 Add the `bundle.macOS.dmg` block to `desktop/src-tauri/tauri.conf.json` with exactly the five valid fields: `background: "./images/dmg-background.png"`, `windowSize {660,400}`, `windowPosition {200,120}`, `appPosition {180,190}`, `applicationFolderPosition {480,190}`. Keep `minimumSystemVersion` at 10.15 and CSP byte-identical.
- [x] 5.2 Add a committed deterministic generator (script using sips/ImageMagick/Pillow or equivalent) plus the produced `desktop/src-tauri/images/dmg-background.png` at 1320×800 px, DPI 144.07, mid-to-light palette, title band y≈40–110, ≥40 pt edge margins, ±95 pt clear zones around (180,190) and (480,190). Do NOT paint fake app/Applications icons. ← (verify: PNG is shipped so no stray `.VolumeIcon.icns` appears; art fits the window without an induced scrollbar and stays sharp on Retina)

## 6. Release CI emits the styled DMG

- [x] 6.1 In `.github/workflows/release.yml`, on the macOS build step add `env: TAURI_BUNDLER_DMG_IGNORE_CI: "true"` and pin `runs-on: macos-15`; keep `--bundles app,dmg` and the git-tag version override. Leave Windows (`msi`/`nsis`) and Linux (`deb`/`appimage`) jobs untouched. ← (verify: macOS job produces a styled dmg in CI, not the unstyled fallback; Windows/Linux artifacts unchanged and have no dependency on the dmg config)

## 7. Full verification gate

- [x] 7.1 Run `cd desktop/src && node --test` (frontend). If any packaging build ran `cargo tauri build`, restore `git checkout -- desktop/src-tauri/Cargo.toml`. ← (done: `node --test` → 115/115 pass; `cargo test --lib` → 537 passed; `cargo clippy --all-targets -- -D warnings` clean; `cargo test --test shared_vectors` → 24 passed. No packaging build ran this session, so `desktop/src-tauri/Cargo.toml` was never rewritten and needs no restore — `git diff` shows it unchanged.)
- [x] 7.2 Confirm no out-of-scope files changed: touched only `desktop/src/*`, `desktop/src-tauri/tauri.conf.json`, `desktop/src-tauri/images/`, and `.github/workflows/release.yml`. Version declarations stay at 0.2.0. ← (verify: `git diff --stat` shows only in-scope paths; no domain/vector/CSP/capabilities/version change) ← (done: working tree shows exactly the declared blast radius — M: .github/workflows/release.yml, desktop/src-tauri/tauri.conf.json, desktop/src/i18n.js, desktop/src/index.html, desktop/src/main.js, desktop/src/styles.css, desktop/src/wiring.test.js; ??: desktop/src-tauri/images/. M desktop/src-tauri/src/platform/power_off/macos.rs is from an earlier separately-completed fix, not this change. No diff on desktop/src-tauri/Cargo.toml or mobile/pubspec.yaml. Version at 0.2.0 (tauri.conf.json line 4); CSP and capabilities/default.json byte-identical.)
