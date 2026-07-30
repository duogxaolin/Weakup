import 'dart:async';

import 'package:flutter/material.dart';

import '../../application/job_scheduler.dart';
import '../../domain/job.dart';

/// Full-screen grace-period countdown displayed before power-off executes.
///
/// The countdown cannot be configured to zero and cannot be disabled.
/// Cancel is the initially-focused element and operable by keyboard.
class GracePeriodScreen extends StatefulWidget {
  const GracePeriodScreen({
    required this.job,
    required this.onCancel,
    required this.onProceed,
    super.key,
  });

  final Job job;
  final VoidCallback onCancel;
  final VoidCallback onProceed;

  @override
  State<GracePeriodScreen> createState() => _GracePeriodScreenState();
}

class _GracePeriodScreenState extends State<GracePeriodScreen> {
  // kGracePeriod is the single source of truth — never hard-code 60 here.
  static int get _totalSeconds => kGracePeriod.inSeconds;

  late int _remaining;
  Timer? _timer;
  final _cancelFocusNode = FocusNode();

  @override
  void initState() {
    super.initState();
    _remaining = _totalSeconds;
    _startTimer();
    // Focus Cancel immediately for keyboard operability.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _cancelFocusNode.requestFocus();
    });
  }

  void _startTimer() {
    _timer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      setState(() {
        _remaining--;
        if (_remaining <= 0) {
          _timer?.cancel();
          widget.onProceed();
        }
      });
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    _cancelFocusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.red.shade900,
      body: SafeArea(
        child: Center(
          child: Padding(
            padding: const EdgeInsets.all(32),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                // Icon
                const Icon(
                  Icons.power_settings_new,
                  color: Colors.white,
                  size: 80,
                ),
                const SizedBox(height: 24),

                // Title
                const Text(
                  'Powering Off',
                  style: TextStyle(
                    color: Colors.white,
                    fontSize: 32,
                    fontWeight: FontWeight.bold,
                  ),
                ),
                const SizedBox(height: 8),

                const Text(
                  'Your device will power off in:',
                  style: TextStyle(color: Colors.white70, fontSize: 16),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 32),

                // Countdown
                Semantics(
                  label: 'Countdown: $_remaining seconds remaining',
                  child: Text(
                    '$_remaining',
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 96,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),
                const SizedBox(height: 8),

                const Text(
                  'seconds',
                  style: TextStyle(color: Colors.white70, fontSize: 20),
                ),
                const SizedBox(height: 48),

                // Cancel button — initially focused, keyboard operable.
                Semantics(
                  label: 'Cancel power-off',
                  child: Focus(
                    focusNode: _cancelFocusNode,
                    child: ElevatedButton.icon(
                      key: const Key('btn_cancel_poweroff'),
                      icon: const Icon(Icons.cancel, size: 28),
                      label: const Text(
                        'Cancel',
                        style: TextStyle(fontSize: 20),
                      ),
                      onPressed: widget.onCancel,
                      style: ElevatedButton.styleFrom(
                        backgroundColor: Colors.white,
                        foregroundColor: Colors.red.shade900,
                        padding: const EdgeInsets.symmetric(
                          horizontal: 40,
                          vertical: 16,
                        ),
                        side: const BorderSide(color: Colors.white, width: 2),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
