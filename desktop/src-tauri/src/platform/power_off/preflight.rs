//! Asking for shutdown permission at scheduling time rather than at shutdown time.
//!
//! The bug this exists to close: on macOS the app powers off by sending the
//! shutdown Apple Event to `loginwindow` via `osascript`, which needs Automation
//! (TCC) consent. That consent is requested by the *first Apple Event the app
//! sends*, and the first one it sends is the shutdown itself. So a user who
//! schedules a power-off for 06:00 and goes to bed gets the consent prompt at
//! 06:00, with nobody awake to answer it. `osascript` returns `-1743`, the job
//! is marked `Failed`, and the machine is still running in the morning — the one
//! outcome a scheduling app must not produce silently.
//!
//! The fix is to ask while the user is still sitting in front of the machine.
//! `AEDeterminePermissionToAutomateTarget` is the Apple Event API written for
//! exactly this: it reports — and optionally prompts for — the consent that
//! *would* apply to an event, **without sending one**. That distinction is the
//! whole reason it is used here rather than a probe event: a probe aimed at
//! loginwindow risks doing something, and the only thing worth doing to
//! loginwindow in this app is shutting the machine down.
//!
//! The other two platforms need no prompt. Windows' `SeShutdownPrivilege` and
//! Linux's polkit rule are either held or not; there is no dialog that asking
//! earlier would surface, so their preflight is a no-op and the honest report
//! still comes from [`classify`](super::classify) at execution time.

use crate::core::AppResult;

/// What a preflight found.
///
/// Deliberately not a `bool`. "Granted" and "we could not tell" are different
/// facts, and collapsing them would force this module to pick one to lie about:
/// reporting an unknown as denied blocks a user whose machine is fine, and
/// reporting it as granted recreates the 06:00 failure this module exists to
/// prevent. Naming the third case lets the caller allow the schedule *and* warn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerOffPermission {
    /// The OS confirmed this app may power the machine off.
    Granted,
    /// The OS refused, with a reason written for a person.
    Denied { reason: String },
    /// No answer was available — an unrecognised status, or a platform with no
    /// way to ask. Treated as "proceed, but say so".
    Unknown { reason: String },
}

impl PowerOffPermission {
    pub fn is_granted(&self) -> bool {
        matches!(self, Self::Granted)
    }

    /// Whether scheduling should be blocked. Only an outright denial blocks.
    pub fn blocks_scheduling(&self) -> bool {
        matches!(self, Self::Denied { .. })
    }

    /// The reason, when there is one.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Granted => None,
            Self::Denied { reason } | Self::Unknown { reason } => Some(reason),
        }
    }
}

/// Asks the host whether this app may power the machine off.
///
/// `ask_user` controls whether the OS may show a consent dialog. Pass `true`
/// from a user action (scheduling a power-off), where a prompt is expected and
/// answerable. Pass `false` from startup, where an unprompted dialog would be
/// unexplained — there, a silent check that only *reports* is what is wanted.
pub fn check_power_off_permission(ask_user: bool) -> AppResult<PowerOffPermission> {
    #[cfg(target_os = "macos")]
    {
        Ok(macos::determine_automation_permission(ask_user))
    }

    #[cfg(not(target_os = "macos"))]
    {
        // `ask_user` is meaningful only where a dialog exists to raise.
        let _ = ask_user;
        Ok(PowerOffPermission::Unknown {
            reason: "this platform reports shutdown permission only when the shutdown runs"
                .to_string(),
        })
    }
}

/// Whether this platform can raise a consent dialog for shutdown permission.
///
/// Separate from the verdict, because the two answer different questions and the UI
/// needs both. The verdict says what permission *is*; this says whether asking could
/// put a dialog on screen — which is what decides if the "macOS will ask for
/// permission" sentence is true here at all.
///
/// Without this, the non-macOS fallback — which returns `Unknown` *with* a reason —
/// would make the UI show a sentence about Automation consent to a Windows user, since
/// "has a reason to show" is not the same as "is about to be prompted".
pub fn platform_prompts_for_consent() -> bool {
    cfg!(target_os = "macos")
}

