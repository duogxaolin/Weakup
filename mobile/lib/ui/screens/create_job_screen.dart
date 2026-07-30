import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_timezone/flutter_timezone.dart';
import 'package:timezone/timezone.dart' as tz;

import '../../application/providers.dart';
import '../../domain/job_enums.dart';
import '../widgets/capability_notice.dart';
import '../widgets/trigger_input.dart';

/// Job creation screen.
///
/// Supports creating a keep-awake job, power-off job, or both in one session.
class CreateJobScreen extends ConsumerStatefulWidget {
  const CreateJobScreen({super.key});

  @override
  ConsumerState<CreateJobScreen> createState() => _CreateJobScreenState();
}

class _CreateJobScreenState extends ConsumerState<CreateJobScreen> {
  tz.Location _location = tz.UTC; // default; updated asynchronously from native

  @override
  void initState() {
    super.initState();
    _loadTimezone();
  }

  Future<void> _loadTimezone() async {
    try {
      final tzInfo = await FlutterTimezone.getLocalTimezone();
      if (mounted) {
        setState(() => _location = tz.getLocation(tzInfo.identifier));
      }
    } catch (_) {
      // Fallback to UTC — already the default, no setState needed.
    }
  }

  @override
  Widget build(BuildContext context) {
    final caps = ref.watch(platformCapabilitiesProvider);
    final formState = ref.watch(jobCreationProvider);
    final notifier = ref.read(jobCreationProvider.notifier);

    return Scaffold(
      appBar: AppBar(
        title: const Text('New Job'),
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // ---- Keep-awake section ----
            _SectionHeader(
              title: 'Keep Screen Awake',
              enabled: formState.keepAwakeEnabled,
              onToggle: notifier.toggleKeepAwake,
              semanticsLabel: 'Enable keep-awake job',
            ),
            if (formState.keepAwakeEnabled) ...[
              // Platform limitation notices — shown before Create.
              if (caps.isMobile && !caps.keepAwakePersistsInBackground)
                const CapabilityNotice(
                  message:
                      'On this device, the screen stays awake only while the app '
                      'is open and in the foreground. Backgrounding the app or '
                      'locking the screen ends the effect.',
                  key: Key('notice_ios_keepawake'),
                ),
              if (caps.isMobile && caps.keepAwakePersistsInBackground)
                const CapabilityNotice(
                  message:
                      'The screen stays awake only while the app is in the '
                      'foreground or the background service is running. Locking '
                      'the screen ends the effect.',
                  key: Key('notice_android_keepawake'),
                ),
              if (!caps.isMobile)
                CapabilityNotice(
                  type: NoticeType.info,
                  message: _keepAwakeDesktopNotice(),
                  key: const Key('notice_desktop_keepawake'),
                ),
              const SizedBox(height: 8),
              TriggerInputWidget(
                jobType: JobType.keepAwake,
                trigger: formState.keepAwakeTrigger,
                onChanged: notifier.setKeepAwakeTrigger,
                key: const Key('trigger_keepawake'),
              ),
              const SizedBox(height: 16),
            ],

            // ---- Power-off section ----
            _SectionHeader(
              title: 'Power Off',
              enabled: formState.powerOffEnabled,
              onToggle: notifier.togglePowerOff,
              semanticsLabel: 'Enable power-off job',
            ),
            if (formState.powerOffEnabled) ...[
              // Power-off limitation notices — shown BEFORE Create button.
              if (!caps.supportsPowerOff)
                const CapabilityNotice(
                  message:
                      'This operating system does not allow apps to power off '
                      'the device automatically. A reminder notification will be '
                      'sent instead so you can power off manually.',
                  key: Key('notice_mobile_poweroff'),
                ),
              if (caps.supportsPowerOff && !caps.isMobile)
                CapabilityNotice(
                  type: NoticeType.info,
                  message: _powerOffDesktopNotice(),
                  key: const Key('notice_desktop_poweroff'),
                ),
              const SizedBox(height: 8),
              TriggerInputWidget(
                jobType: JobType.powerOff,
                trigger: formState.powerOffTrigger,
                onChanged: notifier.setPowerOffTrigger,
                key: const Key('trigger_poweroff'),
              ),
              const SizedBox(height: 16),
            ],

            // ---- Error display ----
            if (formState.error != null)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Text(
                  formState.error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                  key: const Key('error_message'),
                ),
              ),

            // ---- Replace confirmation ----
            if (formState.pendingReplaceType != null)
              _ReplaceConfirmationBanner(
                type: formState.pendingReplaceType!,
                onConfirm: () async {
                  notifier.confirmReplace();
                  await _submit();
                },
                onCancel: () {
                  ref.read(jobCreationProvider.notifier).confirmReplace();
                },
              ),

            const SizedBox(height: 8),

            // ---- Submit button ----
            SizedBox(
              width: double.infinity,
              child: Semantics(
                label: 'Create scheduled job',
                child: ElevatedButton(
                  key: const Key('btn_create'),
                  onPressed: formState.isSubmitting ? null : _submit,
                  child: formState.isSubmitting
                      ? const SizedBox(
                          height: 20,
                          width: 20,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Text('Create'),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _submit() async {
    final location = _location;

    final result = await ref
        .read(jobCreationProvider.notifier)
        .submit(location: location);

    if (result.isSuccess && mounted) {
      // Only pop if no pending confirmation.
      final state = ref.read(jobCreationProvider);
      if (state.pendingReplaceType == null) {
        Navigator.pop(context);
      }
    }
  }

  /// Desktop notice for keep-awake jobs — platform-neutral text so it is
  /// accurate on Windows, macOS, and Linux without naming any one OS.
  String _keepAwakeDesktopNotice() {
    return 'The keep-awake effect is active while this app is running. '
        'Closing the app window minimizes it to the system tray rather than '
        'exiting, so the effect continues until you quit the app or cancel the job.';
  }

  /// Desktop notice for power-off jobs — platform-neutral text so it is
  /// accurate on Windows, macOS, and Linux without naming any one OS.
  String _powerOffDesktopNotice() {
    return 'Your computer will be powered off at the scheduled time. '
        'A 60-second countdown will appear first so you can cancel. '
        'On some systems you may be asked to grant permission the first time.';
  }
}

class _SectionHeader extends StatelessWidget {
  const _SectionHeader({
    required this.title,
    required this.enabled,
    required this.onToggle,
    required this.semanticsLabel,
  });

  final String title;
  final bool enabled;
  final ValueChanged<bool> onToggle;
  final String semanticsLabel;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: semanticsLabel,
      child: Row(
        children: [
          Expanded(
            child: Text(
              title,
              style: Theme.of(context).textTheme.titleMedium,
            ),
          ),
          Switch(
            value: enabled,
            onChanged: onToggle,
          ),
        ],
      ),
    );
  }
}

class _ReplaceConfirmationBanner extends StatelessWidget {
  const _ReplaceConfirmationBanner({
    required this.type,
    required this.onConfirm,
    required this.onCancel,
  });

  final JobType type;
  final VoidCallback onConfirm;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final typeName = type == JobType.keepAwake ? 'keep-awake' : 'power-off';
    return Container(
      padding: const EdgeInsets.all(12),
      margin: const EdgeInsets.only(bottom: 12),
      decoration: BoxDecoration(
        color: Colors.orange.shade50,
        border: Border.all(color: Colors.orange),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Replace existing $typeName job?',
            style: const TextStyle(fontWeight: FontWeight.bold),
            key: Key('replace_confirm_title_${type.name}'),
          ),
          const SizedBox(height: 4),
          Text(
            'A $typeName job is already active. Creating this job will cancel the existing one.',
          ),
          const SizedBox(height: 8),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              TextButton(
                onPressed: onCancel,
                child: const Text('Keep existing'),
              ),
              const SizedBox(width: 8),
              ElevatedButton(
                key: Key('btn_confirm_replace_${type.name}'),
                onPressed: onConfirm,
                child: const Text('Replace'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
