/*
 * The web view's DOM and IPC layer (section 12).
 *
 * Everything here either reads a control, writes a node, or calls Rust. Every decision the
 * user reads lives in `logic.js`, which has no DOM and no Tauri in it and is tested in
 * `logic.test.js`. Keeping that line sharp is the point: this file cannot be tested without
 * a running app, so as little as possible is allowed to accumulate in it.
 *
 * Two rules shape the whole file.
 *
 * It never decides *when* anything happens. Triggers are described here and resolved in
 * Rust (`resolve_trigger`), so the DST and rollover rules exist in one language. The
 * countdowns below are presentation of an instant Rust already computed.
 *
 * It cannot shut the machine down. There is no command for it. The only shutdown-related
 * thing reachable from here is `cancel_grace_period`, which can only prevent one.
 */

import {
  actionsFor,
  computeClockSkewMs,
  countdownText,
  degradedEntries,
  formatRemaining,
  gracePercent,
  graceUnitLabel,
  isUnavailable,
  messageOf,
  needsAttention,
  previewText,
  quitPrompt,
  readTriggerFrom,
  settingsSavedMessage,
} from "./logic.js";

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const el = (id) => document.getElementById(id);

/** Offset from the browser clock to Rust's, learned from the job list. */
let clockSkewMs = 0;
const nowMs = () => Date.now() + clockSkewMs;

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
    body: "power-off-body",
    mode: "power-off-mode",
    durationField: "power-off-duration-field",
    minutes: "power-off-minutes",
    timeField: "power-off-time-field",
    time: "power-off-time",
    preview: "power-off-preview",
  },
];

function readTrigger(form) {
  return readTriggerFrom({
    mode: el(form.mode).value,
    minutesValue: el(form.minutes).value,
    timeValue: el(form.time).value,
  });
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
    preview.textContent = "Runs until you turn it off.";
    return;
  }

  try {
    const resolved = await invoke("resolve_trigger", { jobType: form.jobType, trigger });
    preview.textContent = previewText(form.jobType, resolved, nowMs());
  } catch (error) {
    // A rejected trigger is worth saying now rather than on submit.
    preview.textContent = messageOf(error);
  }
}

/** Shows only the fields the chosen mode needs, and keeps the tab order honest. */
function syncFormVisibility(form) {
  const body = el(form.body);

  // `inert` rather than a class: it takes the fields out of the tab order too, so a
  // keyboard user does not tab through inputs for a job they are not creating.
  if (el(form.want).checked) {
    body.removeAttribute("inert");
  } else {
    body.setAttribute("inert", "");
  }

  const mode = el(form.mode).value;
  el(form.durationField).hidden = mode !== "duration";
  el(form.timeField).hidden = mode !== "absoluteTime";
}