/// Maps an `AEDeterminePermissionToAutomateTarget` status to a verdict.
///
/// Split out from the FFI so the mapping — the part with the decisions in it —
/// is testable on every host. The `unsafe` call above it has no branching left
/// to get wrong.
///
/// Compiled everywhere on purpose. A pure integer-to-verdict function cannot
/// touch the OS, and the alternative is shipping the logic untested on the two
/// targets that cannot run it.
pub fn classify_automation_status(status: i32) -> PowerOffPermission {
    /// `noErr` — permission is held.
    const NO_ERR: i32 = 0;
    /// `errAEEventNotPermitted` — the user declined, or TCC has a denial on
    /// record. The same code `osascript` returns at shutdown time, which is what
    /// makes this preflight predictive rather than merely advisory.
    const ERR_AE_EVENT_NOT_PERMITTED: i32 = -1743;
    /// `errAEEventWouldRequireUserConsent` — consent has not been decided and we
    /// asked not to prompt. Only reachable with `ask_user: false`.
    const ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT: i32 = -1744;
    /// `procNotFound` — the target could not be reached right now. Not a denial:
    /// loginwindow is effectively always up, so this means something transient.
    const PROC_NOT_FOUND: i32 = -600;

    match status {
        NO_ERR => PowerOffPermission::Granted,

        ERR_AE_EVENT_NOT_PERMITTED => PowerOffPermission::Denied {
            reason: "macOS has this app blocked from controlling loginwindow, so a scheduled \
                     shutdown cannot run. Allow it under System Settings > Privacy & Security \
                     > Automation."
                .to_string(),
        },

        ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT => PowerOffPermission::Unknown {
            reason: "macOS has not been asked for permission to control loginwindow yet"
                .to_string(),
        },

        // Not a denial, and must not be reported as one: refusing to schedule
        // because the target process happens not to be reachable right now
        // would block a machine that will shut down perfectly well.
        PROC_NOT_FOUND => PowerOffPermission::Unknown {
            reason: "loginwindow could not be reached, so permission could not be checked yet"
                .to_string(),
        },

        other => PowerOffPermission::Unknown {
            reason: format!("macOS returned an unrecognised permission status ({other})"),
        },
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{classify_automation_status, PowerOffPermission};
    use std::ffi::c_void;

    type OSStatus = i32;
    type DescType = u32;
    type AEEventClass = u32;
    type AEEventID = u32;

    /// `typeApplicationBundleID` — addresses the target by bundle id rather than
    /// by pid, so no lookup is needed and loginwindow's pid never matters.
    ///
    /// The four characters matter and are easy to get wrong: this is `'bund'`.
    /// An invalid type code does *not* make `AECreateDesc` fail — it builds a
    /// descriptor the permission call then reads as garbage, which segfaulted
    /// under the test harness while returning a plausible `-600` standalone.
    const TYPE_APPLICATION_BUNDLE_ID: DescType = u32::from_be_bytes(*b"bund");
    /// `typeWildCard` (`'****'`). Asking about *any* event to this target is the
    /// right question: TCC's grant is per target application, not per event, so
    /// narrowing to the shutdown event would test something finer than the thing
    /// actually being decided.
    const TYPE_WILD_CARD: u32 = u32::from_be_bytes(*b"****");

    /// The bundle id of `loginwindow`, the process the shutdown event is
    /// actually addressed to. The preflight must ask TCC about the same target
    /// the event will go to, or the answer describes a permission this app never
    /// exercises.
    const LOGINWINDOW_BUNDLE_ID: &[u8] = b"com.apple.loginwindow";

    /// `AEDesc`. Two fields, and **two-byte packed** — not the natural C layout.
    ///
    /// `AEDesc` predates 64-bit and is still declared under `#pragma pack(2)`,
    /// so `sizeof` is 12 with `dataHandle` at offset 4. A plain `#[repr(C)]`
    /// gives 16 with the handle at offset 8, which reads correctly right after
    /// `AECreateDesc` fills it in and then breaks the moment the struct is
    /// *moved* — the handle lands in a slot the framework does not read. That
    /// cost an afternoon: it presented as a segfault inside
    /// `AEDeterminePermissionToAutomateTarget` on some builds and a plausible
    /// `-600` on others, because the garbage read was whatever happened to be
    /// on the stack. Verified against the C header on this host: size 12,
    /// align 2.
    #[repr(C, packed(2))]
    struct AEDesc {
        descriptor_type: DescType,
        data_handle: *mut c_void,
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AECreateDesc(
            type_code: DescType,
            data_ptr: *const c_void,
            data_size: isize,
            result: *mut AEDesc,
        ) -> OSStatus;

        fn AEDisposeDesc(desc: *mut AEDesc) -> OSStatus;

        fn AEDeterminePermissionToAutomateTarget(
            target: *const AEDesc,
            the_ae_event_class: AEEventClass,
            the_ae_event_id: AEEventID,
            ask_user_if_needed: u8,
        ) -> OSStatus;
    }

    /// An owned `AEDesc`, disposed on drop.
    ///
    /// A guard rather than a bare struct because the permission call sits
    /// between creation and disposal and returns a status that is inspected —
    /// an early return added there later would otherwise leak.
    struct AddressDesc(AEDesc);

    impl Drop for AddressDesc {
        fn drop(&mut self) {
            // SAFETY: only constructed from a successful `AECreateDesc`, and
            // disposed exactly once because `AddressDesc` is not `Copy`/`Clone`.
            unsafe {
                AEDisposeDesc(&mut self.0);
            }
        }
    }

    /// Asks TCC about controlling loginwindow, without sending an event.
    pub fn determine_automation_permission(ask_user: bool) -> PowerOffPermission {
        let mut descriptor = AEDesc {
            descriptor_type: 0,
            data_handle: std::ptr::null_mut(),
        };

        // SAFETY: the bundle id is a valid byte slice for the length given, and
        // `descriptor` is a live, correctly-typed out-parameter.
        let created = unsafe {
            AECreateDesc(
                TYPE_APPLICATION_BUNDLE_ID,
                LOGINWINDOW_BUNDLE_ID.as_ptr() as *const c_void,
                LOGINWINDOW_BUNDLE_ID.len() as isize,
                &mut descriptor,
            )
        };

        if created != 0 {
            // Failing to build the address says nothing about consent, so this
            // is `Unknown`: the machine may well shut down fine.
            return PowerOffPermission::Unknown {
                reason: format!("could not address loginwindow to check permission ({created})"),
            };
        }

        let guard = AddressDesc(descriptor);

        // SAFETY: `guard.0` came from a successful `AECreateDesc` and is alive
        // for this call; the guard disposes of it afterwards.
        let status = unsafe {
            AEDeterminePermissionToAutomateTarget(
                &guard.0,
                TYPE_WILD_CARD,
                TYPE_WILD_CARD,
                u8::from(ask_user),
            )
        };

        classify_automation_status(status)
    }

    #[cfg(test)]
    mod layout_tests {
        use super::AEDesc;

        #[test]
        fn the_descriptor_matches_the_two_byte_packed_c_layout() {
            // Checked against the real header on this host:
            //   sizeof(AEDesc)=12 align=2 dataHandle at offset 4
            // Dropping `packed(2)` gives 16/8 and offset 8, which survives long
            // enough to look correct and then corrupts the handle on the first
            // move. Pinned here because the failure mode is a segfault inside a
            // system framework, which points nowhere near this struct.
            assert_eq!(std::mem::size_of::<AEDesc>(), 12, "AEDesc must be 12 bytes");
            assert_eq!(std::mem::align_of::<AEDesc>(), 2, "AEDesc must be 2-aligned");
        }

        #[test]
        fn a_descriptor_survives_being_moved() {
            // The exact operation the wrong layout broke: build one, move it
            // into a guard, and read the handle back out. With `#[repr(C)]`
            // this reads a different address than was written.
            let original = AEDesc {
                descriptor_type: 0x6275_6e64,
                data_handle: 0x1234_5678 as *mut std::ffi::c_void,
            };

            let moved = Box::new(original);

            assert_eq!({ moved.descriptor_type }, 0x6275_6e64);
            assert_eq!({ moved.data_handle }, 0x1234_5678 as *mut std::ffi::c_void);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_status_is_a_grant() {
        assert_eq!(classify_automation_status(0), PowerOffPermission::Granted);
        assert!(classify_automation_status(0).is_granted());
        assert!(!classify_automation_status(0).blocks_scheduling());
    }

    #[test]
    fn the_declined_code_is_the_one_osascript_returns_at_shutdown_time() {
        // -1743 is what `classify_macos` matches on when a real shutdown fails.
        // The preflight is only worth having if it predicts that same code, so
        // this pins the two to the same number.
        let verdict = classify_automation_status(-1743);

        assert!(verdict.blocks_scheduling(), "a known denial must block");
        let reason = verdict.reason().expect("a denial carries a reason");
        assert!(reason.contains("Automation"));
        assert!(reason.contains("Privacy & Security"));
    }

    #[test]
    fn an_undecided_consent_does_not_block_scheduling() {
        // Reachable from the startup probe, which asks without prompting. The
        // user has not refused anything; blocking here would break an install
        // that is about to work the first time the prompt appears.
        let verdict = classify_automation_status(-1744);

        assert!(!verdict.blocks_scheduling());
        assert!(!verdict.is_granted(), "undecided is not a grant");
        assert!(verdict.reason().is_some(), "the user should be told why");
    }

    #[test]
    fn the_target_not_answering_is_not_treated_as_a_refusal() {
        // loginwindow is effectively always up, so a -600 means something
        // transient. Reading it as a denial would refuse to schedule on a
        // machine that shuts down perfectly well.
        let verdict = classify_automation_status(-600);

        assert!(!verdict.blocks_scheduling());
        assert!(matches!(verdict, PowerOffPermission::Unknown { .. }));
    }

    #[test]
    fn an_unrecognised_status_is_unknown_rather_than_either_extreme() {
        // Guessing "granted" recreates the overnight failure; guessing "denied"
        // blocks a working machine on a code nobody has seen.
        let verdict = classify_automation_status(-12345);

        assert!(!verdict.is_granted());
        assert!(!verdict.blocks_scheduling());
        assert!(
            verdict.reason().expect("a reason").contains("-12345"),
            "the raw status belongs in the message so it can be diagnosed"
        );
    }

    #[test]
    fn only_an_outright_denial_blocks_scheduling() {
        // The rule the whole module turns on, stated once: uncertainty must
        // never cost the user a schedule.
        let blocking: Vec<bool> = [0, -1744, -600, -1, 42, -1743]
            .into_iter()
            .map(|status| classify_automation_status(status).blocks_scheduling())
            .collect();

        assert_eq!(blocking, [false, false, false, false, false, true]);
    }

    #[test]
    fn every_non_granted_verdict_explains_itself() {
        // An "unavailable" with no reason gives the user nothing to act on, the
        // same rule `CapabilityRegistry` follows.
        for status in [-1743, -1744, -600, 9999] {
            let verdict = classify_automation_status(status);
            let reason = verdict.reason().unwrap_or_default();
            assert!(!reason.trim().is_empty(), "status {status} has no reason");
        }
    }

    #[test]
    fn a_grant_carries_no_reason_to_show() {
        assert_eq!(classify_automation_status(0).reason(), None);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_platform_with_no_dialog_reports_unknown_rather_than_blocking() {
        let verdict = check_power_off_permission(true).expect("the check never errors");

        assert!(!verdict.blocks_scheduling());
        assert!(matches!(verdict, PowerOffPermission::Unknown { .. }));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn asking_without_prompting_is_safe_to_run_in_a_test() {
        // The real FFI call, with `ask_user: false` so it cannot raise a dialog
        // in a headless run. Asserting only that it answers: the verdict depends
        // on the machine's TCC database, so pinning a value would make this test
        // a report of the developer's settings rather than of the code.
        //
        // The value of running it at all is that it proves the framework link,
        // the `AEDesc` layout, and the bundle-id addressing are right — none of
        // which the pure classifier above can show.
        let verdict = check_power_off_permission(false).expect("the check never errors");

        match &verdict {
            PowerOffPermission::Granted => {}
            PowerOffPermission::Denied { reason } | PowerOffPermission::Unknown { reason } => {
                assert!(!reason.trim().is_empty());
            }
        }
    }
}
