/*
 * The web view's DOM and IPC layer (section 12).
 *
 * Everything here either reads a control, writes a node, or calls Rust. Every decision the
 * user reads lives in `logic.js`, which has no DOM and no Tauri in it and is tested in
 * `logic.test.js`. Keeping that line sharp is the point: this file cannot be tested without
 * a running app, so as little as possible is allowed to accumulate in it.
 *
 * Three rules shape the whole file.
 *
 * It never decides *when* anything happens. Triggers are described here and resolved in
 * Rust (`resolve_trigger`), so the DST and rollover rules exist in one language. The
 * countdowns below are presentation of an instant Rust already computed.
 *
 * It cannot shut the machine down. There is no command for it. The only shutdown-related
 * thing reachable from here is `cancel_grace_period`, which can only prevent one.
 *
 * It holds no strings of its own. Static text is marked `data-i18n` in the HTML and filled
 * from the table; generated text goes through `t`. That is what lets the language change
 * without a reload, and it is why `applyLanguage` can simply re-render everything.
 */

import {
  accountPresentation,
  actionsFor,
  codeSecondsRemaining,
  computeClockSkewMs,
  countdownText,
  dashboardPresentation,
  degradedEntries,
  devicePresentation,
  formatRemaining,
  gracePercent,
  graceUnitLabel,
  isUnavailable,
  messageOf,
  needsAttention,
  normalizeTheme,
  pairingPresentation,
  permissionOutcome,
  previewText,
  quitPrompt,
  readTriggerFrom,
  resolveTheme,
  selectionPresentation,
  settingsSavedMessage,
} from "./logic.js";
import { DEFAULT_LANGUAGE, LANGUAGES, isSupportedLanguage, translator } from "./i18n.js";

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const el = (id) => document.getElementById(id);

/** Offset from the browser clock to Rust's, learned from the job list. */
let clockSkewMs = 0;
const nowMs = () => Date.now() + clockSkewMs;

/*
 * The active translator, replaced when the language changes. Held in one place rather than
 * threaded through every call site, because the alternative is that a function which forgot
 * to take it silently keeps rendering the old language.
 */
let t = translator(DEFAULT_LANGUAGE);

/* ------------------------------------------------------------------ */
/* Errors                                                             */
/* ------------------------------------------------------------------ */

function showError(node, message) {
  node.textContent = message;
  node.hidden = false;
}

function clearError(node) {
  node.textContent = "";
  node.hidden = true;
}

/* ------------------------------------------------------------------ */
/* Appearance and language                                             */
/* ------------------------------------------------------------------ */

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

/** The stored preference: "auto", "light", or "dark". */
let themePreference = "auto";

/**
 * Paints the resolved palette.
 *
 * The stylesheet keys off `data-theme`, so this is the single place the appearance is
 * decided — an explicit choice and a system-following choice cannot end up disagreeing
 * about what is on screen.
 */
function applyTheme() {
  const resolved = resolveTheme(themePreference, systemDark.matches);
  document.documentElement.dataset.theme = resolved;
}

/**
 * Rewrites every static string, then re-renders everything generated.
 *
 * Cheaper than it looks and much safer than patching individual nodes: the job list and the
 * previews are rebuilt from state that is already in memory, so nothing has to be fetched
 * and nothing can be left behind in the previous language.
 */
function applyLanguage(code) {
  t = translator(code);
  document.documentElement.lang = code;

  for (const node of document.querySelectorAll("[data-i18n]")) {
    node.textContent = t(node.dataset.i18n);
  }

  // Placeholders are a separate attribute because `textContent` is wrong for them: an input
  // has no text content to set, and marking it `data-i18n` would silently do nothing. Kept
  // to placeholders only — a label is a `<label>`, not a placeholder, since placeholder text
  // vanishes the moment the user types.
  for (const node of document.querySelectorAll("[data-i18n-placeholder]")) {
    node.placeholder = t(node.dataset.i18nPlaceholder);
  }

  // The tray button carries two different strings depending on whether a tray exists, and
  // the unavailable one is set from the capability report rather than from the HTML.
  if (trayUnavailable) applyTrayUnavailable();

  syncComposerPresentation();
  renderJobs();
  if (lastCapabilityReports) renderDegraded(lastCapabilityReports);
  for (const form of FORMS) refreshPreview(form);

  // The remote lists carry generated prose — presence labels and silence durations — so
  // they are rebuilt from state already in memory rather than re-fetched. The account line
  // and the code's expiry counter are the same.
  renderPairings();
  renderDevices();
  refreshAccount();
  tickPairingCode();
}

