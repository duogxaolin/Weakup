use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, Row};

use crate::core::{AppError, AppResult};
use crate::data::command_decision::CommandDecisionRecord;
use crate::data::job_repository::JobRepository;
use crate::domain::{
    CommandAcceptance, DeviceId, Job, JobOrigin, JobStatus, JobType, RejectionReason,
    RemoteCommand, TriggerSpec,
};

/// Schema version currently written. Bumping this requires a migration arm in
/// [`SqliteJobRepository::migrate`].
const SCHEMA_VERSION: i64 = 4;

/// SQLite-backed [`JobRepository`].
///
/// One connection behind a `Mutex`. This is not a bottleneck worth avoiding: the
/// write rate is a handful of statements per user action, and a single connection
/// removes the "database is locked" failure mode that a pool would introduce for
/// no benefit at this scale.
pub struct SqliteJobRepository {
    conn: Mutex<Connection>,
}

impl SqliteJobRepository {
    /// Opens (creating if needed) the database at `path` and applies migrations.
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    /// In-memory database, for tests.
    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> AppResult<Self> {
        // Enforced per connection, not stored in the file, so it must be set here
        // rather than in the schema.
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let repo = Self {
            conn: Mutex::new(conn),
        };
        repo.migrate()?;
        Ok(repo)
    }

    fn migrate(&self) -> AppResult<()> {
        let conn = self.lock()?;
        let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if current < 1 {
            // The trigger definition is stored alongside `target_instant_utc`
            // (D8): the target is what fires, the definition is what lets an
            // absolute-time job be re-resolved after a timezone change.
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS jobs (
                     id                 TEXT    PRIMARY KEY NOT NULL,
                     job_type           TEXT    NOT NULL,
                     trigger_kind       TEXT    NOT NULL,
                     trigger_minutes    INTEGER,
                     trigger_hour       INTEGER,
                     trigger_minute     INTEGER,
                     status             TEXT    NOT NULL,
                     target_instant_utc INTEGER,
                     created_at_utc     INTEGER NOT NULL,
                     updated_at_utc     INTEGER NOT NULL,
                     timezone           TEXT    NOT NULL,
                     failure_message    TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs (status);
                 CREATE INDEX IF NOT EXISTS idx_jobs_type_status ON jobs (job_type, status);",
            )?;
        }

        if current < 2 {
            // Key-value rather than a column per setting: adding a setting should not
            // require a migration for what are a few small values read once at startup.
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS settings (
                     key   TEXT PRIMARY KEY NOT NULL,
                     value TEXT NOT NULL
                 );",
            )?;
        }

        if current < 3 {
            // Nullable, and null means "a time of day" — which is what every row
            // written before this column existed meant. So no backfill is needed and
            // existing jobs keep their behaviour exactly.
            conn.execute_batch("ALTER TABLE jobs ADD COLUMN trigger_date TEXT;")?;
        }

        if current < 4 {
            // Nullable, and null means "scheduled at this machine" — which is what every
            // row written before this column existed meant, so no backfill is needed and
            // existing jobs keep their behaviour exactly.
            //
            // Until this column existed the origin was hardcoded on read, so a remote job
            // that survived a restart came back as local and was handed the 60-second
            // countdown instead of 300. That is a safety countdown being silently
            // shortened, which is why the column is not optional.
            conn.execute_batch("ALTER TABLE jobs ADD COLUMN origin TEXT;")?;

            // A separate table rather than a column on `jobs`, because a refused command
            // creates no job and refusals are precisely what must be recorded: a series of
            // them is the visible signature of an attack. A record that existed only when
            // the command succeeded would be blind to the case it is most needed for.
            //
            // A table rather than a log file, because this must be readable at the target
            // with no network, and because a log rotated by size loses its oldest entries
            // first — the opposite of what attribution needs after an unexplained shutdown.
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS command_decisions (
                     id               INTEGER PRIMARY KEY AUTOINCREMENT,
                     sender_device_id TEXT    NOT NULL,
                     command          TEXT    NOT NULL,
                     decision         TEXT    NOT NULL,
                     rejection_reason TEXT,
                     decided_at_utc   INTEGER NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_command_decisions_decided_at
                     ON command_decisions (decided_at_utc);",
            )?;
        }

