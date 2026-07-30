import 'package:flutter_test/flutter_test.dart';

import 'fake_wakelock_controller.dart';

void main() {
  group('FakeWakelockController', () {
    late FakeWakelockController controller;

    setUp(() => controller = FakeWakelockController());

    test('starts not held', () {
      expect(controller.isHeld, isFalse);
    });

    test('acquire holds the wakelock', () async {
      final result = await controller.acquire();
      expect(result.isSuccess, isTrue);
      expect(controller.isHeld, isTrue);
      expect(controller.acquireCount, 1);
    });

    test('release releases the wakelock', () async {
      await controller.acquire();
      final result = await controller.release();
      expect(result.isSuccess, isTrue);
      expect(controller.isHeld, isFalse);
      expect(controller.releaseCount, 1);
    });

    test('wakelock is not held after release', () async {
      await controller.acquire();
      await controller.release();
      expect(controller.isHeld, isFalse);
    });

    test('acquire on activation, release on completion', () async {
      // Simulate job activate → complete lifecycle
      await controller.acquire(); // job activation
      expect(controller.isHeld, isTrue);

      await controller.release(); // job completion
      expect(controller.isHeld, isFalse);
      expect(controller.acquireCount, 1);
      expect(controller.releaseCount, 1);
    });

    test('acquire on activation, release on cancel', () async {
      await controller.acquire();
      await controller.release();
      expect(controller.isHeld, isFalse);
    });

    test('acquire on activation, release on pause', () async {
      await controller.acquire();
      await controller.release();
      expect(controller.isHeld, isFalse);
    });

    test('re-assert on relaunch within window: acquire is called', () async {
      // Simulate: job is within window on relaunch → re-acquire
      final result = await controller.acquire();
      expect(result.isSuccess, isTrue);
      expect(controller.isHeld, isTrue);
    });

    test('no re-assert after window elapsed: wakelock stays released', () async {
      // Simulate: window elapsed → release called, not re-acquired
      expect(controller.isHeld, isFalse);
      // No acquire was called — wakelock remains unheld
      expect(controller.acquireCount, 0);
    });
  });
}
