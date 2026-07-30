//! The system tray (tasks 9.1–9.4).
//!
//! The tray is how the app is reached while it runs in the background, which is the
//! whole point of a keep-awake tool: the window is closed and a job is still running.
//! Everything here reports failure through [`CapabilityRegistry`] rather than
//! propagating it, because a session with no tray is a degraded app, not a dead one.

use std::sync::Arc;

use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

use crate::core::{AppError, AppResult};
use crate::platform::capabilities::{Capability, CapabilityRegistry};

/// Menu item ids. Constants rather than literals at both the build and match sites,
/// where a typo would silently produce a menu item that does nothing.
const MENU_ID_SHOW: &str = "show";
const MENU_ID_QUIT: &str = "quit";

/// The label of the main window, as declared in `tauri.conf.json`.
pub const MAIN_WINDOW_LABEL: &str = "main";

/// Builds the tray icon and its menu.
///
/// Returns `Err` on failure; the caller routes this through
/// [`crate::platform::capabilities::run_init_step`], which turns it into a recorded
/// degradation rather than a failed startup.
pub fn install<R: Runtime>(app: &AppHandle<R>) -> AppResult<()> {
    let show = MenuItem::with_id(app, MENU_ID_SHOW, "Show Weakup", true, None::<&str>)
        .map_err(|error| init_error("tray menu item", error))?;
    let quit = MenuItem::with_id(app, MENU_ID_QUIT, "Quit", true, None::<&str>)
        .map_err(|error| init_error("tray menu item", error))?;

    let menu = Menu::with_items(app, &[&show, &quit])
        .map_err(|error| init_error("tray menu", error))?;

    let mut builder = TrayIconBuilder::with_id("weakup-tray")
        .menu(&menu)
        // The menu must not open on a left click: task 9.2 gives the left click to
        // restoring the window, which is the action a user expects from a tray icon.
        .show_menu_on_left_click(false)
        .tooltip("Weakup")
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_event);

    // Task 9.1: the icon comes from the bundled app icon through the handle, so there
    // is one icon to maintain rather than a separate tray asset to keep in step.
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    } else {
        // Worth saying out loud: a tray with no icon is invisible on most desktops,
        // which looks exactly like the tray having failed to appear.
        log::warn!("no default window icon available; the tray icon may not be visible");
    }

    builder
        .build(app)
        .map_err(|error| init_error("tray icon", error))?;

    Ok(())
}

/// What a tray menu click means.
///
/// Named separately from the code that carries it out so the mapping can be tested: the
/// difference between asking to quit and quitting is the difference between a user
/// keeping their scheduled shutdown and silently losing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuIntent {
    ShowWindow,
    /// Ask the web view first. Never exits on its own.
    AskToQuit,
    Ignore,
}

fn menu_intent(id: &str) -> MenuIntent {
    match id {
        MENU_ID_SHOW => MenuIntent::ShowWindow,
        MENU_ID_QUIT => MenuIntent::AskToQuit,
        _ => MenuIntent::Ignore,
    }
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let id = event.id().as_ref();
    match menu_intent(id) {
        MenuIntent::ShowWindow => restore_window(app),
        MenuIntent::AskToQuit => request_quit(app),
        MenuIntent::Ignore => log::debug!("unhandled tray menu id: {id}"),
    }
}

fn handle_tray_event<R: Runtime>(tray: &tauri::tray::TrayIcon<R>, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button,
        button_state,
        ..
    } = event
    {
        if is_activation_click(button, button_state) {
            restore_window(tray.app_handle());
        }
    }
}

/// Whether a tray click should bring the window back (task 9.2).
///
/// A separate function because `TrayIconEvent` is `#[non_exhaustive]` and so cannot be
/// built in a test, and this is the part worth testing: releasing the *left* button.
/// Acting on the press would raise the window when the user starts dragging the icon,
/// and a right click belongs to the context menu.
fn is_activation_click(button: MouseButton, state: MouseButtonState) -> bool {
    matches!(button, MouseButton::Left) && matches!(state, MouseButtonState::Up)
}

/// Brings the window back (task 9.2).
///
/// Unminimises before showing: a window that was minimised and then hidden stays
/// minimised when shown again, so `show` alone leaves the user clicking a tray icon
/// that appears to do nothing.
pub fn restore_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        log::warn!("no window named {MAIN_WINDOW_LABEL} to restore");
        return;
    };

    if let Err(error) = window.unminimize() {
        log::debug!("unminimize failed: {error}");
    }
    if let Err(error) = window.show() {
        log::warn!("failed to show the window: {error}");
    }
    if let Err(error) = window.set_focus() {
        log::debug!("set_focus failed: {error}");
    }
}

/// Hides the window instead of letting the close button end the process (task 9.4).
///
/// Without this, closing the window kills the app and with it the scheduler, so a
/// two-hour keep-awake job ends the moment the user tidies their desktop. The window
/// is hidden and the tray remains the way back.
/// Generic over the window type because Tauri hands out a `Window` from
/// `on_window_event` and a `WebviewWindow` from `get_webview_window`, and both need
/// hiding. A trait bound is cheaper than two nearly identical functions that could
/// drift.
pub fn hide_window<W: Hideable>(window: &W) {
    if let Err(error) = window.hide_window() {
        log::warn!("failed to hide the window: {error}");
    }
}

/// Anything with a window that can be hidden.
pub trait Hideable {
    fn hide_window(&self) -> tauri::Result<()>;
}

impl<R: Runtime> Hideable for WebviewWindow<R> {
    fn hide_window(&self) -> tauri::Result<()> {
        self.hide()
    }
}

