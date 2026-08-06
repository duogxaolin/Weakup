pub mod application;
pub mod commands;
pub mod core;
pub mod data;
pub mod domain;
pub mod platform;
pub mod setup;

use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        // The autostart plugin needs the launch arguments it should register. `--hidden`
        // matters: an app that opens its window on every login is worse than one that
        // does not start at all, because the user sees it and turns the feature off.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::create_job,
            commands::create_job_replacing,
            commands::list_jobs,
            commands::pause_job,
            commands::resume_job,
            commands::cancel_job,
            commands::has_active_job,
            commands::resolve_trigger,
            commands::grace_state,
            commands::cancel_grace_period,
            commands::grace_period_length,
            commands::capability_state,
            commands::check_shutdown_permission,
            commands::get_settings,
            commands::save_settings,
            commands::available_timezones,
            commands::autostart_enabled,
            commands::set_autostart_enabled,
            commands::hide_to_tray,
            commands::quit_app,
            // The remote surface (task 7.1). Note what is absent: there is no
            // power-off command here, and adding one would be the single change that
            // made the countdown optional. A remote request creates a job through the
            // scheduler, which counts down for `REMOTE_GRACE_PERIOD_SECONDS`.
            commands::account_state,
            commands::sign_in_to_account,
            commands::sign_out_of_account,
            commands::present_pairing_code,
            commands::accept_pairing_code,
            commands::list_pairings,
            commands::revoke_pairing,
            commands::list_devices,
            commands::remote_control_enabled,
            commands::set_remote_control_enabled,
        ])
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(if cfg!(debug_assertions) {
                        log::LevelFilter::Info
                    } else {
                        log::LevelFilter::Warn
                    })
                    .build(),
            )?;

            // Fatal if this fails: without a database and a scheduler there is nothing
            // to degrade into, only a window whose buttons quietly do nothing.
            let scheduler = setup::initialise(app.handle())?;

            // Everything from here on degrades instead of failing (task 9.8).
            setup::initialise_optional(app.handle());

            // Pairing degrades the same way. A locked credential store costs the user
            // remote control and nothing else — the scheduler and the countdown are
            // already built by this point and do not consult it.
            setup::initialise_pairing(app.handle());

            // Task 10.1: the scheduler owns every timer, and this is the one place it
            // is started.
            crate::application::JobScheduler::spawn(scheduler);

            // Launched by the autostart agent: start in the tray rather than opening a
            // window over whatever the user is doing at login.
            if std::env::args().any(|argument| argument == "--hidden") {
                if let Some(window) = app.get_webview_window(platform::tray::MAIN_WINDOW_LABEL) {
                    platform::tray::hide_window(&window);
                }
            }

            // Startup self-report. The reason this project moved desktop to Tauri is
            // that the previous stack could not be built on this machine, so "the
            // window actually exists and is visible" is asserted out loud rather than
            // assumed.
            match app.get_webview_window(platform::tray::MAIN_WINDOW_LABEL) {
                Some(window) => {
                    let visible = window.is_visible().unwrap_or(false);
                    let size = window.inner_size().ok();
                    log::info!("main window present: visible={visible} size={size:?}");
                    println!("BOOT_OK visible={visible} size={size:?}");
                }
                None => {
                    log::error!("main window missing at setup");
                    println!("BOOT_FAIL main window missing");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Task 9.4. Closing the window would otherwise end the process and with it
            // the scheduler, so a two-hour keep-awake job would die the moment the user
            // tidied their desktop.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                platform::tray::hide_window(window);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
