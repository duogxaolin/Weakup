import 'dart:io';
import 'dart:typed_data';

import 'package:launch_at_startup/src/app_auto_launcher.dart';
import 'package:win32_registry/win32_registry.dart'
    if (dart.library.html) 'noop.dart';

bool isRunningInMsix(String packageName) {
  final String resolvedExecutable = Platform.resolvedExecutable;
  final bool isMsix = resolvedExecutable.contains('WindowsApps') &&
      resolvedExecutable.contains(packageName);
  return isMsix;
}

class AppAutoLauncherImplWindows extends AppAutoLauncher {
  AppAutoLauncherImplWindows({
    required super.appName,
    required String appPath,
    List<String> args = const [],
  }) : super(appPath: appPath, args: args) {
    _registryValue = args.isEmpty ? appPath : '$appPath ${args.join(' ')}';
  }

  late String _registryValue;

  static const String _runPath =
      r'Software\Microsoft\Windows\CurrentVersion\Run';
  static const String _startupApprovedPath =
      r'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run';

  RegistryKey _openRunKey() => CURRENT_USER.open(
        _runPath,
        config: const RegistryOpenConfig(access: RegistryAccess.readWrite),
      );

  RegistryKey _openStartupApprovedKey() => CURRENT_USER.open(
        _startupApprovedPath,
        config: const RegistryOpenConfig(access: RegistryAccess.readWrite),
      );

  static const int _startupApprovedRegKeyBytesLength = 12;

  @override
  Future<bool> isEnabled() async {
    final key = _openRunKey();
    try {
      final value = key.getString(appName);
      return value == _registryValue && await _isStartupApproved();
    } finally {
      key.close();
    }
  }

  @override
  Future<bool> enable() async {
    final key = _openRunKey();
    try {
      key.setValue(appName, StringValue(_registryValue));
    } finally {
      key.close();
    }

    final bytes = Uint8List(_startupApprovedRegKeyBytesLength);
    // "2" as a first byte in this register means that the autostart is enabled.
    bytes[0] = 2;

    final approvedKey = _openStartupApprovedKey();
    try {
      approvedKey.setValue(appName, BinaryValue(bytes));
    } finally {
      approvedKey.close();
    }

    return true;
  }

  @override
  Future<bool> disable() async {
    final key = _openRunKey();
    try {
      _removeValue(key, appName);
    } finally {
      key.close();
    }

    final approvedKey = _openStartupApprovedKey();
    try {
      _removeValue(approvedKey, appName);
    } finally {
      approvedKey.close();
    }
    return true;
  }

  // https://renenyffenegger.ch/notes/Windows/registry/tree/HKEY_CURRENT_USER/Software/Microsoft/Windows/CurrentVersion/Explorer/StartupApproved/Run/index
  // Odd first byte will prevent the app from autostarting.
  // Empty or absent value means autostart is allowed.
  Future<bool> _isStartupApproved() async {
    final key = _openStartupApprovedKey();
    try {
      final value = key.getBinary(appName);
      if (value == null || value.isEmpty) return true;
      return value[0].isEven;
    } finally {
      key.close();
    }
  }

  void _removeValue(RegistryKey key, String name) {
    if (key.getValue(name) != null) {
      key.removeValue(name);
    }
  }
}

class AppAutoLauncherImplWindowsMsix extends AppAutoLauncher {
  AppAutoLauncherImplWindowsMsix({
    required super.appName,
    required super.appPath,
    required this.packageName,
    super.args,
  });

  final String packageName;

  File get _shortcutFile {
    return File(
      '${Platform.environment['APPDATA']}\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\$appName.lnk',
    );
  }

  @override
  Future<bool> isEnabled() async {
    return _shortcutFile.existsSync();
  }

  @override
  Future<bool> enable() async {
    final String script = '''
    \$TargetPath = "$appPath"
    \$ShortcutFile = "\$env:APPDATA\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\$appName.lnk"
    \$WScriptShell = New-Object -ComObject WScript.Shell
    \$Shortcut = \$WScriptShell.CreateShortcut(\$ShortcutFile)
    \$Shortcut.TargetPath = \$TargetPath
    \$Shortcut.Arguments = "${args.join(' ')}"
    \$Shortcut.Save()
  ''';
    final result = Process.runSync('powershell', ['-Command', script]);
    if (result.stderr != null && result.stderr!.isNotEmpty) {
      throw Exception('Failed to create shortcut: ${result.stderr}');
    }
    return _shortcutFile.existsSync();
  }

  @override
  Future<bool> disable() async {
    if (_shortcutFile.existsSync()) {
      final String script = '''
    Remove-Item -Path "\$env:APPDATA\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\$appName.lnk"
  ''';
      final result = Process.runSync('powershell', ['-Command', script]);
      if (result.stderr != null && result.stderr!.isNotEmpty) {
        throw Exception('Failed to delete shortcut: ${result.stderr}');
      }
    }
    return !_shortcutFile.existsSync();
  }
}
