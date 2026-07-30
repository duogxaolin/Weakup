//! Windows keep-awake via `SetThreadExecutionState`.
//!
//! UNVERIFIED: this cannot be built or run on the development machine (macOS). It
//! is type-checked against the Windows target only.
//!
//! `SetThreadExecutionState` is **per-thread**, and the state lasts only until the
//! calling thread ends. That makes the obvious implementation — call it inline from
//! whichever task happens to be running — quietly wrong: a tokio worker thread can
//! be parked or retired, and the display assertion would evaporate with it, minutes
//! or hours into a job, with nothing logged.
//!
//! So the flags are owned by one dedicated thread that lives as long as the
//! controller and does nothing else. Acquire and release are messages to it.

use std::sync::mpsc::{self, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread::JoinHandle;

use crate::core::{AppError, AppResult};
use crate::platform::keep_awake::controller::KeepAwakeController;

type ExecutionState = u32;

/// The state stays in effect until the next call — without this, the flags apply to
/// one operation and then lapse.
const ES_CONTINUOUS: ExecutionState = 0x8000_0000;
/// Keeps the display on. This is the flag that distinguishes "screen stays awake"
/// from "machine stays running with a dark screen".
const ES_DISPLAY_REQUIRED: ExecutionState = 0x0000_0002;
/// Also keeps the system from sleeping. Requested alongside the display flag
/// because a system sleep would blank the display regardless.
const ES_SYSTEM_REQUIRED: ExecutionState = 0x0000_0001;

#[link(name = "kernel32")]
extern "system" {
    /// Returns the previous state, or 0 on failure.
    fn SetThreadExecutionState(flags: ExecutionState) -> ExecutionState;
}

enum Command {
    Acquire(mpsc::Sender<AppResult<()>>),
    Release(mpsc::Sender<AppResult<()>>),
    Stop,
}

pub struct WindowsKeepAwakeController {
    commands: Sender<Command>,
    worker: Mutex<Option<JoinHandle<()>>>,
    held: AtomicBool,
}

impl Default for WindowsKeepAwakeController {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsKeepAwakeController {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Command>();

        let worker = std::thread::Builder::new()
            .name("weakup-keep-awake".to_string())
            .spawn(move || {
                // Everything below runs on this one thread for its whole life, which
                // is the entire point of the design.
                let mut asserted = false;

                while let Ok(command) = rx.recv() {
                    match command {
                        Command::Acquire(reply) => {
                            let result = if asserted {
                                Ok(())
                            } else {
                                match set_state(
                                    ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED,
                                ) {
                                    Ok(()) => {
                                        asserted = true;
                                        Ok(())
                                    }
                                    Err(error) => Err(error),
                                }
                            };
                            let _ = reply.send(result);
                        }
                        Command::Release(reply) => {
                            let result = if !asserted {
                                Ok(())
                            } else {
                                // ES_CONTINUOUS alone clears the requirements while
                                // leaving the thread's continuous state defined.
                                match set_state(ES_CONTINUOUS) {
                                    Ok(()) => {
                                        asserted = false;
                                        Ok(())
                                    }
                                    Err(error) => Err(error),
                                }
                            };
                            let _ = reply.send(result);
                        }
                        Command::Stop => break,
                    }
                }

                // Clearing on the way out is not strictly required — the state dies
                // with the thread — but doing it explicitly means the teardown order
                // is not something to reason about.
                if asserted {
                    let _ = set_state(ES_CONTINUOUS);
                }
            })
            .expect("spawning the keep-awake thread");

        Self {
            commands: tx,
            worker: Mutex::new(Some(worker)),
            held: AtomicBool::new(false),
        }
    }

    /// Sends a command and waits for the worker's answer.
    ///
    /// A dead worker is reported rather than silently ignored: without its thread
    /// there is no way to hold the assertion, and a job that believed otherwise
    /// would let the screen sleep while claiming to prevent it.
    fn send(&self, make: impl FnOnce(mpsc::Sender<AppResult<()>>) -> Command) -> AppResult<()> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands
            .send(make(reply_tx))
            .map_err(|_| AppError::KeepAwakeFailed {
                message: "the keep-awake thread is no longer running".to_string(),
            })?;

        reply_rx.recv().map_err(|_| AppError::KeepAwakeFailed {
            message: "the keep-awake thread stopped before answering".to_string(),
        })?
    }
}

fn set_state(flags: ExecutionState) -> AppResult<()> {
    // SAFETY: a plain FFI call taking an integer and returning one. No pointers
    // involved.
    let previous = unsafe { SetThreadExecutionState(flags) };

    if previous == 0 {
        return Err(AppError::KeepAwakeFailed {
            message: format!("SetThreadExecutionState({flags:#x}) failed"),
        });
    }

    Ok(())
}

impl KeepAwakeController for WindowsKeepAwakeController {
    fn acquire(&self) -> AppResult<()> {
        self.send(Command::Acquire)?;
        self.held.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn release(&self) -> AppResult<()> {
        let result = self.send(Command::Release);
        // Cleared first either way: if the call failed, the app should not go on
        // believing it holds an assertion it may not.
        self.held.store(false, Ordering::SeqCst);
        result
    }

    fn is_held(&self) -> bool {
        self.held.load(Ordering::SeqCst)
    }

    fn is_available(&self) -> bool {
        // Present in kernel32 on every supported Windows version, and needs no
        // privilege — unlike shutdown, which does.
        true
    }
}

impl Drop for WindowsKeepAwakeController {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(handle) = worker.take() {
                // Joined so the flags are cleared before the process moves on.
                let _ = handle.join();
            }
        }
    }
}
