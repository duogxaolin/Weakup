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
 */

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

const unit = (value, word) => `${value} ${word}${value === 1 ? "" : "s"}`;

/**
 * Plain English for a span of seconds: "1 hour 5 minutes", not "01:05:00".
 *
 * Coarser as the span grows. Seconds are dropped past an hour because a user reading
 * "2 hours 14 minutes 3 seconds" does not care about the 3, and the digit changing every
 * second draws the eye to the least important part of the line.
 */
export function formatRemaining(totalSeconds) {
  const seconds = Math.max(0, Math.round(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;

  if (hours > 0) return `${unit(hours, "hour")} ${unit(minutes, "minute")}`;
  if (minutes > 0) return `${unit(minutes, "minute")} ${unit(rest, "second")}`;
  return unit(rest, "second");
}

/**
 * The target as a wall-clock time, with the date added only when it is not today.
 *
 * "Shuts down at 00:30" is ambiguous at 23:50 in a way "at 00:30 on 31/07" is not, and
 * that ambiguity is in front of an irreversible action.
 */
export function formatInstant(rfc3339, nowMs) {
  const date = new Date(rfc3339);
  if (Number.isNaN(date.getTime())) return String(rfc3339);

  const time = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const isToday = date.toDateString() === new Date(nowMs).toDateString();
  return isToday ? time : `${time} on ${date.toLocaleDateString()}`;
}

/**
 * Every command rejects with `AppError::user_message()` — prose written for a person. So
 * the message is surfaced as-is rather than replaced with "Something went wrong", which
 * throws away the only part that tells the user what to do about it.
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
 * resolver, so the bounds live in one language (task 11.2).
 */
export function readTriggerFrom({ mode, minutesValue, timeValue }) {
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
    return { kind: "absoluteTime", hour, minute };
  }

  return null;
}

/** The sentence under a control, once Rust has resolved the instant. */
export function previewText(jobType, resolved, nowMs) {
  if (!resolved || !resolved.targetInstantUtc) return "";

  const verb = jobType === "powerOff" ? "Shuts down at" : "Stays awake until";
  return `${verb} ${formatInstant(resolved.targetInstantUtc, nowMs)} (${formatRemaining(
    resolved.remainingSeconds,
  )} from now).`;
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
export function countdownText(job, nowMs) {
  if (!job.targetInstantUtc) return "";

  const at = formatInstant(job.targetInstantUtc, nowMs);

  // Overdue is its own sentence rather than "0 seconds left". The scheduler decided not
  // to run this, and the user needs to know it did not happen rather than that it is due.
  if (job.status === "overdue") return `Was due at ${at}. It did not run.`;

  if (job.status !== "active" && job.status !== "paused") return `Was set for ${at}.`;

  const remaining = (Date.parse(job.targetInstantUtc) - nowMs) / 1000;

  // A paused job's target moves when it resumes, so the time of day would be wrong.
  if (job.status === "paused") return `Paused with ${formatRemaining(remaining)} left.`;

  if (remaining <= 0) return "Due now.";

  return `${formatRemaining(remaining)} left — ${at}.`;
}

/** Which buttons a job's status allows. */
export function actionsFor(status) {
  const actions = [];
  if (status === "active") actions.push({ label: "Pause", command: "pause_job" });
  if (status === "paused") actions.push({ label: "Resume", command: "resume_job" });
  if (status === "active" || status === "paused") {
    actions.push({ label: "Cancel", command: "cancel_job" });
  }
  return actions;
}

/* ------------------------------------------------------------------ */
/* Degraded capabilities                                               */
/* ------------------------------------------------------------------ */

/*
 * What each broken capability costs the user, in their terms.
 *
 * Rust sends the reason it failed ("no D-Bus screensaver interface"), which is true but
 * not actionable. The consequence is the half the user needs: whether the thing they were
 * about to schedule is going to happen.
 */
export const CONSEQUENCES = {
  keep_awake:
    "Screen-awake jobs will not hold the screen on. Nothing in Weakup can change that on this computer.",
  power_off:
    "A scheduled shutdown will not run. The job will be recorded as failed instead, and this computer will stay on.",
  tray: "There is no icon to reach Weakup with, so closing this window will ask before quitting rather than hiding.",
  autostart:
    "Weakup will not start when you log in, so a job set for tomorrow will not run unless Weakup is already open.",
  notifications:
    "No notifications, including the warning before a shutdown. The countdown will appear in this window only.",
};

/**
 * Turns the capability report into what to show, or an empty list when all is well.
 *
 * Essential capabilities are said to be essential in the text: "the system tray is not
 * working" is a nuisance, and "keeping the screen awake is not working" means one of the
 * two things the app exists for is gone. Those must not read alike.
 */
export function degradedEntries(reports) {
  return reports
    .filter((report) => report.state !== "available")
    .map((report) => ({
      capability: report.capability,
      headline: report.essential
        ? `${report.label} is not working on this computer — this is one of the two things Weakup does.`
        : `${report.label} is not working on this computer.`,
      consequence: CONSEQUENCES[report.capability] ?? "",
      reason: report.reason ? `Reported: ${report.reason}` : "",
    }));
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
export function settingsSavedMessage(movedJobs) {
  if (movedJobs > 0) {
    return `Saved. ${movedJobs} job${movedJobs === 1 ? "" : "s"} set for a time of day moved to the new timezone.`;
  }
  return "Saved.";
}

/**
 * The quit dialog's body. The wording depends on whether anything is actually running,
 * because "quitting cancels it" is untrue when nothing is scheduled — and a warning that
 * is sometimes false is one users learn to click through.
 */
export function quitPrompt(hasActiveJob) {
  return hasActiveJob
    ? "A job is still running. Quitting cancels it, and this computer will not shut down on its own."
    : "Nothing is scheduled right now.";
}

/** The label under the grace countdown's big number. */
export function graceUnitLabel(seconds) {
  return seconds === 1 ? "second left" : "seconds left";
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
