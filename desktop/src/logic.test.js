/*
 * Tests for the web view's logic (node --test, no dependencies).
 *
 * These are deliberately about wording and boundaries rather than about rendering. The
 * front end's job is to state, correctly, what is about to happen to the user's machine;
 * "0 seconds left" on a shutdown that already silently did not happen is a bug that no
 * amount of Rust-side testing catches.
 *
 * Instants are always built as offsets from a fixed `now` so the assertions do not depend
 * on the machine's timezone or locale. Where a locale-formatted string is unavoidable, the
 * assertion is about its shape (does it carry a date?) rather than its exact text.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  CONSEQUENCES,
  actionsFor,
  computeClockSkewMs,
  countdownText,
  dashboardPresentation,
  degradedEntries,
  formatInstant,
  formatRemaining,
  gracePercent,
  graceUnitLabel,
  isUnavailable,
  messageOf,
  needsAttention,
  previewText,
  quitPrompt,
  readTriggerFrom,
  selectionPresentation,
  settingsSavedMessage,
} from "./logic.js";

const NOW = Date.parse("2026-07-30T12:00:00Z");
const inSeconds = (seconds) => new Date(NOW + seconds * 1000).toISOString();

/* ------------------------------------------------------------------ */
/* Clock skew                                                          */
/* ------------------------------------------------------------------ */

test("the skew is learned from a job's target and its remaining seconds", () => {
  // Rust says 100 seconds remain until an instant that the browser clock puts 130 seconds
  // away, so the browser is 30 seconds behind and every countdown must be shifted.
  const jobs = [{ targetInstantUtc: inSeconds(130), remainingSeconds: 100 }];

  assert.equal(computeClockSkewMs(jobs, NOW), 30_000);
});

test("agreeing clocks produce no correction", () => {
  const jobs = [{ targetInstantUtc: inSeconds(600), remainingSeconds: 600 }];

  assert.equal(computeClockSkewMs(jobs, NOW), 0);
});

test("an indefinite job teaches nothing about the clock", () => {
  // Null means "keep the correction you had" rather than "the offset is zero". Resetting
  // to zero because the only running job is indefinite would undo a correction that was
  // measured correctly a moment ago.
  const jobs = [{ targetInstantUtc: null, remainingSeconds: null }];

  assert.equal(computeClockSkewMs(jobs, NOW), null);
  assert.equal(computeClockSkewMs([], NOW), null);
});

test("an unparseable target is ignored rather than producing a NaN clock", () => {
  // A NaN skew would poison every countdown in the window, not just this job's.
  const jobs = [{ targetInstantUtc: "not a date", remainingSeconds: 60 }];

  assert.equal(computeClockSkewMs(jobs, NOW), null);
});

test("the skew is taken from the first job that carries both halves", () => {
  const jobs = [
    { targetInstantUtc: null, remainingSeconds: null },
    { targetInstantUtc: inSeconds(50), remainingSeconds: 45 },
  ];

  assert.equal(computeClockSkewMs(jobs, NOW), 5_000);
});

/* ------------------------------------------------------------------ */
/* Durations                                                           */
/* ------------------------------------------------------------------ */

test("a span is spelled out rather than shown as a clock", () => {
  assert.equal(formatRemaining(45), "45 seconds");
  assert.equal(formatRemaining(90), "1 minute 30 seconds");
  assert.equal(formatRemaining(3600), "1 hour 0 minutes");
  assert.equal(formatRemaining(7 * 3600 + 25 * 60), "7 hours 25 minutes");
});

test("singulars are singular", () => {
  // "1 seconds left" on the last tick before a shutdown is the most-read string in the
  // app at the moment it matters most.
  assert.equal(formatRemaining(1), "1 second");
  assert.equal(formatRemaining(61), "1 minute 1 second");
  assert.equal(formatRemaining(3661), "1 hour 1 minute");
});

