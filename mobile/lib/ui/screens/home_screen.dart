import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../application/providers.dart';
import '../../domain/device_id.dart';
import '../../domain/job.dart';
import '../widgets/job_list_tile.dart';
import 'create_job_screen.dart';
import 'paired_devices_screen.dart';
import 'pairing_screen.dart';
import 'settings_screen.dart';

/// Main screen showing the active job list.
class HomeScreen extends ConsumerWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final jobListAsync = ref.watch(jobListProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Weakup'),
        actions: [
          Semantics(
            label: 'Open paired devices',
            child: IconButton(
              icon: const Icon(Icons.devices_other),
              tooltip: 'Paired devices',
              onPressed: () => _openPairedDevices(context, ref),
            ),
          ),
          Semantics(
            label: 'Open settings',
            child: IconButton(
              icon: const Icon(Icons.settings),
              tooltip: 'Settings',
              onPressed: () {
                Navigator.push(
                  context,
                  MaterialPageRoute(builder: (_) => const SettingsScreen()),
                );
              },
            ),
          ),
        ],
      ),
      body: jobListAsync.when(
        loading: () => const Center(
          child: CircularProgressIndicator(),
        ),
        error: (error, _) => Center(
          child: Text(
            'Error loading jobs: $error',
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        ),
        data: (jobs) {
          if (jobs.isEmpty) {
            return const _EmptyJobList();
          }
          return _JobList(jobs: jobs);
        },
      ),
      floatingActionButton: Semantics(
        label: 'Create new job',
        child: FloatingActionButton.extended(
          onPressed: () => _openCreateJobScreen(context),
          label: const Text('New Job'),
          icon: const Icon(Icons.add),
          tooltip: 'Create a new keep-awake or power-off job',
        ),
      ),
    );
  }

  void _openCreateJobScreen(BuildContext context) {
    Navigator.push(
      context,
      MaterialPageRoute(builder: (_) => const CreateJobScreen()),
    );
  }

  /// Opens the paired-devices list.
  ///
  /// The callbacks are supplied here rather than reached for inside the screen, which is what
  /// lets the screen be pumped in a test with no database, keychain, or relay.
  ///
  /// `onSendCommand` reports that sending is unavailable rather than pretending. This build has
  /// no configured transport — obtaining an access token needs the OAuth exchange the
  /// repository cannot complete — and a button that silently did nothing would be worse than
  /// one that says why. Revoking and listing work regardless, because neither needs a network:
  /// a device must be de-authorizable when nothing is reachable, which is exactly when someone
  /// is most likely to be doing it.
  void _openPairedDevices(BuildContext context, WidgetRef ref) {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => Consumer(
          builder: (context, ref, _) {
            final peersAsync = ref.watch(pairedPeersProvider);

            return peersAsync.when(
              loading: () => const Scaffold(
                body: Center(child: CircularProgressIndicator()),
              ),
              error: (error, _) => Scaffold(
                appBar: AppBar(title: const Text('Paired devices')),
                body: Center(child: Text('Could not read pairings: $error')),
              ),
              data: (peers) => PairedDevicesScreen(
                peers: peers,
                now: DateTime.now().toUtc(),
                onSendCommand: (deviceId, command) async {
                  return 'This device cannot reach the relay yet, so the request was not '
                      'sent. Pairing still works.';
                },
                onRevoke: (deviceId) async {
                  final store = ref.read(pairingStoreProvider);
                  await store.revokePairing(
                    peer: DeviceId(deviceId),
                    revokedAt: DateTime.now().toUtc(),
                  );
                  ref.invalidate(pairedPeersProvider);
                },
                onPairNew: () => _openPairing(context, ref),
              ),
            );
          },
        ),
      ),
    );
  }

  /// Opens the code-entry screen.
  ///
  /// The submit callback runs the real pairing flow: `acceptPairingAtIssuer` then
  /// `completePairingAtRequester`, in that order, with a compensating withdrawal if this
  /// side's write fails. Design D7 — a half-recorded pairing is the worst outcome available.
  void _openPairing(BuildContext context, WidgetRef ref) {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => PairingScreen(
          onSubmit: ({
            required String code,
            required String peerDeviceId,
            required String peerVerifyingKey,
          }) async {
            final outcome = await runPairingExchange(
              ref: ref,
              enteredCode: code,
              peerDeviceId: peerDeviceId,
              peerVerifyingKeyHex: peerVerifyingKey,
            );
            ref.invalidate(pairedPeersProvider);
            return outcome;
          },
        ),
      ),
    );
  }
}

class _EmptyJobList extends StatelessWidget {
  const _EmptyJobList();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          const Icon(Icons.schedule, size: 64, color: Colors.grey),
          const SizedBox(height: 16),
          Text(
            'No jobs scheduled',
            style: Theme.of(context).textTheme.titleLarge?.copyWith(
                  color: Colors.grey[600],
                ),
          ),
          const SizedBox(height: 8),
          Text(
            'Tap the + button to create a keep-awake or power-off job.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: Colors.grey[500],
                ),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }
}

class _JobList extends StatelessWidget {
  const _JobList({required this.jobs});
  final List<Job> jobs;

  @override
  Widget build(BuildContext context) {
    return ListView.separated(
      padding: const EdgeInsets.all(16),
      itemCount: jobs.length,
      separatorBuilder: (_, _) => const Divider(),
      itemBuilder: (context, index) {
        return JobListTile(job: jobs[index]);
      },
    );
  }
}
