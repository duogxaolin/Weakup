use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Startup self-report. The whole reason this project moved desktop to
            // Tauri is that the previous stack could not be run on this machine, so
            // "the window actually exists and is visible" is worth asserting out
            // loud rather than assuming.
            match app.get_webview_window("main") {
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