        // Not parameterisable — PRAGMA does not accept bound values.
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
        Ok(())
    }

    /// Runs a statement directly, for tests that need to create a state the public API
    /// cannot — chiefly a settings row written by a future version of the app.
    #[cfg(test)]
    pub(crate) fn execute_raw_for_test(&self, sql: &str) -> AppResult<()> {
        self.lock()?.execute_batch(sql)?;
        Ok(())
    }

    /// A poisoned mutex means another thread panicked mid-write. Surfaced as a
    /// storage error rather than propagating the panic.
    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::Storage { message: "database lock poisoned".into() })
    }
}

/// Timestamps are stored as milliseconds since the Unix epoch: an absolute
/// instant with no timezone or formatting ambiguity, and directly sortable in SQL.
fn to_millis(value: DateTime<Utc>) -> i64 {
    value.timestamp_millis()
}

fn from_millis(millis: i64) -> AppResult<DateTime<Utc>> {
    Utc.timestamp_millis_opt(millis).single().ok_or_else(|| AppError::Storage {
        message: format!("row holds an out-of-range timestamp: {millis}"),
    })
}

/// The columns every read selects, in the order [`row_to_job`] expects.
///
/// `trigger_date` and `origin` are appended at the end rather than placed beside the other
/// job fields, where each would read better. `row_to_job` and `row_to_trigger` address
/// columns by position, so inserting either mid-list would silently shift every subsequent
/// field by one — status reading a timestamp, and so on — and the compiler cannot catch it
/// because the indices are all integers. Appending leaves indices 0-12 exactly as they were
/// and makes `origin` index 13.
const JOB_COLUMNS: &str = "id, job_type, trigger_kind, trigger_minutes, trigger_hour, \
                           trigger_minute, status, target_instant_utc, created_at_utc, \
                           updated_at_utc, timezone, failure_message, trigger_date, origin";

fn row_to_job(row: &Row<'_>) -> AppResult<Job> {
    let job_type_raw: String = row.get(1)?;
    let job_type = JobType::from_str_value(&job_type_raw).ok_or_else(|| AppError::Storage {
        message: format!("row holds an unknown job type: {job_type_raw}"),
    })?;

    let trigger = row_to_trigger(row)?;

    let status_raw: String = row.get(6)?;
    let status = JobStatus::from_str_value(&status_raw).ok_or_else(|| AppError::Storage {
        message: format!("row holds an unknown job status: {status_raw}"),
    })?;

    let target_millis: Option<i64> = row.get(7)?;
    let target_instant_utc = target_millis.map(from_millis).transpose()?;

    // An indefinite trigger has no target and every other kind must have one.
    // Checked on read because a row violating it would otherwise become a job
    // that silently never fires.
    if trigger.has_target_instant() != target_instant_utc.is_some() {
        return Err(AppError::Storage {
            message: format!(
                "row is inconsistent: trigger {} with target_instant_utc {}",
                trigger.kind_str(),
                if target_instant_utc.is_some() { "present" } else { "absent" },
            ),
        });
    }

    // Index 13: appended, see JOB_COLUMNS. Null reads as `Local` — every row written
    // before this column existed was scheduled at the machine, so this is a faithful
    // reading of old data rather than a default standing in for missing information.
    let origin_raw: Option<String> = row.get(13)?;
    let origin = match origin_raw {
        None => JobOrigin::Local,
        Some(raw) => JobOrigin::from_str_value(&raw).ok_or_else(|| AppError::Storage {
            message: format!("row holds an unknown job origin: {raw}"),
        })?,
    };

    Ok(Job {
        id: row.get(0)?,
        job_type,
        trigger,
        status,
        target_instant_utc,
        created_at_utc: from_millis(row.get(8)?)?,
        updated_at_utc: from_millis(row.get(9)?)?,
        timezone: row.get(10)?,
        failure_message: row.get(11)?,
        origin,
    })
}