/* ------------------------------------------------------------------ */
/* The form (task 12.1)                                                */
/* ------------------------------------------------------------------ */

/*
 * The two job types are configured independently and submitted together. Describing them
 * with one table rather than two copies of the same wiring is what keeps "either or both"
 * from becoming "both, or whichever one was wired correctly".
 */
const FORMS = [
  {
    jobType: "keepAwake",
    want: "want-keep-awake",
    card: "keep-awake-card",
    body: "keep-awake-body",
    mode: "keep-awake-mode",
    durationField: "keep-awake-duration-field",
    minutes: "keep-awake-minutes",
    timeField: "keep-awake-time-field",
    time: "keep-awake-time",
    preview: "keep-awake-preview",
  },
  {
    jobType: "powerOff",
    want: "want-power-off",
    card: "power-off-card",
    body: "power-off-body",
    mode: "power-off-mode",
    durationField: "power-off-duration-field",
    minutes: "power-off-minutes",
    timeField: "power-off-time-field",
    time: "power-off-time",
    // Power-off only, so every read of these is guarded. There is no
    // `keep-awake-date` element, and an unguarded `el()` on it would return null and
    // break the "every element main.js reaches for exists" wiring test.
    dateField: "power-off-date-field",
    date: "power-off-date",
    preview: "power-off-preview",
  },
];

function readTrigger(form) {
  return readTriggerFrom({
    mode: el(form.mode).value,
    minutesValue: el(form.minutes).value,
    timeValue: el(form.time).value,
    dateValue: form.date ? el(form.date).value : "",
  });
}

function syncComposerPresentation() {
  const keepAwake = el("want-keep-awake").checked;
  const powerOff = el("want-power-off").checked;
  const presentation = selectionPresentation(keepAwake, powerOff, t);

  el("create-selection").textContent = presentation.summary;
  el("create-submit").textContent = presentation.submit;

  for (const form of FORMS) {
    const selected = el(form.want).checked;
    el(form.card).classList.toggle("is-selected", selected);
  }
}

/*
 * Shows the *resolved* instant under each control, which is task 11.2's reason for
 * existing. "For 120 minutes" is what the user typed; "until 20:12" is what will happen,
 * and only Rust knows which of those two crosses a DST boundary tonight.
 */
async function refreshPreview(form) {
  const preview = el(form.preview);

  if (!el(form.want).checked) {
    preview.textContent = "";
    return;
  }

  const trigger = readTrigger(form);
  if (!trigger) {
    preview.textContent = "";
    return;
  }

  if (trigger.kind === "indefinite") {
    preview.textContent = t("preview.indefinite");
    return;
  }

  try {
    const resolved = await invoke("resolve_trigger", { jobType: form.jobType, trigger });
    preview.textContent = previewText(form.jobType, resolved, nowMs(), t);
  } catch (error) {
    // A rejected trigger is worth saying now rather than on submit.
    preview.textContent = messageOf(error);
  }
}

/** Shows only the fields the chosen mode needs, and keeps the tab order honest. */
function syncFormVisibility(form) {
  const body = el(form.body);
  const checked = el(form.want).checked;

  body.hidden = !checked;
  if (checked) {
    body.removeAttribute("inert");
  } else {
    body.setAttribute("inert", "");
  }

  const mode = el(form.mode).value;
  el(form.durationField).hidden = mode !== "duration";
  el(form.timeField).hidden = mode !== "absoluteTime";
  if (form.dateField) {
    el(form.dateField).hidden = mode !== "absoluteTime";
  }
}

function wireForm(form) {
  const onChange = () => {
    syncFormVisibility(form);
    syncComposerPresentation();
    refreshPreview(form);
  };

  el(form.want).addEventListener("change", onChange);
  el(form.mode).addEventListener("change", onChange);

  /*
   * Ticking Power Off checks permission *silently* and, if consent is not yet
   * settled, says so on the card. No dialog is raised here — the point is that the
   * explanation lands before the OS asks, which the spec requires and which a
   * prompt fired from this handler could not guarantee.
   *
   * The real prompt happens in `submitForm`. That keeps the irreversible-action
   * consent tied to a deliberate submit rather than to ticking a checkbox, and by
   * then the user has read the sentence below.
   */
  if (form.jobType === "powerOff") {
    el(form.want).addEventListener("change", () => {
      const note = el("power-off-consent-note");
      if (!el(form.want).checked) {
        note.hidden = true;
        return;
      }

      checkShutdownPermission(false).then((permission) => {
        // Two conditions, both required. `promptsForConsent` is what makes the
        // sentence true at all — the Windows and Linux fallback carries a reason but
        // raises no dialog, so keying on the reason alone would tell a Windows user
        // that macOS is about to ask them something. And a machine that already has
        // consent needs no warning about a permission it holds.
        note.hidden = !(permission.promptsForConsent && permission.warning !== "");
        if (!permission.allow) showError(el("create-error"), permission.warning);
      });
    });
  }

  el(form.minutes).addEventListener("input", () => refreshPreview(form));
  el(form.time).addEventListener("input", () => refreshPreview(form));
  // `input` rather than `change`, so the preview — including a "that date has already
  // passed" rejection — updates as the date is picked rather than on blur.
  if (form.date) {
    el(form.date).addEventListener("input", () => refreshPreview(form));
  }

  onChange();
}

