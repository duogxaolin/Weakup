import 'package:flutter_test/flutter_test.dart';

// The old counter smoke test is removed — Weakup's main.dart no longer has a counter widget.
// Widget tests live in test/ui/*.

void main() {
  testWidgets('Weakup app smoke test placeholder', (WidgetTester tester) async {
    // Intentionally minimal — real widget tests are in test/ui/.
    expect(true, isTrue);
  });
}