fn row_to_trigger(row: &Row<'_>) -> AppResult<TriggerSpec> {
    let kind: String = row.get(2)?;
    match kind.as_str() {
        "indefinite" => Ok(TriggerSpec::Indefinite),
        "duration" => {
            let minutes: Option<i64> = row.get(3)?;
            Ok(TriggerSpec::Duration {
                minutes: minutes.ok_or_else(|| AppError::Storage {
                    message: "duration trigger row has no trigger_minutes".into(),
                })?,
            })
        }
        "absoluteTime" => {
            let hour: Option<u32> = row.get(4)?;
            let minute: Option<u32> = row.get(5)?;
            // Index 12: appended, see JOB_COLUMNS. Null means an undated trigger,
            // which is what every pre-migration row is.
            let date_raw: Option<String> = row.get(12)?;
            let date = date_raw
                .map(|raw| {
                    NaiveDate::parse_from_str(&raw, "%Y-%m-%d").map_err(|_| AppError::Storage {
                        message: format!("row holds an unparseable trigger_date: {raw}"),
                    })
                })
                .transpose()?;

            match (hour, minute) {
                (Some(hour), Some(minute)) => Ok(TriggerSpec::AbsoluteTime { hour, minute, date }),
                _ => Err(AppError::Storage {
                    message: "absoluteTime trigger row is missing hour or minute".into(),
                }),
            }
        }
        other => Err(AppError::Storage {
            message: format!("row holds an unknown trigger kind: {other}"),
        }),
    }
}

/// The trigger's column values, in the order the INSERT binds them.
type TriggerColumns = (
    &'static str,
    Option<i64>,
    Option<u32>,
    Option<u32>,
    Option<String>,
);

fn trigger_columns(trigger: &TriggerSpec) -> TriggerColumns {
    match *trigger {
        TriggerSpec::Indefinite => ("indefinite", None, None, None, None),
        TriggerSpec::Duration { minutes } => ("duration", Some(minutes), None, None, None),
        TriggerSpec::AbsoluteTime { hour, minute, date } => (
            "absoluteTime",
            None,
            Some(hour),
            Some(minute),
            // `%Y-%m-%d`, matching the JSON wire format and what `row_to_trigger`
            // parses back.
            date.map(|date| date.format("%Y-%m-%d").to_string()),
        ),
    }
}

/// Shared by `insert` and the insert half of `replace_active_job_of_type`, so the
/// two cannot drift into writing different column sets.
fn insert_job(conn: &Connection, job: &Job) -> AppResult<()> {
    let (kind, minutes, hour, minute, date) = trigger_columns(&job.trigger);
    conn.execute(
        "INSERT INTO jobs (
             id, job_type, trigger_kind, trigger_minutes, trigger_hour, trigger_minute,
             status, target_instant_utc, created_at_utc, updated_at_utc, timezone,
             failure_message, trigger_date, origin
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![
            job.id,
            job.job_type.as_str(),
            kind,
            minutes,
            hour,
            minute,
            job.status.as_str(),
            job.target_instant_utc.map(to_millis),
            to_millis(job.created_at_utc),
            to_millis(job.updated_at_utc),
            job.timezone,
            job.failure_message,
            date,
            // Written explicitly rather than left null so a remote job reads back as
            // remote. A null would be read as local, which for a remote job means the
            // 60-second countdown instead of 300.
            job.origin.as_str(),
        ],
    )?;
    Ok(())
}

