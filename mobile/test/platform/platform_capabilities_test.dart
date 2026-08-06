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
        canBeRemoteTarget: false,
        isDesktop: false,
        isMobile: true,
      );
      expect(fake.supportsPowerOff, isFalse);
      expect(fake.isMobile, isTrue);
    });

    group('Remote-control target capability', () {
      test('desktop platforms can be remote-control targets', () {
        for (final caps in [
          PlatformCapabilities.windows,
          PlatformCapabilities.macos,
          PlatformCapabilities.linux,
        ]) {
          expect(caps.canBeRemoteTarget, isTrue,
              reason: 'a desktop can keep a background presence and can power off');
        }
      });

      test('mobile platforms cannot be remote-control targets', () {
        // Not a limitation of the feature but the point of it: a phone is a useful
        // remote *controller* precisely because it need not be a *target*. The OS
        // suspends the app and forbids power-off, so a command sent to one could
        // neither be received reliably nor carried out — which is why it is refused
        // with the platform-capability reason rather than accepted and dropped.
        for (final caps in [
          PlatformCapabilities.android,
          PlatformCapabilities.ios,
        ]) {
          expect(caps.canBeRemoteTarget, isFalse);
        }
      });

      test('being a target implies being able to power off', () {
        // The two travel together across the whole matrix. A device that could be
        // commanded but could not act would accept a power-off and then do nothing,
        // which is the silent-drop failure the spec rules out.
        for (final caps in [
          PlatformCapabilities.windows,
          PlatformCapabilities.macos,
          PlatformCapabilities.linux,
          PlatformCapabilities.android,
          PlatformCapabilities.ios,
        ]) {
          expect(caps.canBeRemoteTarget, caps.supportsPowerOff, reason: '$caps');
        }
      });
    });
  });
}
