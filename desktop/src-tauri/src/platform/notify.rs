//! Desktop notifications (task 9.7).
//!
//! The app spends most of its life with no window on screen, so a notification is
//! usually the only way the user learns that a job finished, that a shutdown is about
//! to happen, or that one failed. It is also the one part of the app that can be
//! switched off by the OS without warning, so nothing here is allowed to fail loudly.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::application::scheduler::SchedulerObserver;
use crate::core::{AppError, AppResult};
use crate::domain::Job;
use crate::platform::capabilities::{Capability, CapabilityRegistry};

/// Shows a notification.
pub fn show<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) -> AppResult<()> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| AppError::Notification {
            message: error.to_string(),
        })
}

/// Confirms at startup that notifications can be built at all.
pub fn probe_or_degrade<R: Runtime>(app: &AppHandle<R>, registry: &Arc<CapabilityRegistry>) {
    let _ = crate::platform::capabilities::run_init_step(registry, Capability::Notifications, || {
        // Permission is requested rather than assumed: on macOS the first `show` would
        // otherwise be silently dropped, and the user would think the app was broken.
        match app.notification().permission_state() {
            Ok(tauri_plugin_notification::PermissionState::Granted) => Ok(()),
            Ok(_) => match app.notification().request_permission() {
                Ok(tauri_plugin_notification::PermissionState::Granted) => Ok(()),
                Ok(_) => Err(AppError::Notification {
                    message: "notifications are turned off for Weakup in system settings"
                        .to_string(),
                }),
                Err(error) => Err(AppError::Notification {
                    message: error.to_string(),
                }),
            },
            Err(error) => Err(AppError::Notification {
                message: error.to_string(),
            }),
        }
    });
}

/// Where a notification goes.
///
/// A trait rather than a direct call to Tauri because the wording of these messages is
/// the whole point of the feature — "the computer will shut down in 60 seconds" is the
/// user's only warning before an irreversible action — and a message that can only be
/// produced by a running app with a granted OS permission is a message nobody tests.
pub trait NotificationSink: Send + Sync {
    fn deliver(&self, title: &str, body: &str) -> AppResult<()>;
}

/// The real sink: the OS notification centre.
pub struct SystemNotifications<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> SystemNotifications<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> NotificationSink for SystemNotifications<R> {
    fn deliver(&self, title: &str, body: &str) -> AppResult<()> {
        show(&self.app, title, body)
    }
}

/// The user's notification preference, shared between the settings command and the
/// observer.
///
/// An atomic rather than a read of the settings table on every event: the observer is
/// called from inside the scheduler tick, which can be holding the grace period open,
/// and a database read there is both slow and able to fail. The settings row remains
/// the durable copy; this is the live one.
#[derive(Debug)]
pub struct NotificationPreference {
    enabled: AtomicBool,
}

impl NotificationPreference {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
        }
    }

    pub fn set(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
}

impl Default for NotificationPreference {
    fn default() -> Self {
        Self::new(true)
    }
}

/// Turns scheduler events into notifications.
///
/// The scheduler knows nothing about Tauri; this is the adapter that joins them. It
/// swallows notification failures on purpose — a notification that could not be shown
/// must not stop a shutdown from happening, and the capability registry already tells
/// the user that notifications are not working.
pub struct NotifyingObserver {
    sink: Arc<dyn NotificationSink>,
    registry: Arc<CapabilityRegistry>,
    preference: Arc<NotificationPreference>,
}

impl NotifyingObserver {
    pub fn new(
        sink: Arc<dyn NotificationSink>,
        registry: Arc<CapabilityRegistry>,
        preference: Arc<NotificationPreference>,
    ) -> Self {
        Self {
            sink,
            registry,
            preference,
        }
    }

    /// Builds an observer that notifies through the OS.
    pub fn for_app<R: Runtime>(
        app: AppHandle<R>,
        registry: Arc<CapabilityRegistry>,
        preference: Arc<NotificationPreference>,
    ) -> Self {
        Self::new(
            Arc::new(SystemNotifications::new(app)),
            registry,
            preference,
        )
    }

    fn notify(&self, title: &str, body: &str) {
        // The user's own choice comes first: an app that keeps notifying after being
        // told not to is one the OS gets muted for entirely. The UI says plainly that
        // turning this off includes the shutdown warning, so the choice is informed.
        if !self.preference.is_enabled() {
            return;
        }
        if !self.registry.is_available(Capability::Notifications) {
            return;
        }
        if let Err(error) = self.sink.deliver(title, body) {
            // Recorded once rather than logged on every event, so a permanently
            // revoked permission shows up in the UI instead of only in a log file.
            self.registry
                .mark_unavailable(Capability::Notifications, error.user_message());
        }
    }
}