/* ------------------------------------------------------------------ */
/* Creating                                                            */
/* ------------------------------------------------------------------ */

let pendingReplacement = null;

/** Asks the replace dialog, resolving true only if the user confirmed (task 10.9). */
function confirmReplacement(message) {
  el("replace-body").textContent = message;

  return new Promise((resolve) => {
    pendingReplacement = resolve;
    el("replace-dialog").showModal();
  });
}

function settleReplacement(confirmed) {
  const resolve = pendingReplacement;
  pendingReplacement = null;

  const dialog = el("replace-dialog");
  if (dialog.open) dialog.close();

  if (resolve) resolve(confirmed);
}

async function createOne(jobType, trigger) {
  const response = await invoke("create_job", { jobType, trigger });
  if (response.result === "created") return;

  const confirmed = await confirmReplacement(response.message);
  if (!confirmed) return;

  await invoke("create_job_replacing", { jobType, trigger });
}

/*
 * Asks the OS for shutdown permission before a power-off job is created.
 *
 * The reason this is not left to execution time: on macOS the consent dialog is
 * raised by the first Apple Event the app sends, and the first one it sends is the
 * shutdown. A user who schedules 06:00 and goes to bed would get the prompt at
 * 06:00, with nobody there to answer, and wake to a machine still running. Asking
 * here means the dialog appears while they are looking at the screen.
 *
 * Returns an `{ allow, warning }` outcome, the same shape in every case including
 * a failure of the check itself — which is deliberately non-blocking, since a
 * broken diagnostic is not evidence that the shutdown will fail. See
 * `permissionOutcome`.
 *
 * `askUser` is the difference between explaining and prompting. Ticking Power Off
 * asks silently, so the card can say "macOS will ask for permission" *before* any
 * dialog exists; the submit path asks for real. Doing it the other way round would
 * put the OS dialog on screen ahead of the sentence explaining it.
 */
async function checkShutdownPermission(askUser) {
  try {
    const view = await invoke("check_shutdown_permission", { askUser });
    return permissionOutcome(view, t);
  } catch (error) {
    // The check broke, which is not evidence the shutdown will. Log and continue:
    // refusing to schedule here would turn a diagnostic failure into a lost job.
    console.warn("shutdown permission check failed:", messageOf(error));
    return { allow: true, warning: "" };
  }
}

async function submitForm(event) {
  event.preventDefault();
  const errorNode = el("create-error");
  clearError(errorNode);

  const wanted = FORMS.filter((form) => el(form.want).checked);
  if (wanted.length === 0) {
    showError(errorNode, t("error.chooseOne"));
    el(FORMS[0].want).focus();
    return;
  }

  const submit = el("create-submit");
  submit.disabled = true;

  try {
    // Before creating anything: if a power-off is wanted, confirm the OS will
    // actually let it run. `true` here may raise the consent dialog, which is
    // correct at this point — the card has already explained it is coming, and a
    // submit is the deliberate action it should be attached to.
    //
    // Done once, ahead of the loop, so the dialog cannot land in the middle of a
    // replace confirmation. Only an outright denial stops the submit; an `unknown`
    // verdict is already shown on the card and would be noise repeated here.
    if (wanted.some((form) => form.jobType === "powerOff")) {
      const permission = await checkShutdownPermission(true);
      if (!permission.allow) {
        showError(errorNode, permission.warning);
        el("want-power-off").focus();
        return;
      }
    }

    // Sequential rather than concurrent: each may need a confirmation dialog, and two
    // modal dialogs at once is not something a user can answer.
    for (const form of wanted) {
      const trigger = readTrigger(form);
      if (!trigger) {
        showError(errorNode, t("error.checkTime"));
        el(form.mode).focus();
        return;
      }
      await createOne(form.jobType, trigger);
    }
    await refreshJobs();
  } catch (error) {
    showError(errorNode, messageOf(error));
  } finally {
    submit.disabled = false;
  }
}

