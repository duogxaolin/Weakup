import 'package:flutter/material.dart';

/// WCAG 2.1 AA compliant theme.
/// Text contrast ≥ 4.5:1 against background; large text/icons ≥ 3:1.
class WeakupTheme {
  WeakupTheme._();

  static const _primary = Color(0xFF1565C0); // Blue 800
  static const _onPrimary = Color(0xFFFFFFFF);
  static const _secondary = Color(0xFF0D47A1); // Blue 900
  static const _background = Color(0xFFF5F5F5);
  static const _onSurface = Color(0xFF212121); // Near black — ≥ 4.5:1 on white
  static const _error = Color(0xFFB71C1C); // Red 900 — ≥ 4.5:1 on white
  static const _warning = Color(0xFFE65100); // Deep Orange 900

  static ThemeData get light => ThemeData(
        colorScheme: const ColorScheme.light(
          primary: _primary,
          onPrimary: _onPrimary,
          secondary: _secondary,
          surface: _background,
          onSurface: _onSurface,
          error: _error,
        ),
        useMaterial3: true,
        textTheme: const TextTheme(
          bodyLarge: TextStyle(
            color: _onSurface,
            fontSize: 16,
            fontWeight: FontWeight.normal,
          ),
          bodyMedium: TextStyle(
            color: _onSurface,
            fontSize: 14,
          ),
          titleLarge: TextStyle(
            color: _onSurface,
            fontSize: 22,
            fontWeight: FontWeight.w600,
          ),
          titleMedium: TextStyle(
            color: _onSurface,
            fontSize: 16,
            fontWeight: FontWeight.w500,
          ),
        ),
        // Visible focus indicator for keyboard navigation.
        focusColor: const Color(0x1F1565C0),
        hoverColor: const Color(0x0A1565C0),
        highlightColor: const Color(0x1F1565C0),
      );

  /// Warning color that meets 3:1 contrast on white background.
  static const Color warningColor = _warning;
  static const Color errorColor = _error;
}
