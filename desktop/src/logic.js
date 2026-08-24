/*
 * The web view's pure logic, separated from the DOM and from Tauri.
 *
 * This split exists for one reason: everything in here is a decision the user reads. How
 * long is left, whether a shutdown already passed, what a broken capability costs them.
 * In `main.js` those computations sit inside functions that need a live window and a
 * running Rust core, which makes them functions nobody tests. Here they take values and
 * return values, so `logic.test.js` can call them directly.
 *
 * Nothing in this file touches `document`, `window`, or `invoke`.
 *
 * Every function that produces text takes a translator `t` as its first argument rather
 * than reaching for a module-level "current language". A global would make these functions
 * depend on assignment order — and would make the tests below pass or fail depending on
 * which language a previous test happened to leave set.
 */

import { translator } from "./i18n.js";

/** The translator used when a caller has none, so a bare call still returns English. */
const defaultT = translator("en");

/* ------------------------------------------------------------------ */
/* Clock skew                                                          */
/* ------------------------------------------------------------------ */

/**
 * Milliseconds to add to `Date.now()` to get the clock Rust is using.
 *
 * The countdown is derived from the absolute target instant rather than a decremented
 * counter — a counter drifts, and a backgrounded web view stops decrementing entirely.
 * But the target was computed against Rust's clock and is compared against the web view's,
 * and a fraction of a second of disagreement makes the last tick before zero read wrong.
 * Rust sends the remaining seconds alongside the target, so the difference between them is
 * its own "now", and one subtraction removes the skew.
 *
 * Returns null when no job carries both halves, meaning: keep whatever we had.
 */
export function computeClockSkewMs(jobs, browserNowMs) {
  const dated = jobs.find(
    (job) => job.targetInstantUtc && typeof job.remainingSeconds === "number",
  );
  if (!dated) return null;

  const target = Date.parse(dated.targetInstantUtc);
  if (Number.isNaN(target)) return null;

  return target - dated.remainingSeconds * 1000 - browserNowMs;
}

/* ------------------------------------------------------------------ */
/* Formatting                                                          */
/* ------------------------------------------------------------------ */

/*
 * Pluralisation goes through the string table rather than appending "s", because the "s"
 * only works in English. Vietnamese uses one word for both, and the table simply defines
 * the two keys identically.
 */
const unit = (t, value, singular, plural) =>
  `${value} ${t(value === 1 ? singular : plural)}`;

/**
 * Plain language for a span of seconds: "1 hour 5 minutes", not "01:05:00".
 *
 * Coarser as the span grows. Seconds are dropped past an hour because a user reading
 * "2 hours 14 minutes 3 seconds" does not care about the 3, and the digit changing every
 * second draws the eye to the least important part of the line.
 */