test("seconds are dropped past an hour but not below it", () => {
  // Past an hour the seconds digit changes every tick and tells the user nothing; under a
  // minute it is the only thing that matters.
  assert.equal(formatRemaining(3661), "1 hour 1 minute");
  assert.equal(formatRemaining(3599), "59 minutes 59 seconds");
});

test("a span that has already run out reads as zero, never as negative", () => {
  // The tick keeps running after the target passes until the next poll replaces the row,
  // so this is reached in normal use rather than only on a broken clock.
  assert.equal(formatRemaining(-5), "0 seconds");
  assert.equal(formatRemaining(0), "0 seconds");
});

/* ------------------------------------------------------------------ */
/* Instants                                                            */
/* ------------------------------------------------------------------ */

test("a time later today is shown without a date", () => {
  const text = formatInstant(inSeconds(3600), NOW);

  assert.ok(!text.includes(" on "), `unexpected date in ${text}`);
});

test("a time on another day carries its date", () => {
  // At 23:50, "shuts down at 00:30" is ambiguous in a way that matters: the user cannot
  // tell whether it is in forty minutes or in nearly a day.
  const text = formatInstant(inSeconds(36 * 3600), NOW);

  assert.ok(text.includes(" on "), `expected a date in ${text}`);
});

test("an unparseable instant is passed through rather than shown as Invalid Date", () => {
  assert.equal(formatInstant("nonsense", NOW), "nonsense");
});

/* ------------------------------------------------------------------ */
/* Errors                                                              */
/* ------------------------------------------------------------------ */

test("a rejected command's own words reach the user", () => {
  // Rust rejects with `AppError::user_message()`, already written for a person. Replacing
  // it with a generic apology would drop the only actionable part.
  assert.equal(messageOf("Shutdown is not available on this platform."), "Shutdown is not available on this platform.");
  assert.equal(messageOf(new Error("no tray")), "no tray");
  assert.equal(messageOf({ message: "boom" }), "boom");
});

test("something with no message at all still produces text", () => {
  assert.equal(typeof messageOf(undefined), "string");
  assert.notEqual(messageOf(undefined), "");
});

/* ------------------------------------------------------------------ */
/* Reading the form                                                    */
/* ------------------------------------------------------------------ */

test("each mode produces the trigger shape Rust deserialises", () => {
  assert.deepEqual(readTriggerFrom({ mode: "indefinite" }), { kind: "indefinite" });
  assert.deepEqual(readTriggerFrom({ mode: "duration", minutesValue: "120" }), {
    kind: "duration",
    minutes: 120,
  });
  assert.deepEqual(readTriggerFrom({ mode: "absoluteTime", timeValue: "23:05" }), {
    kind: "absoluteTime",
    hour: 23,
    minute: 5,
  });
});

test("midnight is read as zero rather than as an empty field", () => {
  // "00:00" parses to hour 0, which is falsy. A truthiness check here would silently
  // refuse to schedule anything at midnight.
  assert.deepEqual(readTriggerFrom({ mode: "absoluteTime", timeValue: "00:00" }), {
    kind: "absoluteTime",
    hour: 0,
    minute: 0,
  });
});

test("a half-typed field yields nothing rather than a guess", () => {
  assert.equal(readTriggerFrom({ mode: "duration", minutesValue: "" }), null);
  assert.equal(readTriggerFrom({ mode: "absoluteTime", timeValue: "" }), null);
  assert.equal(readTriggerFrom({ mode: "absoluteTime", timeValue: "18" }), null);
});

test("the bounds are not re-implemented here", () => {
  // Out-of-range values are passed to Rust and rejected there, so `MAX_DURATION_MINUTES`
  // and the hour range exist in exactly one language. A check here would be a second copy
  // that drifts, and the drift would show up as a form that refuses input Rust accepts.
  assert.deepEqual(readTriggerFrom({ mode: "duration", minutesValue: "99999" }), {
    kind: "duration",
    minutes: 99999,
  });
  assert.deepEqual(readTriggerFrom({ mode: "absoluteTime", timeValue: "25:70" }), {
    kind: "absoluteTime",
    hour: 25,
    minute: 70,
  });
});