/* ------------------------------------------------------------------ */
/* The job list (task 12.2)                                            */
/* ------------------------------------------------------------------ */

let jobs = [];

function renderJobs() {
  const list = el("jobs-list");
  const status = el("jobs-status");
  const dashboard = dashboardPresentation(jobs, t);

  el("dashboard-status").textContent = dashboard.heading;
  el("dashboard-detail").textContent = dashboard.detail;
  el("dashboard-status").closest(".status-panel").dataset.state = dashboard.state;

  if (jobs.length === 0) {
    list.replaceChildren();
    status.textContent = t("jobs.empty");
    status.hidden = false;
    return;
  }

  status.textContent = "";
  status.hidden = true;
  list.replaceChildren(...jobs.map(renderJob));
}

function renderJob(job) {
  const item = document.createElement("li");
  const typeClass = job.jobType === "power_off" ? "job-power-off" : "job-keep-awake";
  item.className = `job ${typeClass}`;

  const title = document.createElement("p");
  title.className = "job-title";

  const name = document.createElement("span");
  name.textContent = job.jobTypeLabel;

  const statusTag = document.createElement("span");
  statusTag.className = "job-status";
  if (needsAttention(job.status)) statusTag.classList.add("job-status-attention");
  statusTag.textContent = job.statusLabel;

  title.append(name, statusTag);
  item.append(title);

  if (job.targetInstantUtc) {
    const countdown = document.createElement("p");
    countdown.className = "job-countdown";
    // Marked so the tick can rewrite this line without rebuilding the row, which would
    // steal focus from a button inside it.
    countdown.dataset.countdownFor = job.id;
    countdown.textContent = countdownText(job, nowMs(), t);
    item.append(countdown);
  }

  const detail = document.createElement("p");
  detail.className = "job-detail";
  detail.textContent = job.triggerLabel;
  item.append(detail);

  if (job.failureMessage) {
    const failure = document.createElement("p");
    failure.className = "job-failure";
    failure.textContent = job.failureMessage;
    item.append(failure);
  }

  const actions = actionsFor(job.status, t);
  if (actions.length > 0) {
    const wrapper = document.createElement("div");
    wrapper.className = "job-actions";
    wrapper.append(
      ...actions.map((action) => actionButton(action.label, () => act(action.command, job.id))),
    );
    item.append(wrapper);
  }

  return item;
}

function actionButton(label, onClick) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "button button-small";
  button.textContent = label;
  button.addEventListener("click", onClick);
  return button;
}

async function act(command, id) {
  try {
    await invoke(command, { id });
    await refreshJobs();
  } catch (error) {
    showError(el("create-error"), messageOf(error));
  }
}

async function refreshJobs() {
  try {
    jobs = await invoke("list_jobs");

    const skew = computeClockSkewMs(jobs, Date.now());
    if (skew !== null) clockSkewMs = skew;

    renderJobs();
  } catch (error) {
    el("jobs-status").textContent = messageOf(error);
  }
}

/*
 * The per-second tick. It only rewrites the countdown text of rows already on screen, so a
 * user tabbing through the buttons is not interrupted every second by a rebuild.
 */
function tickCountdowns() {
  const at = nowMs();
  for (const job of jobs) {
    const node = document.querySelector(`[data-countdown-for="${job.id}"]`);
    if (node) node.textContent = countdownText(job, at, t);
  }
}

/* ------------------------------------------------------------------ */
/* The shutdown countdown (task 12.3)                                  */
/* ------------------------------------------------------------------ */

let graceLength = 60;
let graceVisible = false;

async function refreshGrace() {
  let grace = null;
  try {
    grace = await invoke("grace_state");
  } catch {
    // A failed read must not remove a banner that may still be counting down.
    return;
  }

  const banner = el("grace");

  if (!grace) {
    if (graceVisible) {
      banner.hidden = true;
      graceVisible = false;
      // The list has changed underneath: the job either powered off or was cancelled.
      refreshJobs();
    }
    return;
  }

  const seconds = grace.secondsRemaining;
  el("grace-seconds").textContent = String(seconds);
  el("grace-unit").textContent = graceUnitLabel(seconds, t);

  const bar = el("grace-bar");
  bar.setAttribute("aria-valuemax", String(graceLength));
  bar.setAttribute("aria-valuenow", String(seconds));
  // The bar's own value would be read as a bare number. This says what it counts.
  bar.setAttribute(
    "aria-valuetext",
    t("grace.valueText", { remaining: formatRemaining(seconds, t) }),
  );
  el("grace-bar-fill").style.width = `${gracePercent(seconds, graceLength)}%`;

  if (!graceVisible) {
    banner.hidden = false;
    graceVisible = true;
    // Focus goes to the cancel button, not the banner: a user who reaches the window
    // during a countdown can then stop it with one key rather than tabbing to find it.
    el("grace-cancel").focus();
  }
}

