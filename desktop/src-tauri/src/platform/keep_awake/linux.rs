//! Linux keep-awake: freedesktop D-Bus idle inhibition, falling back to
//! `systemd-inhibit`.
//!
//! UNVERIFIED: cannot be built or run on the development machine (macOS). The zbus
//! API usage here was type-checked against `aarch64-unknown-linux-gnu`; nothing was
//! executed. The mechanism *choice* in `mechanism.rs` is tested on every host.
//!
//! Two things about D-Bus inhibition drive this design. The inhibition is bound to
//! the *connection*, so dropping the connection releases it — the connection has to
//! live as long as the job, which rules out call-and-return. And zbus's blocking API
//! must not be called from inside a tokio worker, so, as on Windows, the resource
//! lives on one dedicated thread and acquire/release are messages to it.

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::thread::JoinHandle;

use crate::core::{AppError, AppResult};
use crate::platform::keep_awake::controller::KeepAwakeController;
use crate::platform::keep_awake::mechanism::{
    select_mechanism, unavailable_reason, LinuxMechanism, LinuxProbe,
};

const DBUS_SCREENSAVER_SERVICE: &str = "org.freedesktop.ScreenSaver";
const DBUS_SCREENSAVER_PATH: &str = "/org/freedesktop/ScreenSaver";
const APP_NAME: &str = "Weakup";
const REASON: &str = "A keep-awake job is running";

/// Named to avoid confusion with `std::process::Command`, used below for the
/// `systemd-inhibit` fallback.
enum KeepAwakeCommand {
    Acquire(mpsc::Sender<AppResult<()>>),
    Release(mpsc::Sender<AppResult<()>>),
    Stop,
}

/// Whatever is currently holding the screen awake.
enum Holder {
    None,
    /// The connection is kept alive deliberately: dropping it releases the
    /// inhibition, cookie or no cookie.
    DBus {
        connection: zbus::blocking::Connection,
        cookie: u32,
    },
    /// `systemd-inhibit` holds logind's inhibitor lock for as long as its child
    /// runs, so the child is the lock.
    Process(Child),
}

pub struct LinuxKeepAwakeController {
    commands: Sender<KeepAwakeCommand>,
    worker: Mutex<Option<JoinHandle<()>>>,
    held: AtomicBool,
    mechanism: LinuxMechanism,
    reason: Option<String>,
}

impl LinuxKeepAwakeController {
    /// Probes for a mechanism and starts the worker thread.
    pub fn new() -> Self {
        let probe = probe_system();
        let mechanism = select_mechanism(&probe);
        let reason = unavailable_reason(&probe);

        log::info!("linux keep-awake mechanism: {mechanism:?} (probe: {probe:?})");

        let (tx, rx) = mpsc::channel::<KeepAwakeCommand>();

        let worker = std::thread::Builder::new()
            .name("weakup-keep-awake".to_string())
            .spawn(move || {
                let mut holder = Holder::None;

                while let Ok(command) = rx.recv() {
                    match command {
                        KeepAwakeCommand::Acquire(reply) => {
                            let result = match &holder {
                                Holder::None => match acquire_with(mechanism) {
                                    Ok(new_holder) => {
                                        holder = new_holder;
                                        Ok(())
                                    }
                                    Err(error) => Err(error),
                                },
                                // Already held; a second inhibition would need a
                                // second release to undo.
                                _ => Ok(()),
                            };
                            let _ = reply.send(result);
                        }
                        KeepAwakeCommand::Release(reply) => {
                            let previous = std::mem::replace(&mut holder, Holder::None);
                            let _ = reply.send(release_holder(previous));
                        }
                        KeepAwakeCommand::Stop => break,
                    }
                }

                // The thread owns the holder, so it must release before ending.
                let _ = release_holder(std::mem::replace(&mut holder, Holder::None));
            })
            .expect("spawning the keep-awake thread");

        Self {
            commands: tx,
            worker: Mutex::new(Some(worker)),
            held: AtomicBool::new(false),
            mechanism,
            reason,
        }
    }

    fn send(&self, make: impl FnOnce(mpsc::Sender<AppResult<()>>) -> KeepAwakeCommand) -> AppResult<()> {
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

    pub fn mechanism(&self) -> LinuxMechanism {
        self.mechanism
    }
}

impl Default for LinuxKeepAwakeController {
    fn default() -> Self {
        Self::new()
    }
}

/// Looks for each mechanism. Cheap and done once at construction.
fn probe_system() -> LinuxProbe {
    LinuxProbe::new(dbus_screensaver_available(), systemd_inhibit_available())
}

/// True only when the session bus is reachable *and* some service owns the
/// screensaver name. A reachable bus with no owner would fail at `Inhibit` time,
/// which would look like a broken mechanism rather than an absent one and would skip
/// the fallback.
fn dbus_screensaver_available() -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };

    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&connection) else {
        return false;
    };

    let Ok(name) = zbus::names::BusName::try_from(DBUS_SCREENSAVER_SERVICE) else {
        return false;
    };

    proxy.name_has_owner(name).unwrap_or(false)
}