impl SchedulerObserver for NotifyingObserver {
    fn job_completed(&self, job: &Job) {
        self.notify(
            "Weakup",
            &format!("{} finished.", describe(job)),
        );
    }

    fn grace_period_started(&self, _job: &Job, seconds: u64) {
        // The most important notification in the app: the user's last chance to stop
        // an irreversible action, so it names the way out.
        self.notify(
            "Shutting down soon",
            &format!(
                "This computer will shut down in {seconds} seconds. \
                 Open Weakup to cancel."
            ),
        );
    }

    fn grace_period_cancelled(&self, _job: &Job) {
        self.notify("Shutdown cancelled", "This computer will stay on.");
    }

    fn power_off_failed(&self, _job: &Job, error: &AppError) {
        // Task 10.10. Silence here is the worst case: the user believes the machine
        // shut down and walks away.
        self.notify(
            "Shutdown failed",
            &format!("{} The computer is still on.", error.user_message()),
        );
    }

    fn job_overdue(&self, _job: &Job, minutes_late: i64) {
        self.notify(
            "Scheduled shutdown missed",
            &format!(
                "A shutdown was due {minutes_late} minutes ago while Weakup was not \
                 running. It was not carried out, so nothing was lost."
            ),
        );
    }
}

/// A short description of a job for a notification body.
fn describe(job: &Job) -> &'static str {
    match job.job_type {
        crate::domain::JobType::KeepAwake => "Keeping the screen awake",
        crate::domain::JobType::PowerOff => "The scheduled shutdown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{JobStatus, JobType, TriggerSpec};
    use chrono::Utc;
    use std::sync::Mutex;

    /// Records what would have been shown, and can fail on demand.
    struct RecordingSink {
        sent: Mutex<Vec<(String, String)>>,
        fail_with: Mutex<Option<AppError>>,
    }

    impl RecordingSink {
        fn working() -> Arc<Self> {
            Arc::new(Self {
                sent: Mutex::new(Vec::new()),
                fail_with: Mutex::new(None),
            })
        }

        fn failing(error: AppError) -> Arc<Self> {
            Arc::new(Self {
                sent: Mutex::new(Vec::new()),
                fail_with: Mutex::new(Some(error)),
            })
        }

        fn sent(&self) -> Vec<(String, String)> {
            self.sent.lock().unwrap().clone()
        }

        fn count(&self) -> usize {
            self.sent.lock().unwrap().len()
        }

        fn last_body(&self) -> String {
            self.sent.lock().unwrap().last().unwrap().1.clone()
        }
    }

    impl NotificationSink for RecordingSink {
        fn deliver(&self, title: &str, body: &str) -> AppResult<()> {
            self.sent
                .lock()
                .unwrap()
                .push((title.to_string(), body.to_string()));
            match self.fail_with.lock().unwrap().clone() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    fn observer(sink: Arc<RecordingSink>) -> (NotifyingObserver, Arc<CapabilityRegistry>) {
        let registry = Arc::new(CapabilityRegistry::new());
        (
            NotifyingObserver::new(
                sink,
                Arc::clone(&registry),
                Arc::new(NotificationPreference::new(true)),
            ),
            registry,
        )
    }

    /// An observer whose user has turned notifications off.
    fn muted_observer(sink: Arc<RecordingSink>) -> NotifyingObserver {
        NotifyingObserver::new(
            sink,
            Arc::new(CapabilityRegistry::new()),
            Arc::new(NotificationPreference::new(false)),
        )
    }

    fn job(job_type: JobType) -> Job {
        Job {
            id: "job-1".to_string(),
            job_type,
            trigger: TriggerSpec::Indefinite,
            status: JobStatus::Active,
            target_instant_utc: None,
            created_at_utc: Utc::now(),
            updated_at_utc: Utc::now(),
            timezone: "UTC".to_string(),
            failure_message: None,
        }
    }

    #[test]
    fn both_job_types_have_a_description() {
        assert!(!describe(&job(JobType::KeepAwake)).is_empty());
        assert!(!describe(&job(JobType::PowerOff)).is_empty());
        assert_ne!(
            describe(&job(JobType::KeepAwake)),
            describe(&job(JobType::PowerOff))
        );
    }

    #[test]
    fn the_shutdown_warning_says_how_long_is_left_and_how_to_stop_it() {
        // The user's only warning before an irreversible action. If it does not say the
        // time remaining and the way out, it is not a warning.
        let sink = RecordingSink::working();
        let (observer, _registry) = observer(Arc::clone(&sink));

        observer.grace_period_started(&job(JobType::PowerOff), 60);

        let (title, body) = sink.sent().pop().unwrap();
        assert!(title.to_lowercase().contains("shut"), "title was {title:?}");
        assert!(body.contains("60"), "body was {body:?}");
        assert!(
            body.to_lowercase().contains("cancel"),
            "the warning must name the way out, was {body:?}"
        );
    }

    #[test]
    fn a_failed_shutdown_says_the_machine_is_still_on() {
        // Silence is the dangerous case: the user believes it shut down and walks away.
        let sink = RecordingSink::working();
        let (observer, _registry) = observer(Arc::clone(&sink));

        observer.power_off_failed(
            &job(JobType::PowerOff),
            &AppError::PowerOffFailed {
                message: "the shutdown command was refused".to_string(),
            },
        );

        let body = sink.last_body().to_lowercase();
        assert!(body.contains("still on"), "body was {body:?}");
    }

    #[test]
    fn a_missed_shutdown_reassures_that_nothing_happened() {
        let sink = RecordingSink::working();
        let (observer, _registry) = observer(Arc::clone(&sink));

        observer.job_overdue(&job(JobType::PowerOff), 90);

        let body = sink.last_body();
        assert!(body.contains("90"), "body was {body:?}");
        assert!(
            body.to_lowercase().contains("not carried out"),
            "body was {body:?}"
        );
    }

    #[test]
    fn turning_notifications_off_actually_stops_them() {
        // The setting is persisted, so it has to do something. A stored preference the
        // code never reads is worse than no setting at all.
        let sink = RecordingSink::working();
        let observer = muted_observer(Arc::clone(&sink));

        observer.grace_period_started(&job(JobType::PowerOff), 60);
        observer.job_completed(&job(JobType::KeepAwake));
        observer.power_off_failed(
            &job(JobType::PowerOff),
            &AppError::PowerOffFailed {
                message: "refused".to_string(),
            },
        );

        assert_eq!(sink.count(), 0);
    }

    #[test]
    fn turning_notifications_back_on_takes_effect_without_a_restart() {
        let sink = RecordingSink::working();
        let preference = Arc::new(NotificationPreference::new(false));
        let observer = NotifyingObserver::new(
            Arc::clone(&sink) as Arc<dyn NotificationSink>,
            Arc::new(CapabilityRegistry::new()),
            Arc::clone(&preference),
        );

        observer.job_completed(&job(JobType::KeepAwake));
        assert_eq!(sink.count(), 0);

        preference.set(true);
        observer.job_completed(&job(JobType::KeepAwake));
        assert_eq!(sink.count(), 1);
    }

    #[test]
    fn nothing_is_sent_once_notifications_are_known_to_be_unavailable() {
        // Otherwise every scheduler tick produces another failing call to an OS that has
        // already said no.
        let sink = RecordingSink::working();
        let (observer, registry) = observer(Arc::clone(&sink));
        registry.mark_unavailable(Capability::Notifications, "permission revoked");

        observer.grace_period_started(&job(JobType::PowerOff), 60);
        observer.job_completed(&job(JobType::KeepAwake));
        observer.grace_period_cancelled(&job(JobType::PowerOff));

        assert_eq!(sink.count(), 0);
    }

    #[test]
    fn a_notification_failure_is_recorded_once_rather_than_retried_forever() {
        let sink = RecordingSink::failing(AppError::Notification {
            message: "permission revoked".to_string(),
        });
        let (observer, registry) = observer(Arc::clone(&sink));

        observer.job_completed(&job(JobType::KeepAwake));
        assert_eq!(sink.count(), 1);
        assert!(
            !registry.is_available(Capability::Notifications),
            "the failure must be recorded so the UI can show it"
        );

        // Every later event is dropped rather than retried.
        observer.job_completed(&job(JobType::KeepAwake));
        observer.grace_period_started(&job(JobType::PowerOff), 60);
        assert_eq!(sink.count(), 1);
    }

    #[test]
    fn a_failed_notification_does_not_stop_the_scheduler() {
        // The observer is called from inside the scheduler tick. A panic or an error
        // escaping here would take the grace period with it.
        let sink = RecordingSink::failing(AppError::Notification {
            message: "no notification centre".to_string(),
        });
        let (observer, _registry) = observer(sink);

        // Each of these returns `()`; the point is that none of them panics.
        observer.grace_period_started(&job(JobType::PowerOff), 60);
        observer.grace_period_cancelled(&job(JobType::PowerOff));
        observer.job_completed(&job(JobType::KeepAwake));
        observer.job_overdue(&job(JobType::PowerOff), 5);
    }
}
