//! SQLite run history (`history.db`): every executed run is recorded
//! so `fg stats` can answer "what do I actually use, and how much
//! typing has it saved me?".
//!
//! A run row is inserted when execution starts and finished when it
//! ends. A row that never finishes is an interrupted run (the CLI was
//! killed, e.g. Ctrl-C): it is counted separately and excluded from
//! every total, per spec-cli §4 (`stats`).

use anyhow::{Context, Result};
#[cfg(test)]
use rusqlite::OptionalExtension;
use rusqlite::{params, Connection};
use std::path::Path;

/// Aggregates over finished runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Totals {
    /// Finished runs.
    pub runs: u32,
    /// Commands across finished runs.
    pub commands: u64,
    /// Wall time of finished runs, milliseconds.
    pub total_ms: u64,
    /// Finished runs whose exit code was non-zero.
    pub failures: u32,
    /// Distinct snippet names with at least one finished run.
    pub workflows: u32,
    /// Runs that started but never finished (excluded everywhere else).
    pub interrupted: u32,
}

/// Per-snippet aggregates over finished runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowStat {
    /// Snippet name.
    pub workflow: String,
    /// Finished runs.
    pub runs: u32,
    /// Commands across those runs.
    pub commands: u64,
    /// Total wall time, milliseconds.
    pub total_ms: u64,
    /// Failed runs.
    pub failures: u32,
    /// `started_at` of the most recent finished run (SQLite datetime).
    pub last_run: Option<String>,
}

