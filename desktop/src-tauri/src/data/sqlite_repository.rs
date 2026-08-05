use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, Row};

use crate::core::{AppError, AppResult};
use crate::data::job_repository::JobRepository;
use crate::domain::{Job, JobStatus, JobType, TriggerSpec};

/// Schema version currently written. Bumping this requires a migration arm in
/// [`SqliteJobRepository::migrate`].
const SCHEMA_VERSION: i64 = 3;

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
/// `trigger_date` is appended at the end rather than placed beside the other trigger
/// columns, where it would read better. `row_to_job` and `row_to_trigger` address
/// columns by position, so inserting it at index 6 would silently shift every
/// subsequent field by one — status reading a timestamp, and so on. Appending leaves
/// indices 0-11 exactly as they were.
const JOB_COLUMNS: &str = "id, job_type, trigger_kind, trigger_minutes, trigger_hour, \
                           trigger_minute, status, target_instant_utc, created_at_utc, \
                           updated_at_utc, timezone, failure_message, trigger_date";

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
             failure_message, trigger_date
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
