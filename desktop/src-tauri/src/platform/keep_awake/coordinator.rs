//! Decides *when* the OS assertion should be held, given which jobs want it.
//!
//! Task 8.5 requires that no assertion outlives the last active job. The reliable
//! way to get that is to stop scattering acquire/release calls through the
//! scheduler's cancel, complete, and pause paths — each one an opportunity to
//! forget — and instead make the set of interested jobs the single source of truth.
//! The assertion is held exactly when that set is non-empty.
//!
//! So cancel, complete, and pause are all the same operation here: remove the job.
//! Three call sites that could each leak become one that cannot.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use crate::core::{AppError, AppResult};
use crate::platform::keep_awake::controller::KeepAwakeController;

/// What the coordinator did to the OS assertion, so callers can log the real
/// transitions rather than one line per uninteresting no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssertionChange {
    /// The assertion was just taken (the set went from empty to non-empty).
    Acquired,
    /// The assertion was just dropped (the set went from non-empty to empty).
    Released,
    /// Already in the right state.
    Unchanged,
}

pub struct KeepAwakeCoordinator {
    controller: Arc<dyn KeepAwakeController>,
    /// Jobs currently wanting the screen awake. `BTreeSet` for a deterministic
    /// order in assertions and logs.
    holders: Mutex<BTreeSet<String>>,
}

impl KeepAwakeCoordinator {
    pub fn new(controller: Arc<dyn KeepAwakeController>) -> Self {
        Self {
            controller,
            holders: Mutex::new(BTreeSet::new()),
        }
    }

    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, BTreeSet<String>>> {
        self.holders.lock().map_err(|_| AppError::KeepAwakeFailed {
            message: "keep-awake coordinator state was left inconsistent by a panic".to_string(),
        })
    }

    /// Registers a job as wanting the screen awake, acquiring the OS assertion if
    /// this is the first such job.
    ///
    /// Re-registering the same job id is a no-op, not a second hold. The scheduler
    /// re-reconciles on startup and after edits, so this will be called again for a
    /// job that already holds.
    pub fn add(&self, job_id: &str) -> AppResult<AssertionChange> {
        let mut holders = self.lock()?;
        let was_empty = holders.is_empty();

        // A set, not a counter, so a repeated id cannot become a second holder that
        // would need a second removal to undo. That property is why re-registering is
        // safe, and it comes from the data structure rather than from a check here.
        holders.insert(job_id.to_string());

        if !was_empty {
            return Ok(AssertionChange::Unchanged);
        }

        // First holder: take the assertion. On failure the job must not be left
        // registered, or the set would claim an assertion is held that is not, and
        // no later add would retry the acquire.
        match self.controller.acquire() {
            Ok(()) => Ok(AssertionChange::Acquired),
            Err(error) => {
                holders.remove(job_id);
                Err(error)
            }
        }
    }

    /// Deregisters a job — cancel, complete, and pause all land here — releasing
    /// the OS assertion when it was the last one.
    pub fn remove(&self, job_id: &str) -> AppResult<AssertionChange> {
        let mut holders = self.lock()?;

        if !holders.remove(job_id) {
            return Ok(AssertionChange::Unchanged);
        }

        if !holders.is_empty() {
            return Ok(AssertionChange::Unchanged);
        }

        // Last holder gone. The set is already empty, and stays empty even if the
        // release fails: re-adding the job would not fix a stuck OS assertion, and
        // pretending the job still holds it would keep the app from ever trying to
        // release again.
        match self.controller.release() {
            Ok(()) => Ok(AssertionChange::Released),
            Err(error) => Err(error),
        }
    }

    /// Replaces the whole holder set — used when the scheduler reconciles against
    /// what the database says, so a job that vanished while the app was closed
    /// cannot leave a phantom hold behind.
    pub fn reconcile<I, S>(&self, active_job_ids: I) -> AppResult<AssertionChange>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let desired: BTreeSet<String> = active_job_ids.into_iter().map(Into::into).collect();
        let mut holders = self.lock()?;

        let was_empty = holders.is_empty();
        let will_be_empty = desired.is_empty();
        *holders = desired;

        match (was_empty, will_be_empty) {
            (true, false) => match self.controller.acquire() {
                Ok(()) => Ok(AssertionChange::Acquired),
                Err(error) => {
                    holders.clear();
                    Err(error)
                }
            },
            (false, true) => {
                self.controller.release()?;
                Ok(AssertionChange::Released)
            }
            _ => Ok(AssertionChange::Unchanged),
        }
    }

    /// Number of jobs currently wanting the screen awake.
    pub fn holder_count(&self) -> usize {
        self.holders.lock().map(|h| h.len()).unwrap_or(0)
    }

    pub fn holders(&self) -> Vec<String> {
        self.holders
            .lock()
            .map(|h| h.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether the OS assertion is actually held, asked of the controller rather
    /// than inferred from the set — the two disagreeing is the bug worth surfacing.
    pub fn is_asserting(&self) -> bool {
        self.controller.is_held()
    }

    pub fn is_available(&self) -> bool {
        self.controller.is_available()
    }

    pub fn unavailable_reason(&self) -> Option<String> {
        self.controller.unavailable_reason()
    }
}
