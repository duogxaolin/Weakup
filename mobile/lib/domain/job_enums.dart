/// The two job types the scheduler supports.
enum JobType {
  keepAwake,
  powerOff,
}

/// Where the request that created a job came from.
///
/// Mirrors `JobOrigin` in `desktop/src-tauri/src/domain/job_enums.rs`. Not persisted in
/// this change, deliberately: nothing creates a [JobOrigin.remote] job yet, so a column
/// would be written by nothing and read by nothing. [JobOrigin.local] is the default,
/// which is what every existing construction site already means.
///
/// The gap that leaves is worth stating plainly. When persistence lands, a remote job
/// that survives a restart will read back as local and get the shorter countdown. That
/// does not skip a countdown, but it does shorten one — so persisting this field is a
/// blocker for the change that adds a remote transport, not a follow-up to it.
enum JobOrigin {
  /// Scheduled by someone at this device. The person who asked is expected to be
  /// looking at it.
  local,

  /// Created by an authorized remote command. Nobody at the device asked for this,
  /// which is why the countdown it gets is longer.
  remote,
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