async function cancelGrace() {
  try {
    await invoke("cancel_grace_period");
  } catch (error) {
    showError(el("create-error"), messageOf(error));
  }
  await refreshGrace();
  await refreshJobs();
}

/* ------------------------------------------------------------------ */
/* Degraded capabilities (task 12.4)                                   */
/* ------------------------------------------------------------------ */

/* Kept so a language change can redraw these without a second IPC round trip. */
let lastCapabilityReports = null;
let trayUnavailable = false;

function renderDegraded(reports) {
  const entries = degradedEntries(reports, t);
  const section = el("degraded");

  if (entries.length === 0) {
    section.hidden = true;
    return;
  }

  el("degraded-list").replaceChildren(
    ...entries.map((entry) => {
      const item = document.createElement("li");

      const headline = document.createElement("span");
      headline.className = "degraded-what";
      headline.textContent = entry.headline;
      item.append(headline);

      for (const text of [entry.consequence, entry.reason]) {
        if (!text) continue;
        const line = document.createElement("span");
        line.className = "degraded-consequence";
        line.textContent = text;
        item.append(line);
      }

      return item;
    }),
  );

  section.hidden = false;
}

/** Disables the hide button and the badge when there is no tray to get the window back from. */
function applyTrayUnavailable() {
  const button = el("hide-window");
  button.disabled = true;
  button.textContent = t("footer.hideUnavailable");

  el("tray-badge").dataset.state = "unavailable";
}

function applyTrayState(reports) {
  if (!isUnavailable(reports, "tray")) return;

  trayUnavailable = true;
  applyTrayUnavailable();
}

/* ------------------------------------------------------------------ */
/* Settings                                                            */
/* ------------------------------------------------------------------ */

let savedSettings = null;

async function loadSettings() {
  const [settings, zones] = await Promise.all([
    invoke("get_settings"),
    invoke("available_timezones"),
  ]);
  savedSettings = settings;

  el("timezone").replaceChildren(
    ...zones.map((zone) => {
      const option = document.createElement("option");
      option.value = zone;
      option.textContent = zone;
      option.selected = zone === settings.timezone;
      return option;
    }),
  );

  el("notifications-enabled").checked = settings.notificationsEnabled;

  // Appearance and language are applied before anything else is drawn, so the window never
  // appears in the wrong palette or the wrong language and then corrects itself.
  themePreference = normalizeTheme(settings.theme);
  el(`theme-${themePreference}`).checked = true;
  applyTheme();

  const language = isSupportedLanguage(settings.language)
    ? settings.language
    : DEFAULT_LANGUAGE;
  el("language").replaceChildren(
    ...LANGUAGES.map((entry) => {
      const option = document.createElement("option");
      option.value = entry.code;
      option.textContent = entry.label;
      option.selected = entry.code === language;
      return option;
    }),
  );
  applyLanguage(language);
}

/** Everything `save_settings` needs, read from the controls. */
function readSettings() {
  return {
    timezone: el("timezone").value,
    notificationsEnabled: el("notifications-enabled").checked,
    theme: themePreference,
    language: el("language").value,
  };
}

async function saveSettings() {
  const errorNode = el("settings-error");
  clearError(errorNode);

  const settings = readSettings();

  try {
    const moved = await invoke("save_settings", { settings });
    savedSettings = settings;
    el("settings-status").textContent = settingsSavedMessage(moved, t);

    await refreshJobs();
    for (const form of FORMS) refreshPreview(form);
  } catch (error) {
    showError(errorNode, messageOf(error));
    // Put the controls back to what is actually stored, so they never show a setting the
    // app is not using.
    if (savedSettings) restoreControlsFrom(savedSettings);
  }
}

function restoreControlsFrom(settings) {
  el("timezone").value = settings.timezone;
  el("notifications-enabled").checked = settings.notificationsEnabled;

  themePreference = normalizeTheme(settings.theme);
  el(`theme-${themePreference}`).checked = true;
  applyTheme();

  const language = isSupportedLanguage(settings.language) ? settings.language : DEFAULT_LANGUAGE;
  el("language").value = language;
  applyLanguage(language);
}

/*
 * The theme is repainted before the save, not after. A user clicking Dark expects the window
 * to change immediately; waiting for a database write would make the control feel broken, and
 * a failed save restores the old value anyway.
 */
async function onThemeChange(event) {
  themePreference = normalizeTheme(event.target.value);
  applyTheme();
  await saveSettings();
}

