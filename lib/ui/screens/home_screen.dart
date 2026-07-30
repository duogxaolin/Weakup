import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../application/providers.dart';
import '../../domain/job.dart';
import '../widgets/job_list_tile.dart';
import 'create_job_screen.dart';
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
