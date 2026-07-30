import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/platform/platform_capabilities.dart';

void main() {
  group('PlatformCapabilities', () {
    group('Desktop capabilities (Windows)', () {
      test('Windows reports full support', () {
        final caps = PlatformCapabilities.windows;
        expect(caps.supportsPowerOff, isTrue);
        expect(caps.keepAwakePersistsInBackground, isTrue);
        expect(caps.supportsAutostart, isTrue);
        expect(caps.requiresRuntimeSchedulingPermission, isFalse);
        expect(caps.isDesktop, isTrue);
        expect(caps.isMobile, isFalse);
      });

      test('macOS reports full support', () {
        final caps = PlatformCapabilities.macos;
        expect(caps.supportsPowerOff, isTrue);
        expect(caps.keepAwakePersistsInBackground, isTrue);
        expect(caps.supportsAutostart, isTrue);
        expect(caps.isDesktop, isTrue);
      });

      test('Linux reports full support', () {
        final caps = PlatformCapabilities.linux;
        expect(caps.supportsPowerOff, isTrue);
        expect(caps.keepAwakePersistsInBackground, isTrue);
        expect(caps.supportsAutostart, isTrue);
        expect(caps.isDesktop, isTrue);
      });
    });

    group('Android capabilities', () {
      test('Android reports power-off false, background keepAwake true, autostart false', () {
        final caps = PlatformCapabilities.android;
        expect(caps.supportsPowerOff, isFalse,
            reason: 'Android cannot power off — no public API exists');
        expect(caps.keepAwakePersistsInBackground, isTrue,
            reason: 'Android keep-awake persists while foreground service is active');
        expect(caps.supportsAutostart, isFalse);
        expect(caps.requiresRuntimeSchedulingPermission, isTrue,
            reason: 'Android 13+ requires SCHEDULE_EXACT_ALARM at runtime');
        expect(caps.isMobile, isTrue);
        expect(caps.isDesktop, isFalse);
      });
    });

    group('iOS capabilities', () {
      test('iOS reports power-off false, background keepAwake false, autostart false', () {
        final caps = PlatformCapabilities.ios;
        expect(caps.supportsPowerOff, isFalse,
            reason: 'iOS cannot power off — no public or private API exists');
        expect(caps.keepAwakePersistsInBackground, isFalse,
            reason: 'iOS isIdleTimerDisabled has zero effect when backgrounded');
        expect(caps.supportsAutostart, isFalse);
        expect(caps.isMobile, isTrue);
        expect(caps.isDesktop, isFalse);
      });
    });

    test('Capability instances are constructible explicitly for tests', () {
      // Verifies the injectable constructor works for arbitrary fake capability sets.
      const fake = PlatformCapabilities(
        supportsPowerOff: false,
        keepAwakePersistsInBackground: false,
        supportsAutostart: false,
        requiresRuntimeSchedulingPermission: true,
        isDesktop: false,
        isMobile: true,
      );
      expect(fake.supportsPowerOff, isFalse);
      expect(fake.isMobile, isTrue);
    });
  });
}
