/*
 * The seams between the web view and everything either side of it.
 *
 * `logic.test.js` covers the decisions. This file covers the joins, which are what actually
 * broke during development: `main.js` referenced `#boot-status` for a while after the
 * element was renamed out of `index.html`, and nothing failed — the window simply came up
 * with dead controls. A typo in an element id or a command name is invisible until someone
 * runs the app and clicks the thing.
 *
 * So these tests read the real files as text and check the lists agree: the ids `main.js`
 * asks the DOM for, the commands it invokes, the commands Rust registers, the translation
 * keys the markup names, and the themes the stylesheet actually defines.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { TABLES } from "./i18n.js";
import { THEMES } from "./logic.js";

const here = dirname(fileURLToPath(import.meta.url));
const read = (...parts) => readFileSync(join(here, ...parts), "utf8");

const html = read("index.html");
const mainJs = read("main.js");
const logicJs = read("logic.js");
const libRs = read("..", "src-tauri", "src", "lib.rs");
const commandsRs = read("..", "src-tauri", "src", "commands", "mod.rs");

const matchAll = (source, pattern) => [...source.matchAll(pattern)].map((match) => match[1]);

/* ------------------------------------------------------------------ */
/* The DOM                                                            */
/* ------------------------------------------------------------------ */

test("every element main.js reaches for exists in the HTML", () => {
  const referenced = new Set(matchAll(mainJs, /\bel\("([^"]+)"\)/g));
  const defined = new Set(matchAll(html, /\bid="([^"]+)"/g));

  assert.ok(referenced.size > 20, `expected the real file, found ${referenced.size} ids`);

  const missing = [...referenced].filter((id) => !defined.has(id)).sort();
  assert.deepEqual(missing, [], `main.js reads elements that do not exist: ${missing}`);
});

test("every dataset selector main.js queries is one it also writes", () => {
  // The countdown rows are found by data attribute rather than id, since there is one per
  // job. A rename on either side would silently stop the countdowns updating.
  const queried = matchAll(mainJs, /\[data-([a-z-]+)-for=/g);
  assert.ok(queried.length > 0, "the countdown lookup is gone");

  for (const attribute of queried) {
    const camel = attribute.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    assert.match(
      mainJs,
      new RegExp(`dataset\\.${camel}For\\s*=`),
      `nothing ever sets data-${attribute}-for, so the query matches nothing`,
    );
  }
});

/* ------------------------------------------------------------------ */
/* The IPC surface                                                    */
/* ------------------------------------------------------------------ */

/*
 * Commands reached two ways: named literally in `main.js`, or carried as data out of
 * `actionsFor` in `logic.js` and passed to one `invoke`. Both count as used.
 */
const invokedNames = new Set([
  ...matchAll(mainJs, /invoke\("([a-z_]+)"/g),
  ...matchAll(logicJs, /command:\s*"([a-z_]+)"/g),
]);

const registeredNames = new Set(
  matchAll(
    libRs.slice(libRs.indexOf("generate_handler!"), libRs.indexOf("])", libRs.indexOf("generate_handler!"))),
    /commands::([a-z_]+)/g,
  ),
);

const definedNames = new Set(
  matchAll(commandsRs, /#\[tauri::command\]\s*\npub fn ([a-z_]+)/g),
);

test("the frontend only invokes commands Rust registered", () => {
  assert.ok(registeredNames.size >= 19, `parsed ${registeredNames.size} registrations`);

  const unknown = [...invokedNames].filter((name) => !registeredNames.has(name)).sort();
  assert.deepEqual(unknown, [], `the UI calls commands that do not exist: ${unknown}`);
});

test("every command Rust defines is registered", () => {
  // A `#[tauri::command]` that is never registered compiles, exports nothing, and fails
  // only at runtime with "command not found".
  const unregistered = [...definedNames].filter((name) => !registeredNames.has(name)).sort();
  assert.deepEqual(unregistered, [], `defined but unreachable: ${unregistered}`);
});

test("no registered command is left with nothing calling it", () => {
  // Not a correctness bug, but it means either the UI is unfinished or the command is
  // dead. Either way it should be a decision rather than an oversight.
  const unused = [...registeredNames].filter((name) => !invokedNames.has(name)).sort();
  assert.deepEqual(unused, [], `registered but no UI reaches them: ${unused}`);
});

/* ------------------------------------------------------------------ */
/* The shutdown boundary                                              */
/* ------------------------------------------------------------------ */

test("nothing the web view can call powers the machine off", () => {
  // The web view can cancel a shutdown and can schedule one for later, both of which pass
  // through the grace period. What it must not have is a way to reach the executor
  // directly — that would put an irreversible action one compromised page away, with no
  // countdown in between.
  //
  // `check_shutdown_permission` is deliberately named to survive this filter on its
  // meaning rather than its spelling: it asks the OS about consent via
  // `AEDeterminePermissionToAutomateTarget`, which reports without sending an Apple
  // Event, so it holds no executor and cannot shut anything down.
  const shutdownish = [...invokedNames].filter((name) =>
    /^(power_off|shutdown|force_|execute_power)/.test(name),
  );
  assert.deepEqual(shutdownish, [], `the UI can trigger a shutdown directly: ${shutdownish}`);

  // And the only grace-related thing it can do is stop one.
  const graceCommands = [...invokedNames].filter((name) => name.includes("grace")).sort();
  assert.deepEqual(graceCommands, ["cancel_grace_period", "grace_period_length", "grace_state"]);
});

test("the shutdown permission check runs before the job is created", () => {
  // The ordering *is* the feature. Asking after `create_job` would leave the user
  // with a saved schedule and a consent dialog they can dismiss, which is exactly
  // the state that produces a machine still running in the morning.
  const submit = mainJs.slice(
    mainJs.indexOf("async function submitForm"),
    mainJs.indexOf("/* ---", mainJs.indexOf("async function submitForm")),
  );
  assert.ok(submit.length > 100, "submitForm was not found");

  const checkAt = submit.indexOf("checkShutdownPermission");
  const createAt = submit.indexOf("createOne");

  assert.ok(checkAt > 0, "submitForm never checks shutdown permission");
  assert.ok(createAt > 0, "submitForm never creates a job");
  assert.ok(
    checkAt < createAt,
    "permission is checked after the job is created, which defeats the point",
  );

  // And a denial must stop the submit rather than merely warn.
  assert.match(
    submit,
    /if\s*\(!permission\.allow\)[\s\S]{0,200}?return/,
    "a denied permission does not stop the submit",
  );
});

test("the permission check is asked of Rust, not decided in the web view", () => {
  // Whether the OS will permit a shutdown is not knowable from JavaScript. A guess
  // here would either block working machines or promise shutdowns that cannot run.
  assert.match(mainJs, /invoke\("check_shutdown_permission",\s*\{\s*askUser\s*\}\)/);

  // Comments are stripped first: `permissionOutcome`'s doc comment explains what the
  // OS-level verdicts mean, and a scan that counted prose would fail on the file
  // documenting the rule it checks — the same reason the stylesheet scan strips them.
  const code = logicJs.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
  const guesses = ["Automation", "System Events", "TCC", "SeShutdown", "osascript"].filter(
    (term) => code.includes(term),
  );

  assert.deepEqual(guesses, [], `logic.js reasons about OS permissions itself: ${guesses}`);
});

test("the consent dialog is explained before it is raised", () => {
  // A spec requirement (`device-power-off`: "macOS Automation consent requested
  // before first use"): the user must be told macOS is about to ask *before* the
  // dialog appears. An unexplained system prompt is the kind a user dismisses.
  //
  // The ordering is structural, not a matter of timing: ticking the checkbox asks
  // with `false` (reports, cannot prompt) and reveals the note; only the submit
  // path asks with `true`. A prompt fired from the checkbox handler could not
  // guarantee the sentence had painted first.
  const wire = mainJs.slice(
    mainJs.indexOf("function wireForm"),
    mainJs.indexOf("async function submitForm"),
  );
  assert.ok(wire.length > 100, "wireForm was not found");

  assert.match(
    wire,
    /checkShutdownPermission\(false\)/,
    "ticking Power Off may raise a dialog before anything has explained it",
  );
  assert.ok(
    !wire.includes("checkShutdownPermission(true)"),
    "the checkbox handler prompts, so the explanation cannot be guaranteed first",
  );

  // And the note must exist, be hidden by default, and be driven from the verdict.
  assert.match(html, /id="power-off-consent-note"[^>]*hidden/);
  assert.match(wire, /note\.hidden\s*=/, "nothing ever reveals the consent note");

  // The reveal must consult `promptsForConsent`, not just whether there is a reason.
  // The Windows and Linux preflight returns "undetermined" *with* a reason, so keying
  // on the reason alone puts a sentence about macOS and System Events in front of a
  // Windows user — a platform-specific claim on a platform where it is false.
  //
  // There are two assignments to `note.hidden`: the untick branch sets it to a plain
  // `true`, and the verdict branch computes it. The one under test is the computed one.
  const wireCode = wire.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
  const revealLine = wireCode
    .split("\n")
    .find((line) => /note\.hidden\s*=/.test(line) && !/=\s*(true|false)\s*;/.test(line));

  assert.ok(revealLine, "nothing computes the note's visibility from the verdict");
  assert.match(
    revealLine,
    /promptsForConsent/,
    "the consent note is revealed without checking that a dialog can appear at all",
  );

  const submit = mainJs.slice(mainJs.indexOf("async function submitForm"));
  assert.match(
    submit,
    /checkShutdownPermission\(true\)/,
    "the submit path never actually asks for consent",
  );
});

test("the capability permissions stay narrow", () => {
  // Filesystem, shell, and process permissions would each give the web view a way around
  // the Rust-side-only shutdown rule.
  const capabilities = JSON.parse(read("..", "src-tauri", "capabilities", "default.json"));
  const forbidden = capabilities.permissions.filter((permission) =>
    /^(fs|shell|process|http):/.test(permission),
  );
  assert.deepEqual(forbidden, [], `permissions that bypass the Rust boundary: ${forbidden}`);
});

/* ------------------------------------------------------------------ */
/* Accessibility, as far as source can show it                        */
/* ------------------------------------------------------------------ */

test("every form control has a label pointing at it", () => {
  // A select or input a screen reader announces as "edit text, blank" is unusable, and
  // this is the half of task 12.6 that a file can actually be checked for.
  const labelled = new Set(matchAll(html, /<label[^>]*\bfor="([^"]+)"/g));

  const controls = [...html.matchAll(/<(select|input|textarea)\b[^>]*>/g)].map((m) => m[0]);
  const unlabelled = controls
    // Checkboxes and radios use the wrapping-label form instead, checked in the next test.
    .filter((tag) => !/type="(checkbox|radio|submit|hidden)"/.test(tag))
    .map((tag) => (tag.match(/\bid="([^"]+)"/) ?? [])[1])
    .filter((id) => id && !labelled.has(id));

  assert.deepEqual(unlabelled, [], `controls with no <label for>: ${unlabelled}`);
});

test("the checkboxes and radios are wrapped in their labels", () => {
  // These use the wrapping form rather than `for=`, which is equally valid and gives a
  // larger hit area. Checking they are wrapped rather than bare.
  for (const id of [
    "want-keep-awake",
    "want-power-off",
    "notifications-enabled",
    "autostart-enabled",
    "theme-auto",
    "theme-light",
    "theme-dark",
  ]) {
    const index = html.indexOf(`id="${id}"`);
    assert.ok(index > 0, `${id} is gone`);
    const preceding = html.slice(0, index);
    const openedLabel = preceding.lastIndexOf("<label");
    const closedLabel = preceding.lastIndexOf("</label>");
    assert.ok(openedLabel > closedLabel, `${id} is not inside a <label>`);
  }
});

test("the shutdown banner announces itself and stays before the app", () => {
  // It appears without the user doing anything, and it is the last chance to stop an
  // irreversible action. Without a live region a screen reader user gets no notice at all.
  const graceIndex = html.indexOf('id="grace"');
  const shellIndex = html.indexOf('class="shell"');
  const banner = html.slice(graceIndex, html.indexOf("</section>", graceIndex));
  assert.ok(graceIndex > 0 && graceIndex < shellIndex, "the grace banner is not before the app");
  assert.match(banner, /role="alert"/, "the banner is not announced when it appears");
  assert.match(banner, /aria-valuemin/, "the progress bar has no announced range");
  assert.match(banner, /id="grace-cancel"/, "the banner has no direct cancel control");
});

test("secondary settings sit in the settings panel", () => {
  const settings = html.slice(
    html.indexOf('id="settings"'),
    html.indexOf("</section>", html.indexOf('id="settings"')),
  );

  for (const id of ["timezone", "language", "notifications-enabled", "autostart-enabled"]) {
    assert.ok(settings.includes(`id="${id}"`), `${id} is not in the settings panel`);
  }

  // The theme control has to be radios rather than a select or buttons: three exclusive
  // options that a screen reader should announce as "2 of 3", with arrow-key navigation.
  assert.match(settings, /role="radiogroup"/);
  for (const theme of ["auto", "light", "dark"]) {
    assert.match(
      settings,
      new RegExp(`type="radio"[^>]*id="theme-${theme}"[^>]*value="${theme}"`),
      `no radio for the ${theme} theme`,
    );
  }
});

test("the dashboard retains semantic landmarks", () => {
  assert.match(html, /<nav class="sidebar"[^>]*aria-label="Weakup">/);
  assert.match(html, /<main class="content">/);
  assert.match(html, /class="status-panel"[^>]*aria-labelledby="dashboard-status"/);
  assert.match(html, /id="create"[^>]*class="composer"[^>]*aria-labelledby="create-heading"/);
  assert.match(html, /id="jobs"[^>]*class="jobs-section"[^>]*aria-labelledby="jobs-heading"/);
  assert.match(html, /id="settings"[^>]*class="settings-panel"[^>]*aria-labelledby="settings-heading"/);
  assert.match(html, /<footer class="page-footer">/);
});

test("every sidebar link points at a section that exists", () => {
  // The sidebar is in-page navigation, not a router. A link to a removed id scrolls
  // nowhere and looks like a dead control.
  const targets = matchAll(html, /<a\s+href="#([\w-]+)"\s+class="sidebar-link"/g);
  assert.ok(targets.length >= 3, `parsed ${targets.length} sidebar links`);

  const missing = targets.filter((id) => !html.includes(`id="${id}"`)).sort();
  assert.deepEqual(missing, [], `sidebar links with no target: ${missing}`);
});

test("no control is made focusable by hand", () => {
  // Native buttons, inputs, and selects are focusable and keyboard-operable already
  // (task 12.5). A positive tabindex, or a div given one, is the usual way that breaks.
  assert.doesNotMatch(html, /tabindex="[1-9]/, "a positive tabindex reorders the tab sequence");
  assert.doesNotMatch(mainJs, /tabIndex\s*=/, "main.js makes something focusable by hand");
});

test("focus is never suppressed in the stylesheet", () => {
  // Comments are stripped first. The stylesheet's own header says "never `outline: none`",
  // and a scan that counted that would fail on the file documenting the rule it checks.
  const rules = read("styles.css").replace(/\/\*[\s\S]*?\*\//g, "");
  const suppressed = [...rules.matchAll(/outline:\s*(none|0)\b/g)].map((match) => match[0]);
  assert.deepEqual(suppressed, [], `a focus ring is removed: ${suppressed}`);
});

/* ------------------------------------------------------------------ */
/* Translation and theme wiring                                       */
/* ------------------------------------------------------------------ */

test("the date control is on the power-off card only", () => {
  // A scope decision rather than a domain rule: the resolver accepts a dated keep-awake
  // trigger, and a shared vector case says so. Only the UI withholds it, because
  // "keep the display awake on 10 August" is not a case anyone asked for. Encoded here
  // so adding it later is a deliberate act rather than a copy-paste.
  const composer = html.slice(html.indexOf('id="create"'), html.indexOf("</form>"));

  const powerOff = composer.slice(composer.indexOf('id="power-off-card"'));
  assert.ok(
    powerOff.includes('id="power-off-date"'),
    "the power-off card has no date control",
  );

  const keepAwake = composer.slice(
    composer.indexOf('id="keep-awake-card"'),
    composer.indexOf('id="power-off-card"'),
  );
  assert.ok(
    !/id="keep-awake-date/.test(keepAwake),
    "a date control appeared on the keep-awake card",
  );

  // main.js reads the date through a `form.date &&` guard precisely because there is no
  // keep-awake element to read. An unguarded `el()` would return null and the "every
  // element main.js reaches for exists" test above would not catch it, since the id is
  // never written down.
  assert.match(
    mainJs,
    /form\.date\s*\?/,
    "the date read is unguarded, so the keep-awake form will look for an element that does not exist",
  );
});

test("the date field starts empty so the default behaviour is unchanged", () => {
  // A `value` attribute here would silently convert every absolute-time power-off into
  // a dated one-off, which does not roll forward — changing what an untouched form does.
  const field = html.slice(
    html.indexOf('id="power-off-date-field"'),
    html.indexOf("</div>", html.indexOf('id="power-off-date-field"')),
  );
  const input = field.slice(field.indexOf("<input"), field.indexOf(">", field.indexOf("<input")));

  assert.match(input, /type="date"/);
  assert.ok(!input.includes("value="), `the date input has a default value: ${input}`);
});

test("every data-i18n attribute names a key the table defines", () => {
  // These are filled by `applyLanguage` looping over the attribute. A key with no entry
  // writes the key itself into the window — "settings.languageLabel" as a visible label.
  const keys = matchAll(html, /data-i18n="([^"]+)"/g);
  assert.ok(keys.length > 30, `found only ${keys.length} translated nodes`);

  const unknown = [...new Set(keys)].filter((key) => !(key in TABLES.en)).sort();
  assert.deepEqual(unknown, [], `markup asks for keys no table has: ${unknown}`);
});

test("nothing but text is marked for translation", () => {
  // `applyLanguage` assigns `textContent`, which would wipe out any children. A container
  // marked by mistake would lose its icons the first time the language changed.
  const withChildren = [...html.matchAll(/<(\w+)[^>]*\bdata-i18n="([^"]+)"[^>]*>([\s\S]*?)<\/\1>/g)]
    .filter((match) => /<(?!\/)/.test(match[3]))
    .map((match) => match[2]);

  assert.deepEqual(withChildren, [], `these would lose their children: ${withChildren}`);
});

test("the theme radios cover exactly the themes the logic resolves", () => {
  const values = matchAll(html, /name="theme"[^>]*value="([a-z]+)"/g).sort();
  assert.deepEqual(values, [...THEMES].sort());
});

test("the stylesheet defines every theme the radios offer", () => {
  // `main.js` writes the resolved theme into `data-theme`. A value with no rule block
  // behind it leaves every custom property unset, which renders as black on black.
  const css = read("styles.css");
  for (const theme of ["light", "dark"]) {
    assert.ok(
      css.includes(`[data-theme="${theme}"]`),
      `the stylesheet has no palette for ${theme}`,
    );
  }
});

test("colour is never mixed at runtime", () => {
  // The macOS floor is 10.15. A WebView that does not know `color-mix()` drops the whole
  // declaration, and for a background that means unreadable text rather than a near-miss
  // shade — so both palettes are written out explicitly instead.
  const css = read("styles.css").replace(/\/\*[\s\S]*?\*\//g, "");
  const mixed = [...css.matchAll(/\b(color-mix|light-dark)\s*\(/g)].map((match) => match[1]);

  assert.deepEqual(mixed, [], `runtime colour functions in the stylesheet: ${mixed}`);
});

test("the language picker is populated from the table rather than the markup", () => {
  // Hard-coded options drift from `LANGUAGES` — an option whose code has no table selects
  // a language that silently falls back to English.
  const select = html.slice(html.indexOf('id="language"'), html.indexOf("</select>", html.indexOf('id="language"')));
  assert.ok(!select.includes("<option"), "the language options are hard-coded in the HTML");
  assert.match(mainJs, /LANGUAGES\.map/);
});

test("the action panels do not use a fieldset and legend", () => {
  // A `<legend>` is positioned by the browser against the fieldset's *border* box rather
  // than its content box, and it straddles the border. That made each panel's header sit
  // narrower than, and offset from, the body directly below it — the controls looked
  // misaligned in the built app while every test here still passed. `role="group"` with
  // `aria-labelledby` carries the same grouping and label without the layout quirk.
  const composer = html.slice(html.indexOf('id="create"'), html.indexOf("</form>"));

  assert.ok(!composer.includes("<fieldset"), "a fieldset is back in the composer");
  assert.ok(!composer.includes("<legend"), "a legend is back in the composer");

  // The grouping itself must not be lost in the process: a screen reader has to still
  // announce the controls as belonging to a named group.
  for (const id of ["keep-awake-card", "power-off-card"]) {
    const card = composer.slice(composer.indexOf(`id="${id}"`));
    const opening = card.slice(0, card.indexOf(">"));
    assert.match(opening, /role="group"/, `${id} is no longer a group`);
    assert.match(opening, /aria-labelledby="[\w-]+"/, `${id} has no accessible name`);
  }
});

test("every aria-labelledby points at an element that exists", () => {
  // A dangling reference leaves the region with no accessible name at all, which is worse
  // than a plain unlabelled group because it looks correct in the markup.
  const referenced = matchAll(html, /aria-labelledby="([^"]+)"/g).flatMap((value) =>
    value.split(/\s+/),
  );
  assert.ok(referenced.length > 5, `parsed ${referenced.length} references`);

  const defined = new Set(matchAll(html, /\bid="([^"]+)"/g));
  const dangling = [...new Set(referenced)].filter((id) => !defined.has(id)).sort();

  assert.deepEqual(dangling, [], `aria-labelledby points at missing ids: ${dangling}`);
});

/* ------------------------------------------------------------------ */
/* The remote surface (tasks 7.2, 7.3, 7.6)                            */
/* ------------------------------------------------------------------ */

test("every remote control exists and is wired to a command", () => {
  // The failure this catches is the one the file header describes: an id renamed on one side
  // leaves a control that looks present and does nothing. These are the controls the pairing
  // flow cannot work without.
  const controls = {
    "account-sign-in": "sign_in_to_account",
    "account-sign-out": "sign_out_of_account",
    "pairing-code-show": "present_pairing_code",
    "pairing-submit": "accept_pairing_code",
    "remote-control-enabled": "set_remote_control_enabled",
  };

  for (const [id, command] of Object.entries(controls)) {
    assert.ok(html.includes(`id="${id}"`), `${id} is not in the markup`);
    assert.ok(
      invokedNames.has(command),
      `${id} is present but nothing invokes ${command}, so the control is dead`,
    );
  }

  // The read side too. A surface that could pair but never list would leave the user unable
  // to see or revoke what they had paired.
  for (const command of ["list_pairings", "list_devices", "account_state", "remote_control_enabled"]) {
    assert.ok(invokedNames.has(command), `nothing reads ${command}`);
  }
});

test("the remote panel is a labelled landmark with the lists inside it", () => {
  assert.match(
    html,
    /id="remote"[^>]*class="settings-panel"[^>]*aria-labelledby="remote-heading"/,
    "the remote panel is not an accessibly named section",
  );

  const remote = html.slice(html.indexOf('id="remote"'), html.indexOf("</section>", html.indexOf('id="remote"')));

  for (const id of [
    "pairing-code",
    "pairing-code-entry",
    "pairing-peer-id",
    "pairing-peer-key",
    "pairings-list",
    "devices-list",
  ]) {
    assert.ok(remote.includes(`id="${id}"`), `${id} is not inside the remote panel`);
  }
});

test("the remote-control setting is in the settings surface, not the pairing one", () => {
  // Task 7.3: changeable only at the machine, and found where a user looks for a setting.
  // Putting it in the pairing panel would imply it is per-pairing, which it is not — it
  // gates every peer at once and revoking is the per-peer act.
  const settings = html.slice(
    html.indexOf('id="settings"'),
    html.indexOf("</section>", html.indexOf('id="settings"')),
  );

  assert.ok(
    settings.includes('id="remote-control-enabled"'),
    "the remote-control toggle is not in the settings panel",
  );
});

test("the remote-control toggle is a checkbox wrapped in its label", () => {
  const index = html.indexOf('id="remote-control-enabled"');
  assert.ok(index > 0, "the toggle is gone");

  const tag = html.slice(html.lastIndexOf("<input", index), html.indexOf(">", index) + 1);
  assert.match(tag, /type="checkbox"/, "the setting is not a checkbox");
  // Deliberately not `checked` in the markup: the stored value is read at boot, and a
  // default-checked box would show a machine as commandable before anything was read.
  assert.doesNotMatch(tag, /\bchecked\b/, "the toggle ships pre-enabled");

  const preceding = html.slice(0, index);
  assert.ok(
    preceding.lastIndexOf("<label") > preceding.lastIndexOf("</label>"),
    "the toggle is not inside a <label>",
  );
});

test("the remote-control setting is saved by its own command, not the settings form", () => {
  // A stale form posting the whole settings object back would silently re-enable a setting
  // the user had just turned off — the same hazard the camelCase tests guard in Rust, with
  // a worse consequence. So `saveSettings` must not carry this field.
  const saveSettings = mainJs.slice(mainJs.indexOf("async function saveSettings"));
  const body = saveSettings.slice(0, saveSettings.indexOf("\n}"));

  assert.ok(
    !body.includes("remoteControlEnabled") && !body.includes("remote-control-enabled"),
    "saveSettings carries the remote-control flag, so a stale form could re-enable it",
  );
  assert.match(
    mainJs,
    /el\("remote-control-enabled"\)\.addEventListener\("change",\s*saveRemoteControl\)/,
    "the toggle is not wired to its own save",
  );
});

test("presence is rendered from the state Rust derived and never computed here", () => {
  /*
   * Task 7.2, and the test most worth being suspicious of.
   *
   * The tempting mistake is not a wrong calculation — it is *skipping* the calculation and
   * passing a relay-supplied flag straight through. So this checks two things a passthrough
   * would fail:
   *
   *   1. the frontend reads `presence`, the derived three-state string, and
   *   2. it contains none of the thresholds that would mean it decided for itself.
   *
   * Verified by deliberately replacing `devicePresentation`'s use of `device.presence` with
   * a relay boolean during development and confirming this test and the two below went red.
   */
  assert.match(logicJs, /PRESENCE_STATES\s*=\s*\["online",\s*"stale",\s*"offline"\]/, "the three states are gone");

  // The rule's own numbers must not appear in the web view. 90 and 900 are the online and
  // offline thresholds; writing either here would be a third implementation of a rule the
  // shared vectors pin for two, and this one has no vectors.
  for (const threshold of ["90", "900"]) {
    const arithmetic = logicJs
      .split("\n")
      .filter((line) => !line.trim().startsWith("*") && !line.trim().startsWith("//"))
      .filter((line) => new RegExp(`[<>]=?\\s*${threshold}\\b`).test(line));

    assert.deepEqual(
      arithmetic,
      [],
      `logic.js compares against ${threshold}, so it is deciding presence itself: ${arithmetic}`,
    );
  }
});

test("no relay-supplied online flag reaches the web view", () => {
  // The passthrough this rules out. A boolean from an untrusted relay is a claim, and two
  // devices asking the same relay about one peer could otherwise be told different things.
  for (const forbidden of ["isOnline", "is_online", ".online", "reportedOnline"]) {
    for (const [name, source] of [
      ["logic.js", logicJs],
      ["main.js", mainJs],
    ]) {
      const hits = source
        .split("\n")
        .filter((line) => !line.trim().startsWith("*") && !line.trim().startsWith("//"))
        .filter((line) => line.includes(forbidden));

      assert.deepEqual(hits, [], `${name} reads ${forbidden}, a relay's own presence claim: ${hits}`);
    }
  }
});

test("presence is three states in the markup and the tables, never a boolean", () => {
  // `stale` is the whole reason this is not a boolean: a backgrounded phone is neither
  // reachable nor gone. A table with only two of the three would force the UI to collapse
  // one into another, and whichever it chose would mislead.
  for (const state of ["online", "stale", "offline"]) {
    assert.ok(`presence.${state}` in TABLES.en, `presence.${state} is missing from English`);
    assert.ok(`presence.${state}` in TABLES.vi, `presence.${state} is missing from Vietnamese`);
  }

  // Rendered onto the row as a data attribute, so the three are distinguishable without
  // relying on colour alone.
  assert.match(mainJs, /dataset\.presence\s*=/, "the presence state never reaches the DOM");
});

test("the pairing code counts down from when it was shown, not by decrementing", () => {
  // A decremented counter stops in a backgrounded web view, and the user would then be
  // reading a code the machine has already forgotten. Derived from the shown instant
  // instead, which is the same reason the job countdowns work from an absolute target.
  assert.match(logicJs, /export function codeSecondsRemaining\(shownAtMs, expiresInSeconds, nowMs\)/);
  assert.match(mainJs, /codeSecondsRemaining\(codeShownAtMs, codeLifetimeSeconds, Date\.now\(\)\)/);
});

test("the web view gained no network permission and no Firebase SDK", () => {
  // Design D2, checked from three directions because this is the decision most likely to be
  // undone later by someone reaching for the Firebase JavaScript SDK.
  const config = JSON.parse(read("..", "src-tauri", "tauri.conf.json"));
  const csp = config.app?.security?.csp ?? "";
  assert.match(String(csp), /default-src 'self'/, "the CSP no longer confines the web view");

  const capabilities = JSON.parse(read("..", "src-tauri", "capabilities", "default.json"));
  const network = capabilities.permissions.filter((permission) => /^(http|websocket):/.test(permission));
  assert.deepEqual(network, [], `the web view gained network permissions: ${network}`);

  // Comments are excluded, and that exclusion is the point rather than a convenience:
  // `index.html` and `main.js` both explain at length *why* the Firebase JavaScript SDK is
  // not here, and naming it in that explanation is what keeps the decision from being
  // quietly reversed. A grep over raw text matched those comments and passed for the wrong
  // reason — so this strips comments first and then looks for real usage: an import, a
  // script tag, a global, or a fetch to Google's hosts.
  const stripComments = (source) =>
    source
      .replace(/<!--[\s\S]*?-->/g, "")
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .split("\n")
      .filter((line) => !line.trim().startsWith("//"))
      .join("\n");

  for (const [name, source] of [
    ["index.html", html],
    ["main.js", mainJs],
    ["logic.js", logicJs],
  ]) {
    const code = stripComments(source);

    for (const pattern of [
      /import\s[^;]*firebase/i,
      /from\s+["'][^"']*firebase/i,
      /<script[^>]+src=["'][^"']*(firebase|gstatic\.com|googleapis\.com)/i,
      /\bfirebase\s*\./i,
      /\bfirestore\s*\(/i,
      /fetch\(\s*["'`]https?:\/\//i,
      /new\s+(WebSocket|EventSource)\b/,
    ]) {
      assert.ok(
        !pattern.test(code),
        `${name} matches ${pattern}, so the web view reaches the network itself instead of \
going through a Tauri command`,
      );
    }
  }
});

test("the remote panel offers no way to power a machine off immediately", () => {
  // The whole surface exists to schedule things through the existing path. A control here
  // that shut a machine down at once would be the change that made the countdown optional,
  // and it would be reachable from the least trusted part of the app.
  const remote = html.slice(html.indexOf('id="remote"'), html.indexOf("</section>", html.indexOf('id="remote"')));

  for (const forbidden of ["power off now", "shut down now", "powerOffNow", "power_off_now"]) {
    assert.ok(
      !remote.toLowerCase().includes(forbidden.toLowerCase()),
      `the remote panel offers "${forbidden}"`,
    );
  }

  // And no command name in the whole frontend suggests one.
  const immediate = [...invokedNames].filter((name) => /now$|immediate/.test(name));
  assert.deepEqual(immediate, [], `commands that would skip the countdown: ${immediate}`);
});
