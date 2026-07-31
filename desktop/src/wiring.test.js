/*
 * The seams between the web view and everything either side of it.
 *
 * `logic.test.js` covers the decisions. This file covers the joins, which are what actually
 * broke during development: `main.js` referenced `#boot-status` for a while after the
 * element was renamed out of `index.html`, and nothing failed — the window simply came up
 * with dead controls. A typo in an element id or a command name is invisible until someone
 * runs the app and clicks the thing.
 *
 * So these tests read the real files as text and check the three lists agree: the ids
 * `main.js` asks the DOM for, the commands it invokes, and the commands Rust registers.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

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
  const shutdownish = [...invokedNames].filter((name) =>
    /^(power_off|shutdown|force_|execute_power)/.test(name),
  );
  assert.deepEqual(shutdownish, [], `the UI can trigger a shutdown directly: ${shutdownish}`);

  // And the only grace-related thing it can do is stop one.
  const graceCommands = [...invokedNames].filter((name) => name.includes("grace")).sort();
  assert.deepEqual(graceCommands, ["cancel_grace_period", "grace_period_length", "grace_state"]);
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
    .filter((tag) => !/type="(checkbox|submit|hidden)"/.test(tag))
    .map((tag) => (tag.match(/\bid="([^"]+)"/) ?? [])[1])
    .filter((id) => id && !labelled.has(id));

  assert.deepEqual(unlabelled, [], `controls with no <label for>: ${unlabelled}`);
});

test("the checkboxes are wrapped in their labels", () => {
  // The two job checkboxes use the wrapping form instead, which is equally valid and gives
  // a larger hit area. Checking they are wrapped rather than bare.
  for (const id of ["want-keep-awake", "want-power-off", "notifications-enabled", "autostart-enabled"]) {
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
  const mainIndex = html.indexOf("<main>");
  const banner = html.slice(graceIndex, html.indexOf("</section>", graceIndex));
  assert.ok(graceIndex > 0 && graceIndex < mainIndex, "the grace banner is not before the app");
  assert.match(banner, /role="alert"/, "the banner is not announced when it appears");
  assert.match(banner, /aria-valuemin/, "the progress bar has no announced range");
  assert.match(banner, /id="grace-cancel"/, "the banner has no direct cancel control");
});

test("secondary settings use a native disclosure", () => {
  const settings = html.slice(
    html.indexOf('id="settings-panel"'),
    html.indexOf("</details>", html.indexOf('id="settings-panel"')),
  );
  assert.match(settings, /^id="settings-panel" class="settings-panel">\s*<summary>/);
  assert.match(settings, /id="timezone"/);
  assert.match(settings, /id="notifications-enabled"/);
  assert.match(settings, /id="autostart-enabled"/);
});

test("the dashboard retains semantic landmarks", () => {
  assert.match(html, /<header class="app-header">/);
  assert.match(html, /class="status-panel"[^>]*aria-labelledby="dashboard-status"/);
  assert.match(html, /id="create"[^>]*class="composer"[^>]*aria-labelledby="create-heading"/);
  assert.match(html, /class="jobs-section"[^>]*aria-labelledby="jobs-heading"/);
  assert.match(html, /<footer class="page-footer">/);
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
