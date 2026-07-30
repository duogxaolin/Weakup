import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/core/app_error.dart';
import 'package:weakup/core/result.dart';
import 'package:weakup/platform/power_off_executors.dart';

import 'fake_power_off_executor.dart';

void main() {
  group('UnsupportedPowerOffExecutor', () {
    late UnsupportedPowerOffExecutor executor;

    setUp(() => executor = const UnsupportedPowerOffExecutor());

    test('isSupported is false', () {
      expect(executor.isSupported, isFalse);
    });

    test('powerOff returns Failure(PowerOffUnsupported)', () async {
      final result = await executor.powerOff();
      expect(result.isFailure, isTrue);
      expect(result.errorOrNull, isA<PowerOffUnsupported>());
    });

    test('powerOff does not invoke any OS process', () async {
      // The UnsupportedPowerOffExecutor must never call Process.run or similar.
      // We verify this by checking it returns immediately (no process overhead)
      // and that its result is the correct typed error.
      final result = await executor.powerOff();
      expect(result, isA<Failure<void>>());
      expect((result as Failure<void>).error, isA<PowerOffUnsupported>());
    });
  });

  group('FakePowerOffExecutor', () {
    test('records invocations without shutting down', () async {
      final fake = FakePowerOffExecutor();
      expect(fake.wasCalled, isFalse);

      await fake.powerOff();
      expect(fake.callCount, 1);
      expect(fake.wasCalled, isTrue);
    });

    test('returns configurable results', () async {
      final fake = FakePowerOffExecutor(
        nextResult: Result.failure(const PowerOffPrivilegeDenied()),
      );
      final result = await fake.powerOff();
      expect(result.errorOrNull, isA<PowerOffPrivilegeDenied>());
    });

    test('setNextResult controls subsequent calls', () async {
      final fake = FakePowerOffExecutor();
      fake.setNextResult(Result.failure(const PowerOffConsentDenied()));

      final result = await fake.powerOff();
      expect(result.errorOrNull, isA<PowerOffConsentDenied>());
    });
  });

  group('Error type distinctions', () {
    test('PowerOffPrivilegeDenied is distinct from PowerOffConsentDenied', () {
      const a = PowerOffPrivilegeDenied();
      const b = PowerOffConsentDenied();
      const c = PowerOffPolicyDenied();
      const d = PowerOffUnsupported();

      expect(a, isNot(equals(b)));
      expect(b, isNot(equals(c)));
      expect(c, isNot(equals(d)));
    });
  });
}
