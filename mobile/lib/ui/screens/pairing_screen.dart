// The pairing screen: read a code off the desktop, type it in here.
//
// # What this screen does not do
//
// It does not decide whether a code is valid. Normalisation, the alphabet, the length, expiry,
// and single use all live in `PairingCode.parse` and `evaluateGrant`, which the shared vectors
// pin and the Rust side mirrors. Validating here would be a second definition of what a code
// is, and the one that drifted would be this one.
//
// It also does not record a pairing on its own. `completePairingAtRequester` records the issuer
// only after the issuer confirms it recorded us, and withdraws that record if this side's write
// then fails — design D7. A half-recorded pairing is the worst outcome available: this device
// would believe it is authorized, send commands, and see every one refused for a reason
// visible nowhere in either interface.
//
// The lint below is deliberate and matches the precedent in `pairing_flow.dart`,
// `google_auth.dart`, and `firebase_transport.dart`: `prefer_initializing_formals` suggests
// `A({required this._x})`, which needs the `private-named-parameters` feature and SDK 3.12.
// This package targets lower, and the analyzer rejects the suggested form outright.
// ignore_for_file: prefer_initializing_formals

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../application/pairing_flow.dart';

/// Enters a pairing code presented on another device.
class PairingScreen extends StatefulWidget {
  const PairingScreen({
    super.key,
    required this.onSubmit,
  });

  /// Hands the entered code and peer identity to the pairing flow.
  ///
  /// Injected rather than reached for, so the screen can be pumped in a test without a
  /// database, a keychain, or a relay. It returns the outcome's own prose — the domain
  /// already wrote the refusal messages, and inventing new ones here would mean a user
  /// seeing one thing on their phone and another on their laptop.
  final Future<PairingOutcome> Function({
    required String code,
    required String peerDeviceId,
    required String peerVerifyingKey,
  }) onSubmit;

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final _codeController = TextEditingController();
  final _peerIdController = TextEditingController();
  final _peerKeyController = TextEditingController();

  String? _message;
  bool _paired = false;
  bool _busy = false;

  @override
  void dispose() {
    _codeController.dispose();
    _peerIdController.dispose();
    _peerKeyController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    setState(() {
      _busy = true;
      _message = null;
    });

    try {
      final outcome = await widget.onSubmit(
        // Passed through verbatim. `PairingCode.parse` normalises case and spacing and
        // refuses glyphs outside the alphabet; trimming further here would be a second,
        // divergent rule about what a code is.
        code: _codeController.text,
        peerDeviceId: _peerIdController.text.trim(),
        peerVerifyingKey: _peerKeyController.text.trim(),
      );

      if (!mounted) return;
      setState(() {
        _paired = outcome is PairedOutcome;
        _message = outcome.userMessage;
        if (_paired) {
          _codeController.clear();
          _peerIdController.clear();
          _peerKeyController.clear();
        }
      });
    } catch (error) {
      if (!mounted) return;
      setState(() => _message = '$error');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(title: const Text('Pair a device')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(
            'Open Weakup on the computer you want to control, show a pairing code there, '
            'and type it in below.',
            style: theme.textTheme.bodyMedium,
          ),
          const SizedBox(height: 8),
          // Says out loud that both devices have to be in hand. This is the requirement the
          // whole design rests on — a pairing that could be established remotely and
          // unattended would return the system to trusting whoever controls the account.
          Text(
            'Both devices need to be with you. A code expires a few minutes after it is '
            'shown and works only once.',
            style: theme.textTheme.bodySmall,
          ),
          const SizedBox(height: 24),
          TextField(
            key: const Key('pairing-code-field'),
            controller: _codeController,
            autocorrect: false,
            enableSuggestions: false,
            textCapitalization: TextCapitalization.characters,
            maxLength: 9,
            inputFormatters: [LengthLimitingTextInputFormatter(9)],
            decoration: const InputDecoration(
              labelText: 'Code from the other device',
              hintText: '6 characters',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 16),
          TextField(
            key: const Key('pairing-peer-id-field'),
            controller: _peerIdController,
            autocorrect: false,
            enableSuggestions: false,
            decoration: const InputDecoration(
              labelText: "That computer's ID",
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 16),
          TextField(
            key: const Key('pairing-peer-key-field'),
            controller: _peerKeyController,
            autocorrect: false,
            enableSuggestions: false,
            decoration: const InputDecoration(
              labelText: "That computer's key",
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 24),
          FilledButton(
            key: const Key('pairing-submit-button'),
            onPressed: _busy ? null : _submit,
            child: Text(_busy ? 'Pairing…' : 'Pair this device'),
          ),
          if (_message != null) ...[
            const SizedBox(height: 16),
            Text(
              _message!,
              key: const Key('pairing-message'),
              style: theme.textTheme.bodyMedium?.copyWith(
                color: _paired ? theme.colorScheme.primary : theme.colorScheme.error,
              ),
            ),
          ],
        ],
      ),
    );
  }
}
