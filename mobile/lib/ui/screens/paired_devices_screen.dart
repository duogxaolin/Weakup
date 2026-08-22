// The paired-devices screen: who this phone can command, and the controls to do it.
//
// # Presence is derived, never reported
//
// Each row's presence comes from [presencePresentation], which runs `evaluatePresence` over the
// instant the transport last reported for that peer. There is no online/offline flag from the
// relay anywhere in this file, and no field on [PairedPeer] through which one could arrive —
// see `presence_presentation.dart` for why that absence is the design rather than an omission.
//
// # Requesting a power-off does not power anything off
//
// The two buttons on each row send a *signed command*. What happens at the other end is the
// target's decision: it verifies the signature against a key it recorded in person, checks
// freshness and replay, checks that its owner enabled remote control, and only then creates a
// job. That job counts down for `remoteGracePeriodSeconds` — five minutes, longer than a local
// one, because the person about to lose their work is not the person who pressed the button.
//
// Nothing here can shorten that countdown, and there is no control that tries. The copy says so
// rather than leaving the user to discover it.
//
// See `pairing_screen.dart` for why this lint is suppressed.
// ignore_for_file: prefer_initializing_formals

import 'package:flutter/material.dart';

import '../../domain/presence.dart';
import '../../domain/remote_command.dart';
import 'presence_presentation.dart';

/// One peer this device is paired with, as the list needs it.
///
/// Carries a *last reported instant*, not a presence state and certainly not a boolean. The
/// derivation happens in the widget from this instant, which is what stops a relay's opinion
/// from reaching the screen.
final class PairedPeer {
  const PairedPeer({
    required this.deviceId,
    required this.displayName,
    required this.lastReportedAt,
    required this.revoked,
  });

  final String deviceId;

  /// A label the person recognises. Display only — it confers nothing, and a peer that lied
  /// about its name would still have to present a signature verifying against a recorded key.
  final String displayName;

  /// When this peer last reported presence, or null if it never has.
  final DateTime? lastReportedAt;

  /// Whether the pairing has been withdrawn. A revoked peer is still listed — see the
  /// build method — but confers nothing and offers no controls.
  final bool revoked;
}

/// Lists paired peers and offers to command them.
class PairedDevicesScreen extends StatelessWidget {
  const PairedDevicesScreen({
    super.key,
    required this.peers,
    required this.now,
    required this.onSendCommand,
    required this.onRevoke,
    this.onPairNew,
  });

  final List<PairedPeer> peers;

  /// The instant presence is derived against.
  ///
  /// Passed in rather than read from a clock inside the widget, so a test can place a peer
  /// exactly on a threshold and get a deterministic answer.
  final DateTime now;

  /// Sends a signed command to a peer.
  ///
  /// Takes a [RemoteCommand] rather than a string, so a caller cannot ask for something the
  /// authorization rule has no opinion about. `enableRemoteControl` is a variant of that enum
  /// and is deliberately *not* offered by this screen: it is refused by `authorize` before the
  /// pairing check even runs, and a button for it would be a control that can only fail.
  final Future<String> Function(String deviceId, RemoteCommand command) onSendCommand;

  final Future<void> Function(String deviceId) onRevoke;

  final VoidCallback? onPairNew;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Paired devices'),
        actions: [
          if (onPairNew != null)
            IconButton(
              key: const Key('pair-new-button'),
              icon: const Icon(Icons.add_link),
              tooltip: 'Pair a device',
              onPressed: onPairNew,
            ),
        ],
      ),
      body: peers.isEmpty
          ? const _NoPairings()
          : ListView.separated(
              itemCount: peers.length,
              separatorBuilder: (_, _) => const Divider(height: 1),
              itemBuilder: (context, index) => _PeerTile(
                peer: peers[index],
                now: now,
                onSendCommand: onSendCommand,
                onRevoke: onRevoke,
              ),
            ),
    );
  }
}

class _NoPairings extends StatelessWidget {
  const _NoPairings();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.devices_other, size: 48),
            const SizedBox(height: 16),
            Text(
              'No devices are paired yet.',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            Text(
              'Show a pairing code on the computer you want to control, then enter it here.',
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }
}

class _PeerTile extends StatefulWidget {
  const _PeerTile({
    required this.peer,
    required this.now,
    required this.onSendCommand,
    required this.onRevoke,
  });

  final PairedPeer peer;
  final DateTime now;
  final Future<String> Function(String deviceId, RemoteCommand command) onSendCommand;
  final Future<void> Function(String deviceId) onRevoke;

