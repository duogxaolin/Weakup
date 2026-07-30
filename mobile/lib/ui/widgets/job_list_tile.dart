import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_timezone/flutter_timezone.dart';
import 'package:timezone/timezone.dart' as tz;

import '../../application/providers.dart';
import '../../domain/job.dart';
import '../../domain/job_enums.dart';
import '../screens/job_detail_screen.dart';

/// A list tile for a single job, showing type, trigger, live countdown, and status.
class JobListTile extends ConsumerStatefulWidget {
  const JobListTile({required this.job, super.key});
  final Job job;

  @override
  ConsumerState<JobListTile> createState() => _JobListTileState();
}

class _JobListTileState extends ConsumerState<JobListTile> {
  Timer? _timer;
  Duration? _remaining;

  @override
  void initState() {
    super.initState();
    _updateRemaining();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) _updateRemaining();
    });
  }

  void _updateRemaining() {
    setState(() {
      _remaining = widget.job.remainingFrom(DateTime.now().toUtc());
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final caps = ref.watch(platformCapabilitiesProvider);
    final job = widget.job;
    final isReminder =
        job.type == JobType.powerOff && !caps.supportsPowerOff;

    final typeLabel = _typeLabel(job.type, isReminder);
    final statusLabel = _statusLabel(job.status);
    final countdownText = _countdownText();

    return Semantics(
      label: '$typeLabel job, $countdownText, status: $statusLabel',
      child: ListTile(
        leading: Icon(
          _typeIcon(job.type),
          color: _statusColor(job.status, context),
          semanticLabel: typeLabel,
        ),
        title: Text(typeLabel),
        subtitle: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(countdownText),
            Text(
              statusLabel,
              style: TextStyle(
                color: _statusColor(job.status, context),
                fontWeight: FontWeight.w500,
              ),
            ),
          ],
        ),
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (job.status == JobStatus.active)
              Semantics(
                label: 'Pause job',
                child: IconButton(
                  icon: const Icon(Icons.pause),
                  tooltip: 'Pause',
                  onPressed: () => _pause(context),
                ),
              ),
            if (job.status == JobStatus.paused)
              Semantics(
                label: 'Resume job',
                child: IconButton(
                  icon: const Icon(Icons.play_arrow),
                  tooltip: 'Resume',
                  onPressed: () => _resume(context),
                ),
              ),
            if (job.status == JobStatus.active || job.status == JobStatus.paused)
              Semantics(
                label: 'Cancel job',
                child: IconButton(
                  icon: const Icon(Icons.cancel_outlined),
                  tooltip: 'Cancel',
                  onPressed: () => _cancel(context),
                ),
              ),
          ],
        ),
        onTap: () => Navigator.push(
          context,
          MaterialPageRoute(builder: (_) => JobDetailScreen(job: job)),
        ),
      ),
    );
  }

  String _countdownText() {
    final target = widget.job.targetInstantUtc;
    if (target == null) return 'Indefinite';
    final r = _remaining;
    if (r == null || r == Duration.zero) return 'Elapsed';
    final h = r.inHours;
    final m = r.inMinutes.remainder(60);
    final s = r.inSeconds.remainder(60);
    return h > 0 ? '${h}h ${m}m ${s}s' : '${m}m ${s}s';
  }

  String _typeLabel(JobType type, bool isReminder) {
    return switch (type) {
      JobType.keepAwake => 'Keep Awake',
      JobType.powerOff => isReminder ? 'Power-off Reminder' : 'Power Off',
    };
  }

  IconData _typeIcon(JobType type) {
    return switch (type) {
      JobType.keepAwake => Icons.lightbulb_outline,
      JobType.powerOff => Icons.power_settings_new,
    };
  }

  String _statusLabel(JobStatus status) {
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

  Color _statusColor(JobStatus status, BuildContext context) {
    return switch (status) {
      JobStatus.active => Colors.green.shade700,
      JobStatus.paused => Colors.orange.shade700,
      JobStatus.completed => Colors.blue.shade700,
      JobStatus.failed => Theme.of(context).colorScheme.error,
      JobStatus.overdue => Colors.deepOrange.shade700,
      _ => Colors.grey.shade600,
    };
  }

  Future<void> _pause(BuildContext context) async {
    final scheduler = ref.read(jobSchedulerProvider);
    if (scheduler == null) return;
    await scheduler.pauseJob(widget.job.id);
  }

  Future<void> _resume(BuildContext context) async {
    final scheduler = ref.read(jobSchedulerProvider);
    if (scheduler == null) return;
    try {
      final tzInfo = await FlutterTimezone.getLocalTimezone();
      final location = tz.getLocation(tzInfo.identifier);
      await scheduler.resumeJob(widget.job.id, location);
    } catch (_) {
      await scheduler.resumeJob(widget.job.id, tz.UTC);
    }
  }

  Future<void> _cancel(BuildContext context) async {
    final scheduler = ref.read(jobSchedulerProvider);
    if (scheduler == null) return;
    await scheduler.cancelJob(widget.job.id);
  }
}
