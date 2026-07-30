/// The two job types the scheduler supports.
enum JobType {
  keepAwake,
  powerOff,
}

/// All possible states a job can be in.
enum JobStatus {
  /// Job is scheduled and actively running.
  active,

  /// Job has been paused by the user; resources released but job retained.
  paused,

  /// Job completed normally (trigger condition met).
  completed,

  /// Job was cancelled by the user.
  cancelled,

  /// Power-off executor returned an error, or the scheduler encountered a fatal issue.
  failed,

  /// Power-off target passed by more than 15 minutes; execution was skipped.
  overdue,

  /// Android 15: foreground service hit the 6-hour dataSync cap while job was pending.
  degraded,
}
