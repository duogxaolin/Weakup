import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../application/providers.dart';
import '../../platform/platform_capabilities.dart';

/// Minimal settings screen.
///
/// Shows:
/// - Launch-at-startup toggle (desktop only)
/// - Platform information
class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final caps = ref.watch(platformCapabilitiesProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Settings'),
      ),
      body: ListView(
        children: [
          if (caps.supportsAutostart) ...[
            _LaunchAtStartupTile(ref: ref),
            const Divider(),
          ],
          ListTile(
            leading: const Icon(Icons.info_outline),
            title: const Text('Platform'),
            subtitle: Text(_platformName(caps)),
            dense: true,
          ),
          ListTile(
            leading: const Icon(Icons.power_settings_new),
            title: const Text('Power-off support'),
            subtitle: Text(caps.supportsPowerOff ? 'Supported' : 'Not supported'),
            dense: true,
          ),
          ListTile(
            leading: const Icon(Icons.wb_sunny_outlined),
            title: const Text('Background keep-awake'),
            subtitle: Text(
              caps.keepAwakePersistsInBackground
                  ? 'Supported'
                  : 'Foreground only',
            ),
            dense: true,
          ),
        ],
      ),
    );
  }

  String _platformName(PlatformCapabilities caps) {
    if (caps.isDesktop) return 'Desktop';
    if (caps.isMobile) return 'Mobile';
    return 'Web / Unknown';
  }
}

class _LaunchAtStartupTile extends StatefulWidget {
  const _LaunchAtStartupTile({required this.ref});
  final WidgetRef ref;

  @override
  State<_LaunchAtStartupTile> createState() => _LaunchAtStartupTileState();
}

class _LaunchAtStartupTileState extends State<_LaunchAtStartupTile> {
  bool? _enabled;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final runtime = widget.ref.read(desktopRuntimeProvider);
    if (runtime == null) {
      setState(() => _loading = false);
      return;
    }
    final enabled = await runtime.isLaunchAtStartupEnabled();
    if (mounted) {
      setState(() {
        _enabled = enabled;
        _loading = false;
      });
    }
  }

  Future<void> _toggle(bool value) async {
    final runtime = widget.ref.read(desktopRuntimeProvider);
    if (runtime == null) return;
    await runtime.setLaunchAtStartup(enabled: value);
    if (mounted) setState(() => _enabled = value);
  }

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: 'Launch at startup toggle',
      child: SwitchListTile(
        secondary: const Icon(Icons.launch),
        title: const Text('Launch at startup'),
        subtitle: const Text('Start Weakup automatically when you log in'),
        value: _enabled ?? false,
        onChanged: _loading ? null : _toggle,
      ),
    );
  }
}