test("an unknown mode produces nothing", () => {
  assert.equal(readTriggerFrom({ mode: "surprise" }), null);
});

/* ------------------------------------------------------------------ */
/* The preview                                                         */
/* ------------------------------------------------------------------ */

test("the shutdown preview says it shuts down, and when", () => {
  const text = previewText(
    "powerOff",
    { targetInstantUtc: inSeconds(5400), remainingSeconds: 5400 },
    NOW,
  );

  assert.match(text, /Shuts down at/);
  assert.match(text, /1 hour 30 minutes from now/);
});

test("the keep-awake preview does not claim to shut anything down", () => {
  const text = previewText(
    "keepAwake",
    { targetInstantUtc: inSeconds(600), remainingSeconds: 600 },
    NOW,
  );

  assert.match(text, /Stays awake until/);
  assert.ok(!/[Ss]hut/.test(text), `keep-awake must not mention shutting down: ${text}`);
});

test("an indefinite resolution previews nothing", () => {
  assert.equal(previewText("keepAwake", { targetInstantUtc: null }, NOW), "");
  assert.equal(previewText("keepAwake", null, NOW), "");
});

test("the composer names exactly the features selected", () => {
  assert.deepEqual(selectionPresentation(true, false), {
    summary: "Keep the display awake",
    submit: "Start keeping awake",
  });
  assert.deepEqual(selectionPresentation(false, true), {
    summary: "Schedule a safe power-off",
    submit: "Schedule power-off",
  });
  assert.deepEqual(selectionPresentation(true, true), {
    summary: "Keep awake + power off",
    submit: "Start both schedules",
  });
  assert.deepEqual(selectionPresentation(false, false), {
    summary: "Choose one or both",
    submit: "Choose an action to start",
  });
});

/* ------------------------------------------------------------------ */
/* The job list                                                        */
/* ------------------------------------------------------------------ */

test("only a failure or a missed shutdown is flagged for attention", () => {
  assert.ok(needsAttention("failed"));
  assert.ok(needsAttention("overdue"));

  // Pausing and finishing are things the user did or expected. Flagging them would make
  // the one status that is a real problem blend in with the routine ones.
  for (const status of ["active", "paused", "completed", "cancelled", "degraded"]) {
    assert.ok(!needsAttention(status), status);
  }
});

test("a running job counts down towards its own instant", () => {
  const job = { status: "active", targetInstantUtc: inSeconds(125) };

  assert.equal(countdownText(job, NOW), `2 minutes 5 seconds left — ${formatInstant(job.targetInstantUtc, NOW)}.`);
});

test("a missed shutdown says it did not run", () => {
  // The most important line in this file. The scheduler deliberately refuses a shutdown
  // more than fifteen minutes late, and the user has to learn that from the window: the
  // machine is still on, and nothing is going to turn it off.
  const job = { status: "overdue", targetInstantUtc: inSeconds(-3600) };
  const text = countdownText(job, NOW);

  assert.match(text, /did not run/);
  assert.ok(!/left/.test(text), `an overdue job must not count down: ${text}`);
});

test("a finished or cancelled job is in the past tense and does not count down", () => {
  for (const status of ["completed", "cancelled", "failed"]) {
    const text = countdownText({ status, targetInstantUtc: inSeconds(90) }, NOW);

    // The instant is still in the future here — a job cancelled a minute before it was
    // due. Counting towards it would read as though it were about to happen.
    assert.match(text, /^Was set for /, `${status}: ${text}`);
    assert.ok(!/left/.test(text), `${status} must not count down: ${text}`);
  }
});