impl JobRepository for SqliteJobRepository {
    fn list_all(&self) -> AppResult<Vec<Job>> {
        let conn = self.lock()?;
        let sql = format!("SELECT {JOB_COLUMNS} FROM jobs ORDER BY updated_at_utc DESC");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Ok(row_to_job(row)))?;

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row??);
        }
        Ok(jobs)
    }

    fn list_occupying_active_slot(&self) -> AppResult<Vec<Job>> {
        let conn = self.lock()?;
        let sql = format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE status IN (?1, ?2) ORDER BY updated_at_utc DESC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            [JobStatus::Active.as_str(), JobStatus::Paused.as_str()],
            |row| Ok(row_to_job(row)),
        )?;

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row??);
        }
        Ok(jobs)
    }

    fn find(&self, id: &str) -> AppResult<Option<Job>> {
        let conn = self.lock()?;
        let sql = format!("SELECT {JOB_COLUMNS} FROM jobs WHERE id = ?1");
        conn.query_row(&sql, [id], |row| Ok(row_to_job(row)))
            .optional()?
            .transpose()
    }

    fn insert(&self, job: &Job) -> AppResult<()> {
        let conn = self.lock()?;
        insert_job(&conn, job)
    }

    fn update_status(
        &self,
        id: &str,
        status: JobStatus,
        failure_message: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.lock()?;
        let affected = conn.execute(
            "UPDATE jobs SET status = ?1, failure_message = ?2, updated_at_utc = ?3 WHERE id = ?4",
            rusqlite::params![
                status.as_str(),
                failure_message,
                to_millis(Utc::now()),
                id
            ],
        )?;

        // Silence here would mean the scheduler believes it recorded a state
        // change that never happened — including a power-off that failed.
        if affected == 0 {
            return Err(AppError::Storage {
                message: format!("no job with id {id} to update"),
            });
        }
        Ok(())
    }

    fn update_target(&self, id: &str, target_instant_utc: DateTime<Utc>) -> AppResult<()> {
        let conn = self.lock()?;
        let affected = conn.execute(
            "UPDATE jobs SET target_instant_utc = ?1, updated_at_utc = ?2 WHERE id = ?3",
            rusqlite::params![to_millis(target_instant_utc), to_millis(Utc::now()), id],
        )?;
        if affected == 0 {
            return Err(AppError::Storage {
                message: format!("no job with id {id} to retarget"),
            });
        }
        Ok(())
    }

    fn delete(&self, id: &str) -> AppResult<()> {
        let conn = self.lock()?;
        let affected = conn.execute("DELETE FROM jobs WHERE id = ?1", [id])?;
        if affected == 0 {
            return Err(AppError::Storage {
                message: format!("no job with id {id} to delete"),
            });
        }
        Ok(())
    }

    fn replace_active_job_of_type(&self, job_type: JobType, new_job: &Job) -> AppResult<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;

        // Cancel whatever holds the slot. `Paused` counts: a paused job is still
        // retained and resumable, so it must be displaced too.
        tx.execute(
            "UPDATE jobs SET status = ?1, updated_at_utc = ?2
             WHERE job_type = ?3 AND status IN (?4, ?5)",
            rusqlite::params![
                JobStatus::Cancelled.as_str(),
                to_millis(Utc::now()),
                job_type.as_str(),
                JobStatus::Active.as_str(),
                JobStatus::Paused.as_str(),
            ],
        )?;

        insert_job(&tx, new_job)?;

        // Either both halves land or neither does; a partial apply would leave
        // the type with two active jobs or none.
        tx.commit()?;
        Ok(())
    }
}

/// Command decisions share the jobs connection, for the same reason settings do: two
/// connections to one SQLite file is how "database is locked" happens.
impl crate::data::job_repository::CommandDecisionLog for SqliteJobRepository {
    fn append_command_decision(&self, record: &CommandDecisionRecord) -> AppResult<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO command_decisions (
                 sender_device_id, command, decision, rejection_reason, decided_at_utc
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                record.sender.as_str(),
                record.command.as_str(),
                record.decision_str(),
                // Null means accepted. The decision column already says which, so this
                // carries no information of its own when the command was accepted.
                record.rejection_reason().map(RejectionReason::as_str),
                to_millis(record.decided_at_utc),
            ],
        )?;
        Ok(())
    }

    fn recent_command_decisions(&self, limit: u32) -> AppResult<Vec<CommandDecisionRecord>> {
        let conn = self.lock()?;
        // Ordered by id alongside the instant so two decisions reached in the same
        // millisecond still come back in the order they were written.
        let mut stmt = conn.prepare(
            "SELECT sender_device_id, command, decision, rejection_reason, decided_at_utc
             FROM command_decisions
             ORDER BY decided_at_utc DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| Ok(row_to_command_decision(row)))?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row??);
        }
        Ok(records)
    }
}

