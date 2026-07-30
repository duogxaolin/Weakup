import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_timezone/flutter_timezone.dart';
import 'package:timezone/timezone.dart' as tz;

import '../../application/providers.dart';
import '../../domain/job.dart';
import '../../domain/job_enums.dart';
import '../../domain/trigger_spec.dart';
import '../widgets/capability_notice.dart';

/// Detail view for a single job.
class JobDetailScreen extends ConsumerStatefulWidget {
  const JobDetailScreen({required this.job, super.key});
  final Job job;

  @override
  ConsumerState<JobDetailScreen> createState() => _JobDetailScreenState();
}

class _JobDetailScreenState extends ConsumerState<JobDetailScreen> {
  tz.Location? _location;

  @override
  void initState() {
    super.initState();
    _loadTimezone();
  }

  Future<void> _loadTimezone() async {
    try {
      final tzInfo = await FlutterTimezone.getLocalTimezone();
      setState(() => _location = tz.getLocation(tzInfo.identifier));
    } catch (_) {
      setState(() => _location = tz.UTC);
    }
  }

  @override
  Widget build(BuildContext context) {
    final caps = ref.watch(platformCapabilitiesProvider);
    final job = widget.job;
    final isReminder = job.type == JobType.powerOff && !caps.supportsPowerOff;

    return Scaffold(
      appBar: AppBar(
        title: Text(isReminder ? 'Power-off Reminder' : _jobTypeName(job.type)),
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Type badge
            Semantics(
              label: 'Job type: ${_jobTypeName(job.type)}',
              child: Chip(
                label: Text(_jobTypeName(job.type)),
                avatar: Icon(_jobTypeIcon(job.type)),
              ),
            ),
            const SizedBox(height: 16),

            // Status
            Semantics(
              label: 'Status: ${_statusName(job.status)}',
              child: Row(
                children: [
                  const Text('Status: ', style: TextStyle(fontWeight: FontWeight.bold)),
                  Text(_statusName(job.status)),
                ],
              ),
            ),
            const SizedBox(height: 8),

            // Trigger
            Row(
              children: [
                const Text('Trigger: ', style: TextStyle(fontWeight: FontWeight.bold)),
                Text(_triggerDescription(job)),
              ],
            ),
            const SizedBox(height: 8),

            // Target
            if (job.targetInstantUtc != null)
              Row(
                children: [
                  const Text('Target: ', style: TextStyle(fontWeight: FontWeight.bold)),
                  Text(job.targetInstantUtc!.toLocal().toString()),
                ],
              ),
            const SizedBox(height: 16),

            // Platform-specific notices
            if (job.type == JobType.keepAwake && caps.isMobile && !caps.keepAwakePersistsInBackground)
              const CapabilityNotice(
                message:
                    'This screen stays awake only while the app is open and in '
                    'the foreground. Backgrounding or locking the screen ends the effect.',
              ),
            if (job.type == JobType.keepAwake && !caps.isMobile)
              const CapabilityNotice(
                type: NoticeType.info,
                message:
                    'On macOS, closing the lid or choosing Sleep will still put '
                    'the Mac to sleep despite this active keep-awake job.',
              ),
            if (isReminder)
              const CapabilityNotice(
                message:
                    'This operating system does not allow apps to power off the '
                    'device. A reminder notification will be sent at the scheduled time.',
              ),
            if (caps.isMobile)
              const CapabilityNotice(
                type: NoticeType.info,
                message:
                    'Jobs do not survive a device reboot without reopening the app.',
              ),

            // Failure message
            if (job.failureMessage != null) ...[
              const SizedBox(height: 8),
              Text(
                'Error: ${job.failureMessage}',
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],

            const SizedBox(height: 24),

            // Action buttons
            if (job.status == JobStatus.active) ...[
              _ActionButton(
                label: 'Pause',
                icon: Icons.pause,
                onPressed: _pause,
                semanticsLabel: 'Pause this job',
              ),
              const SizedBox(height: 8),
              _ActionButton(
                label: 'Cancel',
                icon: Icons.cancel_outlined,
                onPressed: _cancel,
                semanticsLabel: 'Cancel and remove this job',
                isDestructive: true,
              ),
            ],
            if (job.status == JobStatus.paused) ...[
              _ActionButton(
                label: 'Resume',
                icon: Icons.play_arrow,
                onPressed: _resume,
                semanticsLabel: 'Resume this paused job',
              ),
              const SizedBox(height: 8),
              _ActionButton(
                label: 'Cancel',
                icon: Icons.cancel_outlined,
                onPressed: _cancel,
                semanticsLabel: 'Cancel and remove this job',
                isDestructive: true,
              ),
            ],
          ],
        ),
      ),
    );
  }

  String _jobTypeName(JobType type) {
    return switch (type) {
      JobType.keepAwake => 'Keep Awake',
      JobType.powerOff => 'Power Off',
    };
  }

  IconData _jobTypeIcon(JobType type) {
    return switch (type) {
      JobType.keepAwake => Icons.lightbulb_outline,
      JobType.powerOff => Icons.power_settings_new,
    };
  }

  String _statusName(JobStatus status) {
    return switch (status) {
      JobStatus.active => 'Active',
      JobStatus.paused => 'Paused',
      JobStatus.completed => 'Completed',
      JobStatus.cancelled => 'Cancelled',
      JobStatus.failed => 'Failed',
      JobStatus.overdue => 'Skipped (overdue)',
      JobStatus.degraded => 'Degraded',
    };
  }

  String _triggerDescription(Job job) {
    return switch (job.trigger) {
      IndefiniteTrigger() => 'Indefinite (until cancelled)',
      DurationTrigger(:final minutes) => '${minutes}min duration',
      AbsoluteTimeTrigger(:final hour, :final minute) =>
        '${hour.toString().padLeft(2, '0')}:${minute.toString().padLeft(2, '0')}',
    };
  }

  Future<void> _pause() async {
    final scheduler = ref.read(jobSchedulerProvider);
    if (scheduler == null) return;
    await scheduler.pauseJob(widget.job.id);
    if (mounted) Navigator.pop(context);
  }

  Future<void> _resume() async {
    final scheduler = ref.read(jobSchedulerProvider);
    final location = _location ?? tz.UTC;
    if (scheduler == null) return;
    await scheduler.resumeJob(widget.job.id, location);
    if (mounted) Navigator.pop(context);
  }

  Future<void> _cancel() async {
    final scheduler = ref.read(jobSchedulerProvider);
    if (scheduler == null) return;
    await scheduler.cancelJob(widget.job.id);
    if (mounted) Navigator.pop(context);
  }
}

class _ActionButton extends StatelessWidget {
  const _ActionButton({
    required this.label,
    required this.icon,
    required this.onPressed,
    required this.semanticsLabel,
    this.isDestructive = false,
  });

  final String label;
  final IconData icon;
  final VoidCallback onPressed;
  final String semanticsLabel;
  final bool isDestructive;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      child: Semantics(
        label: semanticsLabel,
        child: isDestructive
            ? OutlinedButton.icon(
                icon: Icon(icon,
                    color: Theme.of(context).colorScheme.error),
                label: Text(
                  label,
                  style: TextStyle(
                      color: Theme.of(context).colorScheme.error),
                ),
                onPressed: onPressed,
                style: OutlinedButton.styleFrom(
                  side: BorderSide(
                      color: Theme.of(context).colorScheme.error),
                ),
              )
            : ElevatedButton.icon(
                icon: Icon(icon),
                label: Text(label),
                onPressed: onPressed,
              ),
      ),
    );
  }
}