fn systemd_inhibit_available() -> bool {
    Command::new("systemd-inhibit")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn acquire_with(mechanism: LinuxMechanism) -> AppResult<Holder> {
    match mechanism {
        LinuxMechanism::DBusScreenSaver => acquire_dbus(),
        LinuxMechanism::SystemdInhibit => acquire_systemd_inhibit(),
        LinuxMechanism::None => Err(AppError::KeepAwakeUnavailable {
            detail: unavailable_reason(&LinuxProbe::default()),
        }),
    }
}

fn acquire_dbus() -> AppResult<Holder> {
    let connection = zbus::blocking::Connection::session().map_err(dbus_error)?;

    let proxy = zbus::blocking::Proxy::new(
        &connection,
        DBUS_SCREENSAVER_SERVICE,
        DBUS_SCREENSAVER_PATH,
        DBUS_SCREENSAVER_SERVICE,
    )
    .map_err(dbus_error)?;

    let cookie: u32 = proxy
        .call("Inhibit", &(APP_NAME, REASON))
        .map_err(dbus_error)?;

    Ok(Holder::DBus { connection, cookie })
}

fn acquire_systemd_inhibit() -> AppResult<Holder> {
    // The lock lasts as long as the child. `sleep infinity` is a coreutils
    // extension, so a long finite sleep is used instead — 10 years, re-acquired by
    // the coordinator long before then in any realistic session.
    let child = Command::new("systemd-inhibit")
        .arg("--what=idle:sleep")
        .arg(format!("--who={APP_NAME}"))
        .arg(format!("--why={REASON}"))
        .arg("--mode=block")
        .arg("sleep")
        .arg("315360000")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::KeepAwakeFailed {
            message: format!("could not start systemd-inhibit: {error}"),
        })?;

    Ok(Holder::Process(child))
}

fn release_holder(holder: Holder) -> AppResult<()> {
    match holder {
        Holder::None => Ok(()),
        Holder::DBus { connection, cookie } => {
            let proxy = zbus::blocking::Proxy::new(
                &connection,
                DBUS_SCREENSAVER_SERVICE,
                DBUS_SCREENSAVER_PATH,
                DBUS_SCREENSAVER_SERVICE,
            )
            .map_err(dbus_error)?;

            let result = proxy
                .call::<_, _, ()>("UnInhibit", &(cookie,))
                .map_err(dbus_error);

            // Dropped after UnInhibit either way. Dropping the connection releases
            // the inhibition regardless, so even a failed UnInhibit does not leave
            // the screen pinned awake.
            drop(proxy);
            drop(connection);
            result
        }
        Holder::Process(mut child) => {
            // Killing the child drops logind's inhibitor lock. Then reaped, or it
            // would sit as a zombie for the life of the app.
            let kill = child.kill();
            let _ = child.wait();

            kill.map_err(|error| AppError::KeepAwakeFailed {
                message: format!("could not stop systemd-inhibit: {error}"),
            })
        }
    }
}

fn dbus_error(error: zbus::Error) -> AppError {
    AppError::KeepAwakeFailed {
        message: format!("D-Bus screensaver inhibition failed: {error}"),
    }
}

impl KeepAwakeController for LinuxKeepAwakeController {
    fn acquire(&self) -> AppResult<()> {
        if self.mechanism == LinuxMechanism::None {
            return Err(AppError::KeepAwakeUnavailable {
                detail: self.reason.clone(),
            });
        }

        self.send(KeepAwakeCommand::Acquire)?;
        self.held.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn release(&self) -> AppResult<()> {
        let result = self.send(KeepAwakeCommand::Release);
        self.held.store(false, Ordering::SeqCst);
        result
    }

    fn is_held(&self) -> bool {
        self.held.load(Ordering::SeqCst)
    }

    fn is_available(&self) -> bool {
        self.mechanism != LinuxMechanism::None
    }

    fn unavailable_reason(&self) -> Option<String> {
        self.reason.clone()
    }
}

impl Drop for LinuxKeepAwakeController {
    fn drop(&mut self) {
        let _ = self.commands.send(KeepAwakeCommand::Stop);
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(handle) = worker.take() {
                // Joined so the inhibition is actually released — and any
                // `systemd-inhibit` child reaped — before the process moves on.
                let _ = handle.join();
            }
        }
    }
}