test("a paused job shows what is left but not a time of day", () => {
  // The remaining span survives a pause; the wall-clock time does not, because it moves
  // when the job resumes. Showing "at 14:02" for a paused job would be a promise broken
  // by the act of resuming it.
  const text = countdownText({ status: "paused", targetInstantUtc: inSeconds(600) }, NOW);

  assert.equal(text, "Paused with 10 minutes 0 seconds left.");
});

test("a job whose instant has just passed reads as due rather than as negative", () => {
  const text = countdownText({ status: "active", targetInstantUtc: inSeconds(-2) }, NOW);

  assert.equal(text, "Due now.");
});

test("an indefinite job has no countdown line at all", () => {
  assert.equal(countdownText({ status: "active", targetInstantUtc: null }, NOW), "");
});

test("the buttons offered match what the status allows", () => {
  const labels = (status) => actionsFor(status).map((action) => action.label);

  assert.deepEqual(labels("active"), ["Pause", "Cancel"]);
  assert.deepEqual(labels("paused"), ["Resume", "Cancel"]);

  // Nothing to pause or cancel once it is over: a Cancel button on a finished job would
  // invoke a command Rust rejects, turning a stale row into an error message.
  for (const status of ["completed", "cancelled", "failed", "overdue"]) {
    assert.deepEqual(labels(status), [], status);
  }
});

test("pause and resume are never offered together", () => {
  // One control for one state. Offering both would leave the user guessing which of the
  // two describes the job in front of them.
  for (const status of ["active", "paused"]) {
    const commands = actionsFor(status).map((action) => action.command);
    assert.ok(
      !(commands.includes("pause_job") && commands.includes("resume_job")),
      status,
    );
  }
});

test("the dashboard distinguishes idle, active, and paused schedules", () => {
  assert.deepEqual(dashboardPresentation([]), {
    heading: "No active schedules",
    detail: "Choose an action below. Weakup keeps working when this window is closed.",
    state: "idle",
  });

  const active = dashboardPresentation([
    { status: "active", jobTypeLabel: "Keep screen awake" },
  ]);
  assert.equal(active.heading, "Keep screen awake is active");
  assert.equal(active.state, "active");

  const paused = dashboardPresentation([{ status: "paused" }, { status: "paused" }]);
  assert.equal(paused.heading, "2 schedules are paused");
  assert.equal(paused.state, "paused");
});

test("a schedule needing attention takes priority in the dashboard", () => {
  const presentation = dashboardPresentation([
    { status: "active", jobTypeLabel: "Keep screen awake" },
    { status: "overdue", jobTypeLabel: "Shut down" },
  ]);

  assert.equal(presentation.heading, "1 schedule needs attention");
  assert.equal(presentation.state, "attention");
  assert.match(presentation.detail, /No missed power-off runs without warning/);
});

test("the dashboard reports active and paused schedules together", () => {
  const presentation = dashboardPresentation([
    { status: "active", jobTypeLabel: "Keep screen awake" },
    { status: "active", jobTypeLabel: "Shut down" },
    { status: "paused", jobTypeLabel: "Keep screen awake" },
  ]);

  assert.equal(presentation.heading, "2 schedules are active");
  assert.match(presentation.detail, /1 more paused/);
});

/* ------------------------------------------------------------------ */
/* Degraded capabilities                                               */
/* ------------------------------------------------------------------ */

const AVAILABLE = { capability: "tray", label: "system tray", essential: false, state: "available" };

test("a healthy report produces no warnings", () => {
  assert.deepEqual(degradedEntries([AVAILABLE]), []);
});

test("every capability has a consequence written for the user", () => {
  // A warning with no consequence tells the user a component name and leaves them to
  // work out whether their schedule still happens.
  for (const capability of ["keep_awake", "power_off", "tray", "autostart", "notifications"]) {
    const entry = degradedEntries([
      { capability, label: capability, essential: false, state: "unavailable", reason: "why" },
    ])[0];

    assert.ok(entry.consequence.length > 0, capability);
  }
});

