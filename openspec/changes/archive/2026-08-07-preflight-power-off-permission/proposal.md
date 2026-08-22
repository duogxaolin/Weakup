# Ask for shutdown permission when the schedule is made, not when it fires

## Why

A user asked whether scheduling a power-off needs a password, reasoning that
`sudo shutdown -h 06:00` does — and that if permission is needed, it must be requested
when the command is set up rather than when it runs.

The app does not use `sudo`; it goes through `osascript` on macOS, `shutdown /s` on
Windows, and `systemctl poweroff` on Linux, none of which need root. So there is no
password. But the underlying concern was correct, and on macOS it exposed a real defect.

macOS raises its Automation (TCC) consent dialog on the **first Apple Event the app
sends**, and the first one this app sends is the shutdown itself. The failure that
follows:

1. The user schedules a power-off for 06:00 and goes to bed.
2. At 06:00 the grace period elapses and `osascript` runs.
3. macOS raises the consent dialog. Nobody is awake to answer it.
4. `osascript` returns `-1743`, the job is marked `Failed`, and the machine is still
   running in the morning.

A scheduling app must not produce that outcome silently. A second, related defect made it
worse: `PowerOffExecutor::is_supported()` is hard-coded `true` on macOS — correctly, since
it reports whether the *platform* can shut down, not whether this app is permitted to — and
it was the only thing consulted at startup. So `CapabilityRegistry` reported "scheduled
power-off: available" even on a machine whose consent had already been refused, and the
user got no warning until the shutdown failed.

## What changes

A preflight that asks the OS for shutdown permission at scheduling time, and reports it.

- **`AEDeterminePermissionToAutomateTarget`** on macOS. This is the API written for the
  purpose: it reports — and optionally prompts for — the consent that *would* apply to an
  event, **without sending one**. That distinction is why it is used instead of a probe
  event: any probe aimed at System Events risks doing something, and the only thing worth
  doing to System Events in this app is shutting the machine down. The preflight therefore
  holds no executor and cannot power anything off.

- **A three-state verdict**, not a boolean: `Granted`, `Denied { reason }`,
  `Unknown { reason }`. Collapsing "refused" and "could not tell" would force a choice
  between blocking machines that work and staying silent about machines that do not. Only
  `Denied` blocks scheduling.

- **Two moments, two behaviours.** Ticking Power Off asks *without* prompting and reveals a
  sentence saying macOS is about to ask; submitting asks for real and may raise the dialog.
  That order satisfies the existing requirement that the consent dialog be explained before
  it appears, and it is structural rather than a matter of timing — the checkbox handler
  cannot raise a dialog at all.

- **Startup probes silently** and degrades `Capability::PowerOff` on an outright denial, so
  the dashboard banner stops claiming a power-off will work when it will not. An undecided
  verdict deliberately does *not* degrade: nobody has refused anything, and a warning there
  would train the user to ignore the banner that matters.

Windows and Linux need no dialog — `SeShutdownPrivilege` and the polkit rule are either
held or not, and asking earlier surfaces nothing — so their preflight is a no-op that
returns `Unknown`, and the authoritative answer still comes from `classify` at execution
time.

## Impact

- Affected specs: `device-power-off` (the preflight requirement and the capability
  reporting rule), `platform-capability-probe` (power-off availability must account for
  permission, not only platform support).
- No new dependency: `AEDeterminePermissionToAutomateTarget` comes from
  `ApplicationServices`, linked the same way the existing IOKit keep-awake FFI is.
- The grace period, the executor, the command surface's shutdown boundary, and
  `capabilities/default.json` are untouched. The new command asks about permission and
  cannot act on it.
- One FFI hazard worth recording: `AEDesc` is declared under `#pragma pack(2)`, so it is 12
  bytes with `dataHandle` at offset 4. A plain `#[repr(C)]` gives 16 and offset 8, which
  reads correctly immediately after `AECreateDesc` and then corrupts the handle on the
  first *move* — presenting as a segfault inside a system framework. Pinned by a layout
  test.