  @override
  State<_PeerTile> createState() => _PeerTileState();
}

class _PeerTileState extends State<_PeerTile> {
  String? _result;
  bool _busy = false;

  Future<void> _send(RemoteCommand command) async {
    setState(() {
      _busy = true;
      _result = null;
    });

    try {
      final message = await widget.onSendCommand(widget.peer.deviceId, command);
      if (!mounted) return;
      setState(() => _result = message);
    } catch (error) {
      if (!mounted) return;
      setState(() => _result = '$error');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _confirmRevoke() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Unpair this device?'),
        // Says what unpairing does and what it does not. It takes effect locally and at
        // once, with no network needed — which matters because someone unpairing a lost
        // phone is often doing it from somewhere with no signal.
        content: const Text(
          'This device will no longer accept commands from that computer, and this takes '
          'effect immediately even with no network. You can pair it again later.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Keep paired'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Unpair'),
          ),
        ],
      ),
    );

    if (confirmed != true) return;
    await widget.onRevoke(widget.peer.deviceId);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final presence = presencePresentation(
      lastReportedAt: widget.peer.lastReportedAt,
      now: widget.now,
    );

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              // Shape as well as colour. Colour alone would leave the three states
              // indistinguishable to a user who cannot tell them apart.
              Icon(
                _presenceIcon(presence.state),
                size: 18,
                color: _presenceColour(presence.state, theme),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  widget.peer.displayName.isEmpty
                      ? widget.peer.deviceId
                      : widget.peer.displayName,
                  style: theme.textTheme.titleMedium,
                ),
              ),
              Text(
                // A revoked pairing says so rather than showing a presence it cannot act
                // on. Its key is absent from the map commands are checked against, so
                // "online" would be true and entirely misleading.
                widget.peer.revoked ? 'Unpaired' : presence.label,
                key: Key('presence-${widget.peer.deviceId}'),
                style: theme.textTheme.labelMedium,
              ),
            ],
          ),
          const SizedBox(height: 4),
          Text(
            widget.peer.revoked ? 'This pairing was withdrawn.' : presence.detail,
            style: theme.textTheme.bodySmall,
          ),

          // A revoked pairing offers no controls: there is nothing left to command and
          // nothing left to withdraw.
          if (!widget.peer.revoked) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              children: [
                FilledButton.tonalIcon(
                  key: Key('request-power-off-${widget.peer.deviceId}'),
                  onPressed: _busy || !worthSending(presence.state)
                      ? null
                      : () => _send(RemoteCommand.powerOff),
                  icon: const Icon(Icons.power_settings_new, size: 18),
                  label: const Text('Request power off'),
                ),
                FilledButton.tonalIcon(
                  key: Key('request-keep-awake-${widget.peer.deviceId}'),
                  onPressed: _busy || !worthSending(presence.state)
                      ? null
                      : () => _send(RemoteCommand.keepAwake),
                  icon: const Icon(Icons.visibility, size: 18),
                  label: const Text('Request keep awake'),
                ),
                TextButton(
                  key: Key('revoke-${widget.peer.deviceId}'),
                  onPressed: _busy ? null : _confirmRevoke,
                  child: const Text('Unpair'),
                ),
              ],
            ),

            // Stated on the row rather than in a dialog after the fact. A user pressing
            // "request power off" should already know the machine will warn its owner and
            // wait, and that the owner can stop it.
            Text(
              'The computer warns whoever is using it and waits five minutes. They can '
              'cancel it.',
              style: theme.textTheme.bodySmall,
            ),

            if (!worthSending(presence.state))
              Text(
                'That computer has not reported in for a while, so a command may not '
                'reach it.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),

            if (_result != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  _result!,
                  key: Key('result-${widget.peer.deviceId}'),
                  style: theme.textTheme.bodyMedium,
                ),
              ),
          ],
        ],
      ),
    );
  }

  IconData _presenceIcon(PresenceState state) {
    switch (state) {
      case PresenceState.online:
        return Icons.circle;
      case PresenceState.stale:
        return Icons.bedtime_outlined;
      case PresenceState.offline:
        return Icons.circle_outlined;
    }
  }

  Color _presenceColour(PresenceState state, ThemeData theme) {
    switch (state) {
      case PresenceState.online:
        return theme.colorScheme.primary;
      case PresenceState.stale:
        return theme.colorScheme.tertiary;
      case PresenceState.offline:
        return theme.colorScheme.outline;
    }
  }
}