async function onLanguageChange(event) {
  applyLanguage(event.target.value);
  await saveSettings();
}

async function loadAutostart() {
  try {
    el("autostart-enabled").checked = await invoke("autostart_enabled");
  } catch (error) {
    showError(el("settings-error"), messageOf(error));
  }
}

async function saveAutostart(event) {
  const wanted = event.target.checked;
  clearError(el("settings-error"));

  try {
    await invoke("set_autostart_enabled", { enabled: wanted });
    el("settings-status").textContent = wanted
      ? t("settings.autostartOn")
      : t("settings.autostartOff");
  } catch (error) {
    showError(el("settings-error"), messageOf(error));
    // The OS refused, so the switch has to go back: a checkbox left on would claim
    // something that is not true.
    event.target.checked = !wanted;
  }
}

/* ------------------------------------------------------------------ */
/* Window and quit                                                     */
/* ------------------------------------------------------------------ */

async function hideWindow() {
  try {
    await invoke("hide_to_tray");
  } catch (error) {
    showError(el("settings-error"), messageOf(error));
  }
}

/*
 * Task 9.3. The tray asks; this answers. The tray itself never exits the process — it
 * emits this event and waits, so the confirmation cannot be skipped by closing the window.
 */
async function onConfirmQuit() {
  let active = true;
  try {
    active = await invoke("has_active_job");
  } catch {
    // If we cannot tell, assume something is running: the cautious wording costs a click,
    // and the other way round loses a scheduled shutdown without warning.
    active = true;
  }

  el("quit-body").textContent = quitPrompt(active, t);

  const dialog = el("quit-dialog");
  if (!dialog.open) dialog.showModal();
}

/* ------------------------------------------------------------------ */
/* The remote surface (tasks 7.2, 7.3)                                 */
/* ------------------------------------------------------------------ */

/*
 * Everything below reaches Rust through Tauri commands, exactly as the job list does.
 * There is no Firebase JavaScript SDK imported here and the CSP still reads
 * `default-src 'self'` — design D2.
 *
 * Presence is *never computed here*. Rust runs `evaluate_presence` over the instant the
 * transport reported and sends a state; this file renders it. Writing the 90-second
 * threshold in JavaScript would be a third implementation of a rule the shared vectors pin
 * for two, and the untested one would be this.
 */

/** The last device and pairing lists, kept so a language change can re-render them. */
let lastDevices = [];
let lastPairings = [];
/** When the displayed pairing code was shown, and how long Rust said it lives. */
let codeShownAtMs = null;
let codeLifetimeSeconds = 0;

async function refreshAccount() {
  try {
    const view = await invoke("account_state");
    const presentation = accountPresentation(view, t);

    el("account-label").textContent = presentation.label;
    el("account-device-id").textContent = presentation.deviceId
      ? `${t("remote.deviceId")}: ${presentation.deviceId}`
      : "";

    // Only one of the two makes sense at a time. Both visible would leave the user
    // guessing which state they are in.
    el("account-sign-in").hidden = presentation.signedIn;
    el("account-sign-out").hidden = !presentation.signedIn;
  } catch (error) {
    // The pairing surface is optional: a machine with no usable identity has no
    // `RemoteSurface`, and the command says so. Reported in the panel rather than
    // thrown, because local scheduling is unaffected.
    el("account-label").textContent = messageOf(error);
    el("account-sign-in").hidden = true;
    el("account-sign-out").hidden = true;
  }
}

async function signIn() {
  try {
    await invoke("sign_in_to_account");
    await refreshAccount();
  } catch (error) {
    showError(el("pairing-error"), messageOf(error));
  }
}

async function signOut() {
  try {
    await invoke("sign_out_of_account");
    await refreshAccount();
    // Re-read the pairings straight afterwards. The claim that signing out leaves them
    // alone is worth showing rather than asserting: the list is on screen, unchanged.
    await refreshPairings();
  } catch (error) {
    showError(el("pairing-error"), messageOf(error));
  }
}

async function showPairingCode() {
  clearError(el("pairing-error"));
  try {
    const view = await invoke("present_pairing_code");
    el("pairing-code").textContent = view.code;
    codeShownAtMs = Date.now();
    codeLifetimeSeconds = view.expiresInSeconds;
    tickPairingCode();
  } catch (error) {
    showError(el("pairing-error"), messageOf(error));
  }
}

/*
 * Counts the displayed code down from the instant it was shown, rather than decrementing a
 * counter. A backgrounded web view stops decrementing, and the user would then be reading a
 * code the machine has already forgotten.
 */