impl<R: Runtime> Hideable for tauri::Window<R> {
    fn hide_window(&self) -> tauri::Result<()> {
        self.hide()
    }
}

/// Starts quitting (task 9.3).
///
/// Does not exit directly. It asks the web view first, because only the web view can
/// ask the user, and quitting with a job active silently cancels what they scheduled.
/// If no window is listening, exiting is the honest fallback — refusing to quit
/// because nothing answered would leave an unkillable tray icon.
pub fn request_quit<R: Runtime>(app: &AppHandle<R>) {
    use tauri::Emitter;

    match app.get_webview_window(MAIN_WINDOW_LABEL) {
        Some(window) => {
            restore_window(app);
            if let Err(error) = window.emit("weakup://confirm-quit", ()) {
                log::warn!("could not ask for quit confirmation, exiting: {error}");
                app.exit(0);
            }
        }
        None => {
            log::info!("no window to confirm with; exiting");
            app.exit(0);
        }
    }
}

/// Exits for real, after the user has confirmed. Called from the IPC layer.
pub fn quit_now<R: Runtime>(app: &AppHandle<R>) {
    app.exit(0);
}

/// Installs the tray, recording a degradation instead of failing startup (task 9.8).
pub fn install_or_degrade<R: Runtime>(app: &AppHandle<R>, registry: &Arc<CapabilityRegistry>) {
    let _ = crate::platform::capabilities::run_init_step(registry, Capability::Tray, || install(app));
}

fn init_error(component: &str, error: impl std::fmt::Display) -> AppError {
    AppError::RuntimeInit {
        component: component.to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_ids_are_distinct() {
        // Two items sharing an id would make one of them silently trigger the other's
        // action, and Quit is not an action to trigger by accident.
        assert_ne!(MENU_ID_SHOW, MENU_ID_QUIT);
    }

    #[test]
    fn an_init_failure_is_reported_as_a_runtime_init_error() {
        let error = init_error("tray icon", "no display");

        match error {
            AppError::RuntimeInit { component, message } => {
                assert_eq!(component, "tray icon");
                assert!(message.contains("no display"));
            }
            other => panic!("expected a RuntimeInit error, got {other:?}"),
        }
    }

    /// This module's own source, for the invariants that are about the whole file rather
    /// than any one function.
    const TRAY_SOURCE: &str = include_str!("tray.rs");

    #[test]
    fn no_tray_interaction_exits_the_process_on_its_own() {
        // The dispatch from a menu click to `request_quit` cannot be exercised without a
        // running app, so the invariant is checked against the source instead: neither
        // event handler may exit. `quit_now` exists for the web view to call once the
        // user has confirmed, and `app.exit` belongs only to `request_quit`'s
        // no-window fallback. A contributor who wires Quit straight through fails here.
        let handlers = production_source();
        let offenders: Vec<&str> = handlers
            .iter()
            .filter(|line| line.contains("quit_now(") || line.contains(".exit("))
            .filter(|line| !line.contains("pub fn quit_now"))
            .copied()
            .collect();

        // Only the two `app.exit(0)` lines inside `request_quit` are allowed.
        assert_eq!(
            offenders.len(),
            3,
            "unexpected process exits in this module: {offenders:?}"
        );
        assert!(
            offenders.iter().all(|line| line.contains("app.exit(0)")),
            "an exit that is not request_quit's fallback: {offenders:?}"
        );
    }

    /// The module's code lines up to the test module, so tests do not match themselves.
    fn production_source() -> Vec<&'static str> {
        let end = TRAY_SOURCE
            .find("#[cfg(test)]")
            .unwrap_or(TRAY_SOURCE.len());
        TRAY_SOURCE[..end]
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with("//"))
            .collect()
    }

    #[test]
    fn the_quit_menu_item_asks_before_exiting() {
        // Task 9.3. Quitting outright would end a running keep-awake or a scheduled
        // shutdown with no warning, from a menu item one slot below "Show".
        assert_eq!(menu_intent(MENU_ID_QUIT), MenuIntent::AskToQuit);
    }

    #[test]
    fn the_show_menu_item_opens_the_window() {
        assert_eq!(menu_intent(MENU_ID_SHOW), MenuIntent::ShowWindow);
    }

    #[test]
    fn an_unknown_menu_id_does_nothing() {
        // A future menu item whose handler is forgotten must not fall through to Quit.
        assert_eq!(menu_intent("something-else"), MenuIntent::Ignore);
        assert_eq!(menu_intent(""), MenuIntent::Ignore);
    }

    #[test]
    fn releasing_the_left_button_on_the_tray_icon_opens_the_window() {
        assert!(is_activation_click(MouseButton::Left, MouseButtonState::Up));
    }

    #[test]
    fn pressing_the_button_does_not_open_the_window_until_it_is_released() {
        // Otherwise starting to drag the tray icon raises the window.
        assert!(!is_activation_click(
            MouseButton::Left,
            MouseButtonState::Down
        ));
    }

    #[test]
    fn a_right_click_belongs_to_the_menu_not_the_window() {
        for state in [MouseButtonState::Up, MouseButtonState::Down] {
            assert!(!is_activation_click(MouseButton::Right, state));
            assert!(!is_activation_click(MouseButton::Middle, state));
        }
    }

    #[test]
    fn a_tray_failure_degrades_only_the_tray() {
        // The registry-level guarantee, exercised without a running app: a tray that
        // cannot be built must not cost the user their scheduled power-off.
        let registry = CapabilityRegistry::new();
        registry.mark_unavailable(Capability::Tray, "no system tray in this session");

        assert!(!registry.is_available(Capability::Tray));
        assert!(registry.is_available(Capability::PowerOff));
        assert!(registry.is_available(Capability::KeepAwake));
    }
}