/// The history database handle.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (creating if needed) the history DB at `path`.
    ///
    /// # Errors
    /// On SQLite failures.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening database {}", path.display()))?;
        Self::init(conn)
    }

    /// An in-memory DB (tests).
    ///
    /// # Errors
    /// On SQLite failures.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        conn.execute_batch(SCHEMA).context("initializing schema")?;
        Ok(Self { conn })
    }

    /// Start a run row; returns its id for [`Db::finish_run`].
    ///
    /// # Errors
    /// On SQLite failures.
    pub fn start_run(&self, workflow: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO runs (workflow, started_at, success, duration_ms, command_count)
             VALUES (?1, datetime('now'), 0, 0, 0)",
            params![workflow],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Mark a run finished with its outcome.
    ///
    /// # Errors
    /// On SQLite failures.
    pub fn finish_run(
        &self,
        run_id: i64,
        success: bool,
        duration_ms: u128,
        command_count: u32,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET success = ?2, duration_ms = ?3, command_count = ?4,
                    finished_at = datetime('now')
             WHERE id = ?1",
            params![
                run_id,
                success,
                i64::try_from(duration_ms).unwrap_or(i64::MAX),
                command_count
            ],
        )?;
        Ok(())
    }

    /// Totals over finished runs; unfinished runs count only as
    /// `interrupted`.
    ///
    /// # Errors
    /// On SQLite failures.
    pub fn totals(&self) -> Result<Totals> {
        let (runs, commands, total_ms, failures): (u32, i64, i64, u32) = self.conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(command_count), 0),
                    COALESCE(SUM(duration_ms), 0),
                    COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), 0)
             FROM runs WHERE finished_at IS NOT NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let workflows: u32 = self.conn.query_row(
            "SELECT COUNT(DISTINCT workflow) FROM runs WHERE finished_at IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        let interrupted: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM runs WHERE finished_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(Totals {
            runs,
            commands: u64::try_from(commands.max(0)).unwrap_or(0),
            total_ms: u64::try_from(total_ms.max(0)).unwrap_or(0),
            failures,
            workflows,
            interrupted,
        })
    }

    /// Per-snippet stats, most-run first.
    ///
    /// # Errors
    /// On SQLite failures.
    pub fn top_workflows(&self, limit: u32) -> Result<Vec<WorkflowStat>> {
        let mut stmt = self.conn.prepare(
            "SELECT workflow,
                    COUNT(*) AS runs,
                    COALESCE(SUM(command_count), 0),
                    COALESCE(SUM(duration_ms), 0),
                    COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), 0),
                    MAX(started_at)
             FROM runs
             WHERE finished_at IS NOT NULL
             GROUP BY workflow
             ORDER BY runs DESC, workflow ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(WorkflowStat {
                workflow: r.get(0)?,
                runs: r.get(1)?,
                commands: u64::try_from(r.get::<_, i64>(2)?.max(0)).unwrap_or(0),
                total_ms: u64::try_from(r.get::<_, i64>(3)?.max(0)).unwrap_or(0),
                failures: r.get(4)?,
                last_run: r.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("reading workflow stats")
    }

    /// Stats for one snippet, or `None` when it has no finished runs.
    /// (Used by the test-suite today; `stats` reads aggregates.)
    ///
    /// # Errors
    /// On SQLite failures.
    #[cfg(test)]
    pub fn stat_for(&self, workflow: &str) -> Result<Option<WorkflowStat>> {
        let row = self
            .conn
            .query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(command_count), 0),
                        COALESCE(SUM(duration_ms), 0),
                        COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), 0),
                        MAX(started_at)
                 FROM runs WHERE workflow = ?1 AND finished_at IS NOT NULL",
                params![workflow],
                |r| {
                    Ok(WorkflowStat {
                        workflow: workflow.to_string(),
                        runs: r.get(0)?,
                        commands: u64::try_from(r.get::<_, i64>(1)?.max(0)).unwrap_or(0),
                        total_ms: u64::try_from(r.get::<_, i64>(2)?.max(0)).unwrap_or(0),
                        failures: r.get(3)?,
                        last_run: r.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(row.filter(|s| s.runs > 0))
    }

    /// Estimated human time avoided, in milliseconds: each automated
    /// command is credited with `overhead_secs` of recall-and-typing
    /// time; the runs' own wall time is subtracted since you still
    /// wait for it. Never negative. Interrupted runs are excluded
    /// (they are excluded from the totals this reads).
    ///
    /// # Errors
    /// On SQLite failures.
    pub fn saved_ms(&self, overhead_secs: f64) -> Result<u128> {
        let t = self.totals()?;
        #[allow(clippy::cast_precision_loss)]
        let manual = t.commands as f64 * overhead_secs * 1000.0;
        #[allow(clippy::cast_precision_loss)]
        let net = manual - t.total_ms as f64;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(net.max(0.0) as u128)
    }
}

const SCHEMA: &str = r"
CREATE TABLE IF NOT EXISTS runs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow      TEXT    NOT NULL,
    started_at    TEXT    NOT NULL,
    finished_at   TEXT,
    success       INTEGER NOT NULL DEFAULT 0,
    duration_ms   INTEGER NOT NULL DEFAULT 0,
    command_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_runs_workflow ON runs(workflow);
CREATE INDEX IF NOT EXISTS idx_runs_started  ON runs(started_at);
";

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(db: &Db, wf: &str, cmds: u32, ms: u128, ok: bool) {
        let id = db.start_run(wf).unwrap();
        db.finish_run(id, ok, ms, cmds).unwrap();
    }

    #[test]
    fn counts_runs_and_totals() {
        let db = Db::open_in_memory().unwrap();
        finished(&db, "setup", 4, 1200, true);
        finished(&db, "setup", 4, 1100, true);
        finished(&db, "deploy", 2, 5000, false);
        let t = db.totals().unwrap();
        assert_eq!(t.runs, 3);
        assert_eq!(t.commands, 10);
        assert_eq!(t.failures, 1);
        assert_eq!(t.workflows, 2);
        assert_eq!(t.interrupted, 0);
    }

    #[test]
    fn unfinished_runs_are_interrupted_and_excluded() {
        let db = Db::open_in_memory().unwrap();
        finished(&db, "setup", 3, 100, true);
        db.start_run("interrupted").unwrap(); // never finished
        let t = db.totals().unwrap();
        assert_eq!(t.runs, 1);
        assert_eq!(t.interrupted, 1);
        assert!(db
            .top_workflows(10)
            .unwrap()
            .iter()
            .all(|s| s.workflow != "interrupted"));
    }

    #[test]
    fn ranks_top_workflows() {
        let db = Db::open_in_memory().unwrap();
        for _ in 0..5 {
            finished(&db, "setup", 4, 100, true);
        }
        for _ in 0..2 {
            finished(&db, "deploy", 3, 100, true);
        }
        let top = db.top_workflows(10).unwrap();
        assert_eq!(top[0].workflow, "setup");
        assert_eq!(top[0].runs, 5);
        assert_eq!(top[0].commands, 20);
        assert_eq!(top[1].workflow, "deploy");
    }

    #[test]
    fn saved_time_nets_out_actual_runtime() {
        let db = Db::open_in_memory().unwrap();
        finished(&db, "a", 10, 5_000, true); // 10 commands, took 5 s
                                             // 10 commands x 12 s = 120 s of typing, minus 5 s actually spent.
        assert_eq!(db.saved_ms(12.0).unwrap(), 115_000);
    }

    #[test]
    fn saved_time_never_goes_negative() {
        let db = Db::open_in_memory().unwrap();
        finished(&db, "slow", 1, 600_000, true); // 10-minute build
        assert_eq!(db.saved_ms(12.0).unwrap(), 0);
    }

    #[test]
    fn stat_for_unknown_workflow_is_none() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.stat_for("nope").unwrap().is_none());
        finished(&db, "setup", 3, 100, false);
        let s = db.stat_for("setup").unwrap().unwrap();
        assert_eq!(s.failures, 1);
    }
}