function tickPairingCode() {
  if (codeShownAtMs === null) return;

  const left = codeSecondsRemaining(codeShownAtMs, codeLifetimeSeconds, Date.now());
  el("pairing-code-expiry").textContent =
    left > 0 ? t("remote.codeExpiresIn", { seconds: left }) : t("remote.codeExpired");
}

async function submitPairingCode(event) {
  event.preventDefault();
  clearError(el("pairing-error"));
  el("pairing-status").textContent = "";

  try {
    // Passed through verbatim. Normalisation, the alphabet check, and the length check all
    // live in `PairingCode::parse` in Rust — validating here would be a second, divergent
    // definition of what a code is.
    const result = await invoke("accept_pairing_code", {
      code: el("pairing-code-entry").value,
      peerDeviceId: el("pairing-peer-id").value.trim(),
      peerVerifyingKey: el("pairing-peer-key").value.trim(),
    });

    el("pairing-status").textContent = result.message;
    if (result.paired) {
      el("pairing-code-entry").value = "";
      el("pairing-peer-id").value = "";
      el("pairing-peer-key").value = "";
    }
    await refreshPairings();
    await refreshDevices();
  } catch (error) {
    showError(el("pairing-error"), messageOf(error));
  }
}

async function refreshPairings() {
  try {
    lastPairings = await invoke("list_pairings");
    renderPairings();
  } catch (error) {
    el("pairings-status").textContent = messageOf(error);
    el("pairings-status").hidden = false;
  }
}

function renderPairings() {
  const list = el("pairings-list");
  list.replaceChildren();

  const status = el("pairings-status");
  if (lastPairings.length === 0) {
    status.textContent = t("remote.pairingsEmpty");
    status.hidden = false;
    return;
  }
  status.hidden = true;

  for (const pairing of lastPairings) {
    list.append(pairingRow(pairingPresentation(pairing, t)));
  }
}

function pairingRow(view) {
  const item = document.createElement("li");
  item.className = "job-item";
  if (view.revoked) item.classList.add("job-item-revoked");
  // Read by the presence tests and by the stylesheet, so the three states are
  // distinguishable without relying on colour alone.
  item.dataset.presence = view.presence;

  const title = document.createElement("p");
  title.className = "job-title";

  const name = document.createElement("span");
  name.textContent = view.peer;

  const presence = document.createElement("span");
  presence.className = "job-status";
  presence.textContent = view.presenceLabel;

  const paired = document.createElement("span");
  paired.className = "job-status";
  paired.textContent = view.statusLabel;

  title.append(name, presence, paired);
  item.append(title);

  const detail = document.createElement("p");
  detail.className = "job-detail";
  detail.textContent = view.detail;
  item.append(detail);

  // A revoked pairing offers no unpair button: there is nothing left to withdraw.
  if (view.canRevoke) {
    const actions = document.createElement("div");
    actions.className = "job-actions";
    actions.append(actionButton(t("remote.revoke"), () => revokePairing(view.peer)));
    item.append(actions);
  }

  return item;
}

async function revokePairing(peer) {
  try {
    await invoke("revoke_pairing", { peer });
    await refreshPairings();
    await refreshDevices();
  } catch (error) {
    showError(el("pairing-error"), messageOf(error));
  }
}

async function refreshDevices() {
  try {
    lastDevices = await invoke("list_devices");
    renderDevices();
  } catch (error) {
    el("devices-status").textContent = messageOf(error);
    el("devices-status").hidden = false;
  }
}

function renderDevices() {
  const list = el("devices-list");
  list.replaceChildren();

  const status = el("devices-status");
  if (lastDevices.length === 0) {
    // "Cannot reach the list" rather than "you have no devices". A machine with no relay
    // configured cannot answer the question, and claiming an empty account would be a
    // different and wrong answer.
    status.textContent = t("remote.devicesUnavailable");
    status.hidden = false;
    return;
  }
  status.hidden = true;

  for (const device of lastDevices) {
    list.append(deviceRow(devicePresentation(device, t)));
  }
}

function deviceRow(view) {
  const item = document.createElement("li");
  item.className = "job-item";
  item.dataset.presence = view.presence;

  const title = document.createElement("p");
  title.className = "job-title";

  const name = document.createElement("span");
  name.textContent = view.isThisDevice ? `${view.name} (${t("remote.thisDevice")})` : view.name;

  const presence = document.createElement("span");
  presence.className = "job-status";
  presence.textContent = view.presenceLabel;

  const paired = document.createElement("span");
  paired.className = "job-status";
  paired.textContent = view.pairedLabel;

  title.append(name, presence, paired);
  item.append(title);

  const detail = document.createElement("p");
  detail.className = "job-detail";
  detail.textContent = view.detail;
  item.append(detail);

  return item;
}

