import 'package:flutter/material.dart';

enum NoticeType { warning, info }

/// Displays a capability limitation notice in plain language.
/// Must be shown BEFORE the Create action, not after.
class CapabilityNotice extends StatelessWidget {
  const CapabilityNotice({
    required this.message,
    this.type = NoticeType.warning,
    super.key,
  });

  final String message;
  final NoticeType type;

  @override
  Widget build(BuildContext context) {
    final isWarning = type == NoticeType.warning;
    final bgColor =
        isWarning ? Colors.orange.shade50 : Colors.blue.shade50;
    final borderColor =
        isWarning ? Colors.orange.shade700 : Colors.blue.shade700;
    final iconColor =
        isWarning ? Colors.orange.shade800 : Colors.blue.shade800;
    final icon = isWarning ? Icons.warning_amber_rounded : Icons.info_outline;

    return Semantics(
      label: 'Notice: $message',
      child: Container(
        padding: const EdgeInsets.all(12),
        margin: const EdgeInsets.only(bottom: 12),
        decoration: BoxDecoration(
          color: bgColor,
          border: Border.all(color: borderColor),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon, color: iconColor, size: 20),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                message,
                style: TextStyle(
                  color: isWarning ? Colors.orange.shade900 : Colors.blue.shade900,
                  fontSize: 14,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