fn row_to_command_decision(row: &Row<'_>) -> AppResult<CommandDecisionRecord> {
    let sender_raw: String = row.get(0)?;
    let sender = DeviceId::new(sender_raw.clone()).map_err(|_| AppError::Storage {
        message: format!("row holds an unusable sender device id: {sender_raw}"),
    })?;

    let command_raw: String = row.get(1)?;
    let command = RemoteCommand::from_str_value(&command_raw).ok_or_else(|| {
        AppError::Storage {
            message: format!("row holds an unknown remote command: {command_raw}"),
        }
    })?;

    let decision_raw: String = row.get(2)?;
    let reason_raw: Option<String> = row.get(3)?;

    // The pairing of decision and reason is validated on read rather than trusted: a
    // rejection with no reason, or an acceptance carrying one, would mean the record can no
    // longer explain what happened — which is the only thing it exists to do.
    let acceptance = match (decision_raw.as_str(), reason_raw) {
        ("accepted", None) => CommandAcceptance::Accepted,
        ("rejected", Some(raw)) => {
            let reason = RejectionReason::from_str_value(&raw).ok_or_else(|| {
                AppError::Storage {
                    message: format!("row holds an unknown rejection reason: {raw}"),
                }
            })?;
            CommandAcceptance::Rejected(reason)
        }
        ("accepted", Some(raw)) => {
            return Err(AppError::Storage {
                message: format!("row is accepted but names a rejection reason: {raw}"),
            })
        }
        ("rejected", None) => {
            return Err(AppError::Storage {
                message: "row is rejected but names no reason".into(),
            })
        }
        (other, _) => {
            return Err(AppError::Storage {
                message: format!("row holds an unknown command decision: {other}"),
            })
        }
    };

    Ok(CommandDecisionRecord {
        sender,
        command,
        acceptance,
        decided_at_utc: from_millis(row.get(4)?)?,
    })
}

/// Settings share the jobs connection rather than opening a second one.
///
/// Two connections to the same SQLite file is how "database is locked" happens, and
/// there is nothing to gain here: settings are read once at startup and written when
/// the user changes one.
impl crate::data::settings::SettingsStore for SqliteJobRepository {
    fn load(&self) -> AppResult<crate::data::settings::Settings> {
        use crate::data::settings::Settings;

        let conn = self.lock()?;
        let mut settings = Settings::default();

        let mut statement = conn.prepare("SELECT key, value FROM settings")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for row in rows {
            let (key, value) = row?;
            match key.as_str() {
                KEY_TIMEZONE => settings.timezone = value,
                KEY_NOTIFICATIONS => settings.notifications_enabled = value == "1",
                KEY_THEME => {
                    settings.theme = crate::data::settings::Theme::from_stored(&value)
                }
                KEY_LANGUAGE => {
                    settings.language = crate::data::settings::Language::from_stored(&value)
                }
                // An unknown key is a setting from a newer version. Ignored rather
                // than treated as corruption, so downgrading does not wipe settings.
                other => log::debug!("ignoring unknown setting: {other}"),
            }
        }

        Ok(settings)
    }

    fn save(&self, settings: &crate::data::settings::Settings) -> AppResult<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;

        for (key, value) in [
            (KEY_TIMEZONE, settings.timezone.clone()),
            (
                KEY_NOTIFICATIONS,
                if settings.notifications_enabled { "1" } else { "0" }.to_string(),
            ),
            (KEY_THEME, settings.theme.as_str().to_string()),
            (KEY_LANGUAGE, settings.language.as_str().to_string()),
        ] {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value],
            )?;
        }

        tx.commit()?;
        Ok(())
    }
}

const KEY_TIMEZONE: &str = "timezone";
const KEY_NOTIFICATIONS: &str = "notifications_enabled";
const KEY_THEME: &str = "theme";
const KEY_LANGUAGE: &str = "language";