async function loadRemoteControl() {
  try {
    el("remote-control-enabled").checked = await invoke("remote_control_enabled");
  } catch (error) {
    // Left unchecked, which is the safe direction: the checkbox agrees with the default a
    // machine that has never been configured holds.
    el("remote-control-enabled").checked = false;
    console.warn("could not read the remote-control setting", messageOf(error));
  }
}

async function saveRemoteControl() {
  const wanted = el("remote-control-enabled").checked;
  try {
    // Read back rather than assumed. A write that silently did nothing must not leave the
    // checkbox claiming otherwise — this setting decides whether the machine obeys peers.
    el("remote-control-enabled").checked = await invoke("set_remote_control_enabled", {
      enabled: wanted,
    });
    clearError(el("settings-error"));
  } catch (error) {
    showError(el("settings-error"), messageOf(error));
    await loadRemoteControl();
  }
}

/* ------------------------------------------------------------------ */
/* Boot                                                               */
/* ------------------------------------------------------------------ */

async function boot() {
  el("create-form").addEventListener("submit", submitForm);
  for (const form of FORMS) wireForm(form);

  el("grace-cancel").addEventListener("click", cancelGrace);
  el("hide-window").addEventListener("click", hideWindow);

  el("timezone").addEventListener("change", saveSettings);
  el("notifications-enabled").addEventListener("change", saveSettings);
  el("autostart-enabled").addEventListener("change", saveAutostart);
  // Its own command, not part of `saveSettings`. A stale form posting the whole settings
  // object back would silently re-enable a setting the user had just turned off.
  el("remote-control-enabled").addEventListener("change", saveRemoteControl);
  el("language").addEventListener("change", onLanguageChange);
  for (const theme of ["auto", "light", "dark"]) {
    el(`theme-${theme}`).addEventListener("change", onThemeChange);
  }

  // Only matters while the preference is "auto", and `applyTheme` already checks that.
  systemDark.addEventListener("change", applyTheme);

  el("quit-cancel").addEventListener("click", () => el("quit-dialog").close());
  el("quit-confirm").addEventListener("click", () => invoke("quit_app"));

  el("account-sign-in").addEventListener("click", signIn);
  el("account-sign-out").addEventListener("click", signOut);
  el("pairing-code-show").addEventListener("click", showPairingCode);
  el("pairing-form").addEventListener("submit", submitPairingCode);

  el("replace-confirm").addEventListener("click", () => settleReplacement(true));
  // Escape closes a native dialog without either button being pressed, so the promise is
  // settled from the close event rather than the cancel button. Otherwise the submit loop
  // would wait forever and the Start button would stay disabled.
  el("replace-cancel").addEventListener("click", () => el("replace-dialog").close());
  el("replace-dialog").addEventListener("close", () => settleReplacement(false));

  await listen("weakup://confirm-quit", onConfirmQuit);

  try {
    graceLength = await invoke("grace_period_length");
  } catch {
    // Keep the default of 60. This number only scales the bar; the seconds are text.
  }

  // Settings first, so the window is painted in the stored theme and language before the
  // rest of the content lands in it.
  await loadSettings();

  try {
    const reports = await invoke("capability_state");
    lastCapabilityReports = reports;
    renderDegraded(reports);
    applyTrayState(reports);
  } catch (error) {
    showError(el("settings-error"), messageOf(error));
  }

  await loadAutostart();
  await loadRemoteControl();
  await refreshAccount();
  await refreshPairings();
  await refreshDevices();
  await refreshJobs();
  await refreshGrace();
  for (const form of FORMS) refreshPreview(form);

  // One interval for both per-second jobs. The countdown redraw is local; only the grace
  // read goes over IPC, and it returns a two-field struct or null.
  setInterval(() => {
    tickCountdowns();
    // Local too: derived from the instant the code was shown, so it stays correct across a
    // backgrounded web view rather than drifting like a decremented counter.
    tickPairingCode();
    refreshGrace();
  }, 1000);

  // Slower, because this is only needed for state the tick cannot infer: a job the
  // scheduler completed, failed, or marked overdue on its own.
  setInterval(refreshJobs, 5000);

  // Slower again. Presence changes on a 60-second heartbeat, so a faster poll would spend
  // IPC calls to redraw the same three states.
  setInterval(() => {
    refreshPairings();
    refreshDevices();
  }, 15000);
}

boot().catch((error) => {
  // A failure here leaves a window whose controls do nothing, so it has to be visible in
  // the window rather than only in a console the user will never open.
  showError(el("create-error"), t("error.bootFailed", { message: messageOf(error) }));
});
