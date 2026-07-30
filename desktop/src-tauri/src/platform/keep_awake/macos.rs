//! macOS keep-awake via `IOPMAssertionCreateWithName`.
//!
//! `NoDisplaySleepAssertion` is the right assertion type: it keeps the *display*
//! awake, which is what the user asked for. `PreventUserIdleSystemSleep` would let
//! the screen go dark while keeping the machine running — the same distinction
//! `caffeinate -d` versus `caffeinate -i` makes.
//!
//! An assertion is released when the process exits, so a crash cannot permanently
//! wedge the display. It is still released explicitly, because the process is
//! expected to outlive the job.

use std::ffi::c_void;
use std::sync::Mutex;

use crate::core::{AppError, AppResult};
use crate::platform::keep_awake::controller::KeepAwakeController;

type IOPMAssertionID = u32;
type IOReturn = i32;
type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;

const KIO_RETURN_SUCCESS: IOReturn = 0;
/// `kIOPMAssertionLevelOn`.
const ASSERTION_LEVEL_ON: u32 = 255;
/// `kCFStringEncodingUTF8`.
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPMAssertionCreateWithName(
        assertion_type: CFStringRef,
        assertion_level: u32,
        assertion_name: CFStringRef,
        assertion_id: *mut IOPMAssertionID,
    ) -> IOReturn;

    fn IOPMAssertionRelease(assertion_id: IOPMAssertionID) -> IOReturn;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithBytes(
        alloc: CFAllocatorRef,
        bytes: *const u8,
        num_bytes: isize,
        encoding: u32,
        is_external_representation: u8,
    ) -> CFStringRef;

    fn CFRelease(cf: *const c_void);
}

/// An owned `CFString`, released on drop.
///
/// Written as a guard rather than a bare pointer because the two creations below
/// have an early return between them: on the second failing, the first would leak
/// without this.
struct CFString(CFStringRef);

impl CFString {
    fn new(value: &str) -> AppResult<Self> {
        let bytes = value.as_bytes();
        // SAFETY: `bytes` is a valid slice for the length given, and a null
        // allocator selects the default one.
        let raw = unsafe {
            CFStringCreateWithBytes(
                std::ptr::null(),
                bytes.as_ptr(),
                bytes.len() as isize,
                CF_STRING_ENCODING_UTF8,
                0,
            )
        };

        if raw.is_null() {
            return Err(AppError::KeepAwakeFailed {
                message: format!("could not create a CFString for {value:?}"),
            });
        }

        Ok(Self(raw))
    }

    fn as_ref(&self) -> CFStringRef {
        self.0
    }
}

impl Drop for CFString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: created by CFStringCreateWithBytes, so we own one reference,
            // and it is released exactly once because Drop runs once.
            unsafe { CFRelease(self.0) };
        }
    }
}

pub struct MacosKeepAwakeController {
    /// `Some` while an assertion is held. Under a mutex so acquire and release
    /// cannot interleave and lose the id — a lost id is an assertion that can never
    /// be released for the life of the process.
    assertion: Mutex<Option<IOPMAssertionID>>,
    reason: String,
}

impl MacosKeepAwakeController {
    /// `reason` appears in `pmset -g assertions`, which is how a user finds out what
    /// is keeping their display on. Worth making recognisable.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            assertion: Mutex::new(None),
            reason: reason.into(),
        }
    }

    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, Option<IOPMAssertionID>>> {
        self.assertion.lock().map_err(|_| AppError::KeepAwakeFailed {
            message: "the keep-awake assertion state was left inconsistent by a panic".to_string(),
        })
    }
}

impl Default for MacosKeepAwakeController {
    fn default() -> Self {
        Self::new("Weakup is keeping the screen awake")
    }
}

impl KeepAwakeController for MacosKeepAwakeController {
    fn acquire(&self) -> AppResult<()> {
        let mut held = self.lock()?;
        if held.is_some() {
            return Ok(());
        }

        let assertion_type = CFString::new("NoDisplaySleepAssertion")?;
        let assertion_name = CFString::new(&self.reason)?;
        let mut id: IOPMAssertionID = 0;

        // SAFETY: both strings outlive the call, and `id` is a valid out-pointer.
        let result = unsafe {
            IOPMAssertionCreateWithName(
                assertion_type.as_ref(),
                ASSERTION_LEVEL_ON,
                assertion_name.as_ref(),
                &mut id,
            )
        };

        if result != KIO_RETURN_SUCCESS {
            return Err(AppError::KeepAwakeFailed {
                message: format!("IOPMAssertionCreateWithName failed with IOReturn {result:#x}"),
            });
        }

        *held = Some(id);
        Ok(())
    }

    fn release(&self) -> AppResult<()> {
        let mut held = self.lock()?;
        let Some(id) = held.take() else {
            return Ok(());
        };

        // SAFETY: `id` came from a successful IOPMAssertionCreateWithName and was
        // taken out of the state above, so it is released exactly once.
        let result = unsafe { IOPMAssertionRelease(id) };

        if result != KIO_RETURN_SUCCESS {
            return Err(AppError::KeepAwakeFailed {
                message: format!("IOPMAssertionRelease failed with IOReturn {result:#x}"),
            });
        }

        Ok(())
    }

    fn is_held(&self) -> bool {
        self.assertion
            .lock()
            .map(|held| held.is_some())
            .unwrap_or(false)
    }

    fn is_available(&self) -> bool {
        // IOKit power assertions are present on every supported macOS version and
        // need no entitlement or user consent.
        true
    }
}

impl Drop for MacosKeepAwakeController {
    fn drop(&mut self) {
        // Belt and braces. The coordinator releases on the last job going away, but
        // dropping the controller with an assertion still held would leave the
        // display awake until the process exited.
        if let Ok(mut held) = self.assertion.lock() {
            if let Some(id) = held.take() {
                // SAFETY: as in `release`.
                unsafe { IOPMAssertionRelease(id) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These call the real IOKit API. Unlike power-off, that is safe to do: the worst
    // outcome is that this machine's display stays awake for the microseconds
    // between acquire and release.

    #[test]
    fn acquiring_and_releasing_a_real_assertion_works() {
        let controller = MacosKeepAwakeController::new("Weakup test assertion");

        assert!(!controller.is_held());
        controller.acquire().expect("IOKit assertion");
        assert!(controller.is_held());

        controller.release().expect("release");
        assert!(!controller.is_held());
    }

    #[test]
    fn acquiring_twice_holds_one_assertion_and_one_release_ends_it() {
        let controller = MacosKeepAwakeController::default();

        controller.acquire().unwrap();
        let first = *controller.assertion.lock().unwrap();
        controller.acquire().unwrap();
        let second = *controller.assertion.lock().unwrap();

        assert_eq!(first, second, "the second acquire must not replace the id");

        controller.release().unwrap();
        assert!(!controller.is_held());
    }

    #[test]
    fn releasing_without_acquiring_is_not_an_error() {
        let controller = MacosKeepAwakeController::default();
        assert!(controller.release().is_ok());
        assert!(controller.release().is_ok());
    }

    #[test]
    fn a_dropped_controller_does_not_leave_the_display_awake() {
        let controller = MacosKeepAwakeController::new("Weakup drop test");
        controller.acquire().unwrap();
        // Nothing to assert against IOKit from here — the guarantee is that Drop
        // releases rather than leaking. This at least exercises that path.
        drop(controller);
    }

    #[test]
    fn macos_reports_keep_awake_as_available() {
        assert!(MacosKeepAwakeController::default().is_available());
        assert!(MacosKeepAwakeController::default().unavailable_reason().is_none());
    }
}
