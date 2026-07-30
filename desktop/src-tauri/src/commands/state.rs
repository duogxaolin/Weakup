//! The state every IPC command is given.
//!
//! Note what the web view can reach through this and what it cannot. It gets the
//! scheduler, the settings, and the capability registry. It does not get a
//! [`PowerOffExecutor`](crate::platform::power_off::PowerOffExecutor) — that is held
//! privately by the grace-period gate inside the scheduler, which is task 11.4's
//! guarantee: there is no handle here through which a command could shut the machine
//! down without the countdown.

use std::sync::Arc;

use crate::application::JobScheduler;
use crate::core::AppResult;
use crate::data::{Settings, SettingsStore};
use crate::platform::capabilities::CapabilityRegistry;

pub struct AppState {
    scheduler: Arc<JobScheduler>,
    settings: Arc<dyn SettingsStore>,
    capabilities: Arc<CapabilityRegistry>,
}

impl AppState {
    pub fn new(
        scheduler: Arc<JobScheduler>,
        settings: Arc<dyn SettingsStore>,
        capabilities: Arc<CapabilityRegistry>,
    ) -> Self {
        Self {
            scheduler,
            settings,
            capabilities,
        }
    }

    pub fn scheduler(&self) -> &Arc<JobScheduler> {
        &self.scheduler
    }

    pub fn capabilities(&self) -> &Arc<CapabilityRegistry> {
        &self.capabilities
    }

    pub fn settings(&self) -> AppResult<Settings> {
        self.settings.load()
    }

    pub fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        self.settings.save(settings)
    }

    /// The timezone to resolve absolute-time triggers with.
    ///
    /// Read from settings on each use rather than cached, so changing it takes effect
    /// on the next job without a restart.
    pub fn timezone(&self) -> String {
        self.settings()
            .map(|settings| settings.timezone)
            .unwrap_or_else(|error| {
                log::warn!("could not read settings, using the system timezone: {error}");
                crate::data::default_timezone()
            })
    }
}
