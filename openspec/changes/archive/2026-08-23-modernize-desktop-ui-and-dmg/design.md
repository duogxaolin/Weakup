## Context

See `proposal.md` — Why. Constraints that shape the approach:

- The frontend is plain HTML/CSS/JS with **no bundler** and **zero npm deps**; tests run under
  `node:test` and largely work by reading source text with regexes. Many invariants below exist
  because a test greps the source, not because of runtime behavior.
- **CSP D2** (`default-src 'self'; style-src 'self' 'unsafe-inline'`) forbids any external font,
  stylesheet, script, or icon pack. The CSP string in `tauri.conf.json` and
  `capabilities/default.json` must stay byte-identical and are out of scope to edit.
- The WebView floor is **macOS 10.15**, which lacks `color-mix()` and `light-dark()`; both are
  additionally banned by a wiring test. Light and dark palettes must be written out explicitly.
- The DMG facts in this document are verified against the crates pinned by the toolchain
  (`tauri-utils 2.9.3` / `tauri-bundler 2.9.4` via `cargo-tauri 2.11.4`).

## Goals / Non-Goals

**Goals:**

- Real view separation and a coherent modern visual system, achieved without adding a framework,
  a build step, or any network dependency.
- A styled macOS installer produced reliably by release CI.
- Keep all 115 existing frontend tests green; extend them to lock the new behavior.

**Non-Goals:**

- No change to scheduling, the grace period, the Rust/Dart domain, `shared/testvectors/`, or any
  Tauri command. This is presentation + packaging only.
- No dark-mode variant of the DMG art (the bundler schema accepts a single background image).
- No client-side validation added to `logic.js` (its contract is that bounds pass through raw).
- No mobile changes.

## Decisions

### D1 — View-switching via `hidden` attribute + hash routing, not DOM insertion/removal

A `syncViews()` reads `location.hash`, shows the matching view and hides the rest by toggling the
`hidden` attribute, and sets `aria-current="page"` on the active sidebar link. It is wired to
`hashchange` and called once at boot **after** `applyTheme()`/`applyLanguage()`.

Why toggle `hidden` rather than insert/remove nodes: countdown intervals run against elements
that may be in a non-active view, and the wiring tests assert (by reading source) that every
element `main.js` reaches for exists. Removing nodes would break both. Alternative considered — a
micro-router that swaps innerHTML — rejected: it destroys timers and defeats the source-reading
tests. Hash routing (not History API) is chosen because the sidebar-link contract is regex-pinned
as `href="#..."` with the `href` attribute **before** `class`, and hash needs no server/routing
shim inside a `tauri://` webview.

### D2 — Add the `#create` nav item and per-view headers additively

The creation surface has no sidebar link today. Add `<a href="#create" class="sidebar-link">`
(href-before-class order preserved) with a `nav.create` span, and add `nav.create` to both i18n
tables. Each view gets a `<header class="view-header">` wrapping — never replacing — the existing
pinned heading ids (`dashboard-status`, `create-heading`, `jobs-heading`, `settings-heading`,
`remote-heading`), adding a `.view-description` with new keys. Wrapping keeps every regex that
targets those ids satisfied.

### D3 — Design tokens v2 with explicit dual palettes

Introduce spacing/radius/type/control tokens and two fully written palettes under
`[data-theme="light"]` and `[data-theme="dark"]` (no `color-mix`/`light-dark`). Panels
(`.status-panel`, `.composer`, `.jobs-section`, `.settings-panel`) become chrome-less
(transparent, no border, no shadow, `max-width: var(--content-max); margin: 0 auto`); surface and
separation move to the inner cards. Focus is expressed only as
`:focus-visible { outline: 2px solid var(--focus-ring); outline-offset: 2px; }` — the string
`outline: none`/`outline: 0` must never appear because a test strips comments then greps for it.

### D4 — Fix three latent frontend bugs in the same pass

(1) Broaden the single input selector so it also matches `input[type=text]` and
`input[type=date]` (today only `select, input[type=number], input[type=time]` are styled).
(2) Add rules for `.job-item`, `.job-item-revoked`, and `[data-presence="online|stale|offline"]`
(remote device rows are currently unstyled). (3) Add the missing `nav.create` key. These are
pre-existing defects the redesign must not paper over.

### D5 — DMG styling via the native `bundle.macOS.dmg` block + authored background

Add the five-field `bundle.macOS.dmg` block (`background`, `windowSize`, `windowPosition`,
`appPosition`, `applicationFolderPosition`) and ship `desktop/src-tauri/images/dmg-background.png`.
The background is authored at 1320×800 px (2× the 660×400 window) with **144.07 DPI** metadata:
144 makes it render at exactly the window size, and the `.07` nudge keeps it a hair *inside* the
window to avoid a rounding-induced scrollbar. A committed generator script produces the PNG
deterministically (no manual art step, no new runtime dependency). Real icons are drawn by Finder
on top of the art, so the art must **not** paint fake icons; it keeps ±95 pt clear zones around
the icon positions. Alternative considered — `create-dmg`/`node-appdmg` — rejected: Tauri already
embeds this exact tooling; a second toolchain adds cost for no gain.

### D6 — Make release CI emit the styled DMG

The bundler skips Finder styling when it detects CI unless `TAURI_BUNDLER_DMG_IGNORE_CI=true`.
Set that env on the macOS release step and pin `runs-on: macos-15` (a GUI-session hosted runner;
the mid-2025 Provisioner Finder-timeout incident is fixed on `20250701.355+`). Keep
`--bundles app,dmg` and the git-tag version override. Documented fallback: if the AppleEvent
`-1712` timeout ever recurs, remove the env var to ship a valid-but-unstyled DMG.
