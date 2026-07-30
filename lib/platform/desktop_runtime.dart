import 'package:flutter/material.dart';
import 'package:launch_at_startup/launch_at_startup.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

/// Desktop background runtime manager.
///
/// Implements:
/// - Hide-to-tray: `setPreventClose(true)` intercepts window close → hide.
/// - Tray icon with Show and Quit actions.
/// - Quit via `windowManager.destroy()` (bypasses close interception per design.md D14).
/// - Optional launch-at-startup toggle.
class DesktopRuntime with WindowListener, TrayListener {
  static const _trayIconPath = 'assets/tray_icon.png';

  DesktopRuntime(this._onQuit);

  final VoidCallback _onQuit;

  Future<void> init() async {
    await windowManager.ensureInitialized();
    await windowManager.setPreventClose(true);
    windowManager.addListener(this);

    await trayManager.setIcon(_trayIconPath);
    await trayManager.setContextMenu(_buildMenu());
    trayManager.addListener(this);
  }

  Menu _buildMenu() {
    return Menu(items: [
      MenuItem(
        key: 'show',
        label: 'Show Weakup',
      ),
      MenuItem.separator(),
      MenuItem(
        key: 'quit',
        label: 'Quit',
      ),
    ]);
  }

  // ---- WindowListener ----

  @override
  void onWindowClose() async {
    // setPreventClose(true) means the window won't actually close;
    // we hide it instead.
    await windowManager.hide();
  }

  // ---- TrayListener ----

  @override
  void onTrayIconMouseDown() async {
    // Single click: show and focus the window.
    await windowManager.show();
    await windowManager.focus();
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) async {
    switch (menuItem.key) {
      case 'show':
        await windowManager.show();
        await windowManager.focus();
      case 'quit':
        // MUST use destroy(), not close().
        // close() is intercepted by setPreventClose(true) and becomes a no-op.
        await windowManager.destroy();
        _onQuit();
    }
  }

  /// Gets the current launch-at-startup state.
  Future<bool> isLaunchAtStartupEnabled() async {
    return launchAtStartup.isEnabled();
  }

  /// Enables or disables launch-at-startup.
  Future<void> setLaunchAtStartup({required bool enabled}) async {
    if (enabled) {
      await launchAtStartup.enable();
    } else {
      await launchAtStartup.disable();
    }
  }

  void dispose() {
    windowManager.removeListener(this);
    trayManager.removeListener(this);
  }
}
