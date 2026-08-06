import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../domain/calendar_date.dart';
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

  /// The optional one-off date for [AbsoluteTimeTrigger].
  ///
  /// Null is a meaningful state, not "unset": it selects the recurring time-of-day
  /// semantic documented on [AbsoluteTimeTrigger]. The date control is therefore only
  /// ever offered for [JobType.powerOff], and clearing back to null must stay reachable.
  CalendarDate? _date;

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
      case AbsoluteTimeTrigger(:final hour, :final minute, :final date):
        _kind = _TriggerKind.absoluteTime;
        _hourController.text = hour.toString().padLeft(2, '0');
        _minuteController.text = minute.toString().padLeft(2, '0');
        // An incoming dated trigger must not lose its date just by being displayed —
        // that would turn a one-off shutdown into a daily alarm on the next edit.
        _date = date;
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
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
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
        ),
        // Power-off only. A dated keep-awake resolves perfectly well, so this is a scope
        // decision rather than a domain rule — the same one `desktop/src/wiring.test.js`
        // records for the web frontend, kept identical so the two UIs do not diverge.
        if (widget.jobType == JobType.powerOff) ...[
          const SizedBox(height: 12),
          _buildDateControl(),
        ],
      ],
    );
  }

  /// The date row: either the recurring hint plus "Pick date", or the chosen date plus a
  /// clear affordance. Both states are reachable in one tap, because null and non-null are
  /// two different behaviours (see [AbsoluteTimeTrigger]) rather than filled/empty.
  Widget _buildDateControl() {
    final date = _date;

    if (date == null) {
      return Row(
        children: [
          const Expanded(
            child: Text(
              'Repeats daily: fires at this time today, or tomorrow if it has '
              'already passed.',
              key: Key('date_recurring_hint'),
            ),
          ),
          const SizedBox(width: 8),
          Semantics(
            label: 'Pick a date for this power off',
            button: true,
            child: TextButton.icon(
              key: const Key('date_pick_button'),
              onPressed: _pickDate,
              icon: const Icon(Icons.event),
              label: const Text('Pick date'),
            ),
          ),
        ],
      );
    }

    return Row(
      children: [
        Expanded(
          child: Text(
            'One-off on ${date.format()}',
            key: const Key('date_value'),
          ),
        ),
        const SizedBox(width: 8),
        Semantics(
          label: 'Change the date for this power off',
          button: true,
          child: TextButton.icon(
            key: const Key('date_pick_button'),
            onPressed: _pickDate,
            icon: const Icon(Icons.event),
            label: const Text('Change'),
          ),
        ),
        Semantics(
          label: 'Clear the date and repeat daily instead',
          button: true,
          child: IconButton(
            key: const Key('date_clear_button'),
            onPressed: () {
              setState(() {
                _date = null;
              });
              _emitChange();
            },
            icon: const Icon(Icons.clear),
            tooltip: 'Clear date',
          ),
        ),
      ],
    );
  }

  Future<void> _pickDate() async {
    final now = DateTime.now();
    final today = DateTime(now.year, now.month, now.day);
    final current = _date;

    final stored = current == null
        ? today
        : DateTime(current.year, current.month, current.day);
    // A stored date can be older than firstDate — it may have arrived on a synced or
    // restored trigger, or simply gone stale in a form left open across midnight. The
    // picker asserts initialDate is not before firstDate, so open on the nearest offer
    // instead of crashing; the stale value stays put until the user actually picks.
    final initialDate = stored.isBefore(today) ? today : stored;

    final picked = await showDatePicker(
      context: context,
      initialDate: initialDate,
      // A date already gone can only ever be refused by the resolver, so do not offer it.
      firstDate: today,
      lastDate: DateTime(today.year + 5, today.month, today.day),
    );

    // The picker awaits a user, so this State may be gone by the time it returns.
    if (!mounted) return;
    if (picked == null) return;

    setState(() {
      // Stored as year/month/day, never as the picker's DateTime: see the class doc on
      // CalendarDate for why a zone-carrying instant must not stand in for a calendar day.
      _date = CalendarDate(
        year: picked.year,
        month: picked.month,
        day: picked.day,
      );
    });
    _emitChange();
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
          date: _date,
        ),
    };
    widget.onChanged(trigger);
  }
}