export function formatRemaining(totalSeconds, t = defaultT) {
  const seconds = Math.max(0, Math.round(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;

  if (hours > 0) {
    return `${unit(t, hours, "time.hour", "time.hours")} ${unit(
      t,
      minutes,
      "time.minute",
      "time.minutes",
    )}`;
  }
  if (minutes > 0) {
    return `${unit(t, minutes, "time.minute", "time.minutes")} ${unit(
      t,
      rest,
      "time.second",
      "time.seconds",
    )}`;
  }
  return unit(t, rest, "time.second", "time.seconds");
}

/**
 * The target as a wall-clock time, with the date added only when it is not today.
 *
 * "Shuts down at 00:30" is ambiguous at 23:50 in a way "at 00:30 on 31/07" is not, and
 * that ambiguity is in front of an irreversible action.
 */
export function formatInstant(rfc3339, nowMs, t = defaultT) {
  const date = new Date(rfc3339);
  if (Number.isNaN(date.getTime())) return String(rfc3339);

  const time = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const isToday = date.toDateString() === new Date(nowMs).toDateString();
  return isToday
    ? time
    : t("time.at", { time, date: date.toLocaleDateString() });
}

/**
 * Every command rejects with `AppError::user_message()` — prose written for a person. So
 * the message is surfaced as-is rather than replaced with "Something went wrong", which
 * throws away the only part that tells the user what to do about it.
 *
 * Not translated: the text comes from Rust, which does not know the chosen language. A
 * lookup here would replace a specific message with a generic one.
 */
export function messageOf(error) {
  if (typeof error === "string") return error;
  if (error && typeof error.message === "string") return error.message;
  return String(error);
}

/* ------------------------------------------------------------------ */
/* The form                                                            */
/* ------------------------------------------------------------------ */

/**
 * Builds the `TriggerInput` Rust deserialises, from what the controls hold.
 *
 * Returns null for input Rust cannot be asked about — an empty time field, a cleared
 * number. Null means "say nothing yet", not "invalid": the user is mid-typing, and an
 * error under a field they have not finished filling is noise.
 *
 * What it deliberately does not do is decide whether the values are *acceptable*. 500000
 * minutes and 25:00 both come back as well-formed triggers here and are rejected by the
 * resolver, so the bounds live in one language (task 11.2). The date is no exception:
 * a malformed one is passed through and refused by Rust in prose.
 */
export function readTriggerFrom({ mode, minutesValue, timeValue, dateValue }) {
  if (mode === "indefinite") return { kind: "indefinite" };

  if (mode === "duration") {
    const minutes = Number.parseInt(minutesValue, 10);
    if (!Number.isFinite(minutes)) return null;
    return { kind: "duration", minutes };
  }

  if (mode === "absoluteTime") {
    const parts = String(timeValue ?? "").split(":");
    const hour = Number.parseInt(parts[0], 10);
    const minute = Number.parseInt(parts[1], 10);
    if (!Number.isFinite(hour) || !Number.isFinite(minute)) return null;

    // The key is omitted rather than sent as null when the box is empty, so the
    // undated payload stays byte-identical to what Rust already accepted.
    const date = String(dateValue ?? "");
    return date === ""
      ? { kind: "absoluteTime", hour, minute }
      : { kind: "absoluteTime", hour, minute, date };
  }

  return null;
}

/**
 * What the UI should do with a `check_shutdown_permission` verdict.
 *
 * Pure, so the policy is testable without a window: the permission check is the
 * one thing standing between "shutdown scheduled" and a machine that quietly
 * stays on all night, and it should not be decided by branching buried in an
 * event handler.
 *
 * The rule, and the reason it is not simply `granted ? go : stop`:
 *
 * - `denied` blocks. The OS has a refusal on record, so the shutdown *will*
 *   fail; letting the job be created would promise something that cannot happen.
 * - `unknown` proceeds, with a warning. It means "nobody has asked yet" or
 *   "loginwindow could not be reached" — neither is evidence of breakage, and
 *   blocking here would refuse a machine that works.
 * - A missing or malformed verdict proceeds silently. If the check itself is
 *   broken, the user still gets the pre-existing behaviour rather than a UI that
 *   cannot schedule anything.
 *
 * `promptsForConsent` is passed through rather than folded into `warning`, because
 * "there is something to say" and "a dialog is about to appear" are different facts.
 * The Windows and Linux fallback has a reason but no dialog, and a caller that
 * conflated them would show a macOS sentence to a Windows user.
 */
export function permissionOutcome(view, t = defaultT) {
  if (!view || typeof view !== "object") {
    return { allow: true, warning: "", promptsForConsent: false };
  }

  const promptsForConsent = view.promptsForConsent === true;

  if (view.blocksScheduling) {
    return {
      allow: false,
      warning: view.reason || t("permission.deniedFallback"),
      promptsForConsent,
    };
  }

  if (view.state === "unknown" && view.reason) {
    return { allow: true, warning: view.reason, promptsForConsent };
  }

  return { allow: true, warning: "", promptsForConsent };
}

/** The sentence under a control, once Rust has resolved the instant. */
export function previewText(jobType, resolved, nowMs, t = defaultT) {
  if (!resolved || !resolved.targetInstantUtc) return "";

  const key = jobType === "powerOff" ? "preview.powerOff" : "preview.keepAwake";
  return t(key, {
    at: formatInstant(resolved.targetInstantUtc, nowMs, t),
    remaining: formatRemaining(resolved.remainingSeconds, t),
  });
}

/* ------------------------------------------------------------------ */
/* The job list                                                        */
/* ------------------------------------------------------------------ */

/*
 * Statuses that need the user to read them. A list rather than "anything not active",
 * because Paused and Finished are normal outcomes; marking them as problems would make
 * the one status that is a problem — a shutdown that did not happen — blend in with them.
 */
const ATTENTION_STATUSES = new Set(["failed", "overdue"]);

export function needsAttention(status) {
  return ATTENTION_STATUSES.has(status);
}

/**
 * The countdown line for one job.
 *
 * Past tense for anything finished, and never a live countdown for it: a cancelled job
 * still carries the instant it was set for, and counting down towards it would read as
 * though it were about to happen.
 */
export function countdownText(job, nowMs, t = defaultT) {
  if (!job.targetInstantUtc) return "";

  const at = formatInstant(job.targetInstantUtc, nowMs, t);

  // Overdue is its own sentence rather than "0 seconds left". The scheduler decided not
  // to run this, and the user needs to know it did not happen rather than that it is due.
  if (job.status === "overdue") return t("countdown.overdue", { at });

  if (job.status !== "active" && job.status !== "paused") {
    return t("countdown.past", { at });
  }

  const remaining = (Date.parse(job.targetInstantUtc) - nowMs) / 1000;

  // A paused job's target moves when it resumes, so the time of day would be wrong.
  if (job.status === "paused") {
    return t("countdown.paused", { remaining: formatRemaining(remaining, t) });
  }

  if (remaining <= 0) return t("countdown.due");

  return t("countdown.left", { remaining: formatRemaining(remaining, t), at });
}

/**
 * Which buttons a job's status allows.
 *
 * The command names are not translated — they are IPC identifiers. Only the labels are.
 */
export function actionsFor(status, t = defaultT) {
  const actions = [];
  if (status === "active") actions.push({ label: t("job.pause"), command: "pause_job" });
  if (status === "paused") actions.push({ label: t("job.resume"), command: "resume_job" });
  if (status === "active" || status === "paused") {
    actions.push({ label: t("job.cancel"), command: "cancel_job" });
  }
  return actions;
}

/** Top-of-window status derived from the same list already rendered below it. */
export function dashboardPresentation(jobs, t = defaultT) {
  const running = jobs.filter((job) => job.status === "active");
  const paused = jobs.filter((job) => job.status === "paused");
  const attention = jobs.filter((job) => needsAttention(job.status));

  if (attention.length > 0) {
    return {
      heading:
        attention.length === 1
          ? t("dashboard.attentionOne")
          : t("dashboard.attentionMany", { count: attention.length }),
      detail: t("dashboard.attentionDetail"),
      state: "attention",
    };
  }

  if (running.length > 0) {
    const heading =
      running.length === 1
        ? t("dashboard.activeOne", { label: running[0].jobTypeLabel })
        : t("dashboard.activeMany", { count: running.length });
    const pausedNote =
      paused.length > 0 ? ` ${t("dashboard.activePausedNote", { count: paused.length })}` : "";
    return {
      heading,
      detail: `${t("dashboard.activeDetail")}${pausedNote}`,
      state: "active",
    };
  }

  if (paused.length > 0) {
    return {
      heading:
        paused.length === 1
          ? t("dashboard.pausedOne")
          : t("dashboard.pausedMany", { count: paused.length }),
      detail: t("dashboard.pausedDetail"),
      state: "paused",
    };
  }

  return {
    heading: t("dashboard.idle"),
    detail: t("dashboard.idleDetail"),
    state: "idle",
  };
}

/* ------------------------------------------------------------------ */
/* Degraded capabilities                                               */
/* ------------------------------------------------------------------ */

/**
 * Turns the capability report into what to show, or an empty list when all is well.
 *
 * Essential capabilities are said to be essential in the text: "the system tray is not
 * working" is a nuisance, and "keeping the screen awake is not working" means one of the
 * two things the app exists for is gone. Those must not read alike.
 *
 * Rust sends the reason it failed ("no D-Bus screensaver interface"), which is true but
 * not actionable. The consequence is the half the user needs: whether the thing they were
 * about to schedule is going to happen.
 */
export function degradedEntries(reports, t = defaultT) {
  return reports
    .filter((report) => report.state !== "available")
    .map((report) => ({
      capability: report.capability,
      headline: report.essential
        ? t("degraded.essential", { label: report.label })
        : t("degraded.nonEssential", { label: report.label }),
      consequence: consequenceOf(report.capability, t),
      reason: report.reason ? t("degraded.reported", { reason: report.reason }) : "",
    }));
}

/**
 * What one broken capability costs the user, in their terms.
 *
 * An unknown capability yields "" rather than the raw key: a new capability added in Rust
 * before its copy lands here should show its headline with no consequence line, not the
 * string `consequence.something`.
 */
function consequenceOf(capability, t) {
  const key = `consequence.${capability}`;
  const text = t(key);
  return text === key ? "" : text;
}

export function isUnavailable(reports, capability) {
  const found = reports.find((report) => report.capability === capability);
  return Boolean(found) && found.state !== "available";
}

/* ------------------------------------------------------------------ */
/* Settings and quit                                                   */
/* ------------------------------------------------------------------ */

/**
 * Confirmation after saving. It reports how many jobs a timezone change moved, because
 * silence would leave a user wondering whether their 23:00 shutdown is still at 23:00.
 */
export function settingsSavedMessage(movedJobs, t = defaultT) {
  if (movedJobs > 0) {
    return t("settings.savedMoved", {
      count: movedJobs,
      plural: movedJobs === 1 ? "" : "s",
    });
  }
  return t("settings.saved");
}

/**
 * The quit dialog's body. The wording depends on whether anything is actually running,
 * because "quitting cancels it" is untrue when nothing is scheduled — and a warning that
 * is sometimes false is one users learn to click through.
 */
export function quitPrompt(hasActiveJob, t = defaultT) {
  return hasActiveJob ? t("quit.activeJob") : t("quit.nothingScheduled");
}

/** The label under the grace countdown's big number. */
export function graceUnitLabel(seconds, t = defaultT) {
  return seconds === 1 ? t("grace.secondLeft") : t("grace.secondsLeft");
}

/**
 * How much of the countdown bar to fill, as a percentage.
 *
 * Clamped, because the bar is drawn from the grace length the app was told at startup and
 * the seconds remaining are read live. A skew between the two must not paint outside the
 * bar or produce a negative width.
 */
export function gracePercent(secondsRemaining, graceLength) {
  if (!(graceLength > 0)) return 0;
  const fraction = secondsRemaining / graceLength;
  return Math.max(0, Math.min(1, fraction)) * 100;
}

/* ------------------------------------------------------------------ */
/* Presence and pairing                                                */
/* ------------------------------------------------------------------ */

/**
 * The three presence states, in the order the rule orders them.
 *
 * Named here so the tests can enumerate them rather than hardcoding strings, and so a
 * fourth state added in Rust fails loudly at `presenceLabel` instead of rendering a raw key.
 *
 * Three, not two. `stale` is the state a boolean cannot express: a phone the OS has
 * backgrounded is neither reachable nor gone, and reporting it as either misleads the user —
 * shown as online, a command sent to it appears ignored; shown as offline, they conclude the
 * device is broken.
 */
export const PRESENCE_STATES = ["online", "stale", "offline"];

/**
 * Prose for a presence state Rust already derived.
 *
 * # What this function must not become
 *
 * It takes a *state*, not a timestamp and not a boolean. The derivation belongs to
 * `evaluate_presence` in Rust, which the shared vectors pin and the Dart side mirrors.
 * Computing "is 90 seconds online?" here would be a second implementation of that rule in a
 * third language, and the one that drifted would be this one — it has no vectors.
 *
 * An unrecognised state yields "" rather than the raw key, so a state added in Rust before
 * its copy lands here shows a blank rather than the literal `presence.something`.
 */
export function presenceLabel(state, t = defaultT) {
  if (!PRESENCE_STATES.includes(state)) return "";
  const text = t(`presence.${state}`);
  return text === `presence.${state}` ? "" : text;
}

/**
 * A device row as the list renders it.
 *
 * `presence` comes straight from Rust's derived state and is never recomputed. `detail` is
 * how long the device has been silent, which the user needs separately: "silent for one
 * minute" and "silent for a day" are both `offline` and mean quite different things.
 *
 * A device that has never reported gets `presence.neverSeen` rather than "silent for 0
 * seconds", which would read as "just now" — the opposite of the truth.
 */
export function devicePresentation(device, t = defaultT) {
  const label = presenceLabel(device.presence, t);
  const detail =
    typeof device.elapsedSeconds === "number"
      ? t("presence.silentFor", { duration: formatRemaining(device.elapsedSeconds, t) })
      : t("presence.neverSeen");

  return {
    deviceId: device.deviceId,
    name: device.displayName || device.deviceId,
    presence: device.presence,
    presenceLabel: label,
    detail,
    // Being visible in an account is not authority to command, so the two badges are
    // separate and the pairing one is not implied by presence.
    pairedLabel: device.isPaired ? t("remote.paired") : t("remote.notPaired"),
    isPaired: Boolean(device.isPaired),
    isThisDevice: Boolean(device.isThisDevice),
  };
}

/**
 * A pairing row as the list renders it.
 *
 * Revoked pairings are presented, not filtered. A row that vanished on revocation would
 * make "was this device ever paired?" unanswerable — the question that matters most after a
 * device is lost. `revoked` is what the UI greys the row out by.
 *
 * A revoked row offers no unpair action, because there is nothing left to withdraw.
 */
export function pairingPresentation(pairing, t = defaultT) {
  return {
    peer: pairing.peer,
    revoked: Boolean(pairing.revoked),
    statusLabel: pairing.revoked ? t("remote.revoked") : t("remote.paired"),
    presence: pairing.presence,
    presenceLabel: presenceLabel(pairing.presence, t),
    detail:
      typeof pairing.elapsedSeconds === "number"
        ? t("presence.silentFor", { duration: formatRemaining(pairing.elapsedSeconds, t) })
        : t("presence.neverSeen"),
    canRevoke: !pairing.revoked,
  };
}

/**
 * The account line: who is signed in, or that nobody is.
 *
 * The device identifier is shown either way, because it exists whether or not anyone is
 * signed in and because it is what the user reads out when pairing. Signing out does not
 * change it, which is the same reason signing out cannot invalidate a pairing.
 */
export function accountPresentation(view, t = defaultT) {
  return {
    signedIn: Boolean(view?.account),
    label: view?.account
      ? t("remote.accountSignedIn", { account: view.account })
      : t("remote.accountSignedOut"),
    deviceId: view?.deviceId ?? "",
  };
}

/**
 * Seconds left on a displayed pairing code, floored at zero.
 *
 * Derived from the instant it was shown plus the lifetime Rust reported, rather than a
 * decremented counter: a backgrounded web view stops decrementing, and the user would then
 * be looking at a code the machine has already forgotten.
 */
export function codeSecondsRemaining(shownAtMs, expiresInSeconds, nowMs) {
  const elapsed = Math.floor((nowMs - shownAtMs) / 1000);
  return Math.max(0, expiresInSeconds - elapsed);
}

/* ------------------------------------------------------------------ */
/* Appearance                                                          */
/* ------------------------------------------------------------------ */

export const THEMES = ["auto", "light", "dark"];

/**
 * Which palette to paint, given the stored preference and the OS preference.
 *
 * Resolved here rather than left to a CSS media query because the stylesheet keys off a
 * `data-theme` attribute: one code path decides the palette, so an explicit choice and a
 * system-following choice cannot disagree about what is on screen.
 */
export function resolveTheme(preference, systemPrefersDark) {
  if (preference === "light" || preference === "dark") return preference;
  return systemPrefersDark ? "dark" : "light";
}

/** Sanitises a stored or IPC theme value, so a bad one cannot leave the window unstyled. */
export function normalizeTheme(value) {
  return THEMES.includes(value) ? value : "auto";
}