function wireForm(form) {
  const onChange = () => {
    syncFormVisibility(form);
    refreshPreview(form);
  };

  el(form.want).addEventListener("change", onChange);
  el(form.mode).addEventListener("change", onChange);
  el(form.minutes).addEventListener("input", () => refreshPreview(form));
  el(form.time).addEventListener("input", () => refreshPreview(form));

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

async function submitForm(event) {
  event.preventDefault();
  const errorNode = el("create-error");
  clearError(errorNode);

  const wanted = FORMS.filter((form) => el(form.want).checked);
  if (wanted.length === 0) {
    showError(errorNode, "Choose at least one of the two.");
    el(FORMS[0].want).focus();
    return;
  }

  const submit = el("create-submit");
  submit.disabled = true;

  try {
    // Sequential rather than concurrent: each may need a confirmation dialog, and two
    // modal dialogs at once is not something a user can answer.
    for (const form of wanted) {
      const trigger = readTrigger(form);
      if (!trigger) {
        showError(errorNode, "Check the time or the number of minutes.");
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

  if (jobs.length === 0) {
    list.replaceChildren();
    status.textContent = "Nothing scheduled.";
    return;
  }

  status.textContent = "";
  list.replaceChildren(...jobs.map(renderJob));
}

function renderJob(job) {
  const item = document.createElement("li");
  item.className = "job";

  const title = document.createElement("p");
  title.className = "job-title";

  const name = document.createElement("span");
  name.textContent = job.jobTypeLabel;

  const statusTag = document.createElement("span");
  statusTag.className = "job-status";
  if (needsAttention(job.status)) statusTag.classList.add("job-status-attention");
  statusTag.textContent = job.statusLabel;

  title.append(name, statusTag);

  const detail = document.createElement("p");
  detail.className = "job-detail";
  detail.textContent = job.triggerLabel;

  item.append(title, detail);

  if (job.targetInstantUtc) {
    const countdown = document.createElement("p");
    countdown.className = "job-detail job-countdown";
    // Marked so the tick can rewrite this line without rebuilding the row, which would
    // steal focus from a button inside it.
    countdown.dataset.countdownFor = job.id;
    countdown.textContent = countdownText(job, nowMs());
    item.append(countdown);
  }

  if (job.failureMessage) {
    const failure = document.createElement("p");
    failure.className = "job-failure";
    failure.textContent = job.failureMessage;
    item.append(failure);
  }

  const actions = actionsFor(job.status);
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
    if (node) node.textContent = countdownText(job, at);
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
  el("grace-unit").textContent = graceUnitLabel(seconds);

  const bar = el("grace-bar");
  bar.setAttribute("aria-valuemax", String(graceLength));
  bar.setAttribute("aria-valuenow", String(seconds));
  // The bar's own value would be read as a bare number. This says what it counts.
  bar.setAttribute("aria-valuetext", `${formatRemaining(seconds)} until this computer shuts down`);
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

function renderDegraded(reports) {
  const entries = degradedEntries(reports);
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

/** Disables the hide button when there is no tray to get the window back from. */
function applyTrayState(reports) {
  if (!isUnavailable(reports, "tray")) return;

  const button = el("hide-window");
  button.disabled = true;
  button.textContent = "Close to the tray (no tray on this computer)";
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
}

async function saveSettings() {
  const errorNode = el("settings-error");
  clearError(errorNode);

  const settings = {
    timezone: el("timezone").value,
    notificationsEnabled: el("notifications-enabled").checked,
  };

  try {
    const moved = await invoke("save_settings", { settings });
    savedSettings = settings;
    el("settings-status").textContent = settingsSavedMessage(moved);

    await refreshJobs();
    for (const form of FORMS) refreshPreview(form);
  } catch (error) {
    showError(errorNode, messageOf(error));
    // Put the controls back to what is actually stored, so they never show a setting the
    // app is not using.
    if (savedSettings) {
      el("timezone").value = savedSettings.timezone;
      el("notifications-enabled").checked = savedSettings.notificationsEnabled;
    }
  }
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
      ? "Weakup will start when you log in."
      : "Weakup will not start when you log in.";
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

  el("quit-body").textContent = quitPrompt(active);

  const dialog = el("quit-dialog");
  if (!dialog.open) dialog.showModal();
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

  el("quit-cancel").addEventListener("click", () => el("quit-dialog").close());
  el("quit-confirm").addEventListener("click", () => invoke("quit_app"));

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

  try {
    const reports = await invoke("capability_state");
    renderDegraded(reports);
    applyTrayState(reports);
  } catch (error) {
    showError(el("settings-error"), messageOf(error));
  }

  await loadSettings();
  await loadAutostart();
  await refreshJobs();
  await refreshGrace();
  for (const form of FORMS) refreshPreview(form);

  // One interval for both per-second jobs. The countdown redraw is local; only the grace
  // read goes over IPC, and it returns a two-field struct or null.
  setInterval(() => {
    tickCountdowns();
    refreshGrace();
  }, 1000);

  // Slower, because this is only needed for state the tick cannot infer: a job the
  // scheduler completed, failed, or marked overdue on its own.
  setInterval(refreshJobs, 5000);
}

boot().catch((error) => {
  // A failure here leaves a window whose controls do nothing, so it has to be visible in
  // the window rather than only in a console the user will never open.
  showError(el("create-error"), `Weakup could not start up properly: ${messageOf(error)}`);
});