test("the consequences are prose, not identifiers", () => {
  for (const [capability, text] of Object.entries(CONSEQUENCES)) {
    assert.ok(/[a-z] [a-z]/.test(text), `${capability}: ${text}`);
    assert.ok(text.trim().endsWith("."), `${capability}: ${text}`);
  }
});

test("a broken essential capability says so, and a cosmetic one does not", () => {
  const [essential] = degradedEntries([
    {
      capability: "power_off",
      label: "scheduled power-off",
      essential: true,
      state: "unavailable",
      reason: "this platform cannot be shut down by an app",
    },
  ]);
  const [cosmetic] = degradedEntries([{ ...AVAILABLE, state: "unavailable", reason: "no tray" }]);

  // Losing the tray is a nuisance; losing power-off means one of the two things the app
  // exists for is gone. Those must not read alike.
  assert.match(essential.headline, /one of the two things/);
  assert.ok(!/one of the two things/.test(cosmetic.headline));
});

test("the reason from Rust is shown alongside the consequence, not instead of it", () => {
  const [entry] = degradedEntries([
    {
      capability: "keep_awake",
      label: "keep screen awake",
      essential: true,
      state: "unavailable",
      reason: "no D-Bus screensaver interface",
    },
  ]);

  assert.match(entry.reason, /no D-Bus screensaver interface/);
  assert.match(entry.consequence, /will not hold the screen on/);
});

test("the notification warning admits the shutdown warning is included", () => {
  // The user's own choice suppresses the shutdown notification too, so the app has to say
  // that plainly rather than let them find out during a countdown they never saw.
  assert.match(CONSEQUENCES.notifications, /shutdown/);
});

test("a capability is only called unavailable when the report says so", () => {
  const reports = [AVAILABLE];

  assert.ok(!isUnavailable(reports, "tray"));
  assert.ok(isUnavailable([{ ...AVAILABLE, state: "unavailable" }], "tray"));

  // Absent from the report is not the same as broken: claiming a missing entry is broken
  // would disable the hide button on a perfectly good tray.
  assert.ok(!isUnavailable(reports, "autostart"));
});

/* ------------------------------------------------------------------ */
/* Settings and quit                                                   */
/* ------------------------------------------------------------------ */

test("a timezone change reports how many jobs moved", () => {
  assert.equal(settingsSavedMessage(0), "Saved.");
  assert.match(settingsSavedMessage(1), /1 job set for a time of day moved/);
  assert.match(settingsSavedMessage(3), /3 jobs set for a time of day moved/);
});

test("the quit warning only claims a job is running when one is", () => {
  // A warning that is sometimes false is one users learn to click through, and the thing
  // being clicked through here is the loss of a scheduled shutdown.
  assert.match(quitPrompt(true), /still running/);
  assert.ok(!/still running/.test(quitPrompt(false)));
});

/* ------------------------------------------------------------------ */
/* The grace countdown                                                 */
/* ------------------------------------------------------------------ */

test("the last second is singular", () => {
  assert.equal(graceUnitLabel(1), "second left");
  assert.equal(graceUnitLabel(2), "seconds left");
  assert.equal(graceUnitLabel(0), "seconds left");
});

test("the bar empties as the countdown runs", () => {
  assert.equal(gracePercent(60, 60), 100);
  assert.equal(gracePercent(30, 60), 50);
  assert.equal(gracePercent(0, 60), 0);
});

test("the bar stays inside itself whatever the two clocks disagree about", () => {
  // The length comes from startup and the seconds are read live, so they can disagree.
  // A negative width silently stops rendering; over 100% paints past the container.
  assert.equal(gracePercent(-5, 60), 0);
  assert.equal(gracePercent(90, 60), 100);
  assert.equal(gracePercent(10, 0), 0);
});
