import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../domain/job_enums.dart';
import '../../domain/trigger_spec.dart';

/// Widget for configuring a job trigger.
/// Hides [IndefiniteTrigger] for [JobType.powerOff] per spec.
class TriggerInputWidget extends StatefulWidget {
  const TriggerInputWidget({
    required this.jobType,
    required this.trigger,
    required this.onChanged,
    super.key,
  });

  final JobType jobType;
  final TriggerSpec trigger;
  final ValueChanged<TriggerSpec> onChanged;

  @override
  State<TriggerInputWidget> createState() => _TriggerInputWidgetState();
}

enum _TriggerKind { indefinite, duration, absoluteTime }

class _TriggerInputWidgetState extends State<TriggerInputWidget> {
  late _TriggerKind _kind;
  final _durationController = TextEditingController();
  final _hourController = TextEditingController();
  final _minuteController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _syncFromTrigger(widget.trigger);
  }

  @override
  void didUpdateWidget(TriggerInputWidget oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.trigger != widget.trigger) {
      _syncFromTrigger(widget.trigger);
    }
  }

  void _syncFromTrigger(TriggerSpec trigger) {
    switch (trigger) {
      case IndefiniteTrigger():
        _kind = _TriggerKind.indefinite;
      case DurationTrigger(:final minutes):
        _kind = _TriggerKind.duration;
        _durationController.text = minutes.toString();
      case AbsoluteTimeTrigger(:final hour, :final minute):
        _kind = _TriggerKind.absoluteTime;
        _hourController.text = hour.toString().padLeft(2, '0');
        _minuteController.text = minute.toString().padLeft(2, '0');
    }
  }

  @override
  void dispose() {
    _durationController.dispose();
    _hourController.dispose();
    _minuteController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final allowedKinds = [
      if (widget.jobType == JobType.keepAwake) _TriggerKind.indefinite,
      _TriggerKind.duration,
      _TriggerKind.absoluteTime,
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Semantics(
          label: 'Select trigger type',
          child: SegmentedButton<_TriggerKind>(
            segments: [
              if (allowedKinds.contains(_TriggerKind.indefinite))
                const ButtonSegment(
                  value: _TriggerKind.indefinite,
                  label: Text('Indefinite'),
                  icon: Icon(Icons.all_inclusive),
                ),
              const ButtonSegment(
                value: _TriggerKind.duration,
                label: Text('Duration'),
                icon: Icon(Icons.timer),
              ),
              const ButtonSegment(
                value: _TriggerKind.absoluteTime,
                label: Text('At time'),
                icon: Icon(Icons.access_time),
              ),
            ],
            selected: {_kind},
            onSelectionChanged: (selected) {
              setState(() {
                _kind = selected.first;
              });
              _emitChange();
            },
          ),
        ),
        const SizedBox(height: 12),
        if (_kind == _TriggerKind.duration) _buildDurationInput(),
        if (_kind == _TriggerKind.absoluteTime) _buildAbsoluteTimeInput(),
      ],
    );
  }

  Widget _buildDurationInput() {
    return Semantics(
      label: 'Duration in minutes',
      child: TextField(
        controller: _durationController,
        key: const Key('duration_input'),
        keyboardType: TextInputType.number,
        inputFormatters: [FilteringTextInputFormatter.digitsOnly],
        decoration: const InputDecoration(
          labelText: 'Duration (minutes)',
          hintText: 'e.g. 60',
          border: OutlineInputBorder(),
        ),
        onChanged: (_) => _emitChange(),
      ),
    );
  }

  Widget _buildAbsoluteTimeInput() {
    return Row(
      children: [
        Expanded(
          child: Semantics(
            label: 'Hour (0 to 23)',
            child: TextField(
              controller: _hourController,
              key: const Key('hour_input'),
              keyboardType: TextInputType.number,
              inputFormatters: [FilteringTextInputFormatter.digitsOnly],
              decoration: const InputDecoration(
                labelText: 'Hour (0–23)',
                border: OutlineInputBorder(),
              ),
              onChanged: (_) => _emitChange(),
            ),
          ),
        ),
        const Padding(
          padding: EdgeInsets.symmetric(horizontal: 8),
          child: Text(':', style: TextStyle(fontSize: 24)),
        ),
        Expanded(
          child: Semantics(
            label: 'Minute (0 to 59)',
            child: TextField(
              controller: _minuteController,
              key: const Key('minute_input'),
              keyboardType: TextInputType.number,
              inputFormatters: [FilteringTextInputFormatter.digitsOnly],
              decoration: const InputDecoration(
                labelText: 'Minute (0–59)',
                border: OutlineInputBorder(),
              ),
              onChanged: (_) => _emitChange(),
            ),
          ),
        ),
      ],
    );
  }

  void _emitChange() {
    final trigger = switch (_kind) {
      _TriggerKind.indefinite => const IndefiniteTrigger(),
      _TriggerKind.duration => DurationTrigger(
          minutes: int.tryParse(_durationController.text) ?? 0,
        ),
      _TriggerKind.absoluteTime => AbsoluteTimeTrigger(
          hour: int.tryParse(_hourController.text) ?? 0,
          minute: int.tryParse(_minuteController.text) ?? 0,
        ),
    };
    widget.onChanged(trigger);
  }
}
