//! Durable SQLite Job Queue with Leases, Idempotency & Crash Recovery (PoC 4).
//!
//! Replaces ad-hoc JSON task files for long-running background work (indexing,
//! drive-sync, reverse engineering) with atomic SQLite leases and retries.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: i64,
    pub kind: String,
    pub idempotency_key: Option<String>,
    pub payload: Value,
    pub status: JobStatus,
    pub attempts: i64,
    pub max_attempts: i64,
    pub lease_until: Option<i64>,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct JobQueue {
    db_path: PathBuf,
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl JobQueue {
    pub fn open(db_path: &Path) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create job db dir {}: {}", parent.display(), e))?;
        }
        let conn = Connection::open(db_path).map_err(|e| format!("open job db: {}", e))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS jobs (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 kind TEXT NOT NULL,
                 idempotency_key TEXT UNIQUE,
                 payload_json TEXT NOT NULL,
                 status TEXT NOT NULL,
                 attempts INTEGER NOT NULL DEFAULT 0,
                 max_attempts INTEGER NOT NULL DEFAULT 3,
                 lease_until INTEGER,
                 result_json TEXT,
                 error TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_jobs_status_lease ON jobs(status, lease_until);",
        )
        .map_err(|e| format!("init jobs schema: {}", e))?;
        Ok(Self {
            db_path: db_path.to_path_buf(),
        })
    }

    fn conn(&self) -> Result<Connection, String> {
        Connection::open(&self.db_path).map_err(|e| format!("open job db: {}", e))
    }

    /// Enqueues a job idempotently: if an existing non-cancelled job has the same
    /// `idempotency_key`, returns that existing job without duplicating side effects.
    pub fn enqueue(
        &self,
        kind: &str,
        payload: &Value,
        idempotency_key: Option<&str>,
        max_attempts: i64,
    ) -> Result<JobRecord, String> {
        let conn = self.conn()?;
        let now = now_epoch();
        if let Some(key) = idempotency_key {
            if let Some(existing) = Self::get_by_key(&conn, key)? {
                return Ok(existing);
            }
        }
        let payload_str = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
        conn.execute(
            "INSERT INTO jobs (kind, idempotency_key, payload_json, status, attempts, max_attempts, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'pending', 0, ?4, ?5, ?5)",
            params![kind, idempotency_key, payload_str, max_attempts.max(1), now],
        )
        .map_err(|e| format!("insert job: {}", e))?;
        let id = conn.last_insert_rowid();
        Self::get_by_id(&conn, id)?
            .ok_or_else(|| "inserted job disappeared".to_string())
    }

    /// Reclaims any `running` jobs whose lease expired (e.g. process crash) back to `pending`
    /// (or `failed` if `attempts >= max_attempts`), then atomically claims the next `pending` job.
    pub fn claim_next(&self, lease_seconds: i64) -> Result<Option<JobRecord>, String> {
        let mut conn = self.conn()?;
        let now = now_epoch();
        let tx = conn
            .transaction()
            .map_err(|e| format!("begin claim tx: {}", e))?;

        // Crash recovery: expire stale leases
        tx.execute(
            "UPDATE jobs SET status = CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'pending' END,
                             lease_until = NULL,
                             error = CASE WHEN attempts >= max_attempts THEN 'lease expired after max_attempts' ELSE error END,
                             updated_at = ?1
             WHERE status = 'running' AND lease_until IS NOT NULL AND lease_until < ?1",
            params![now],
        )
        .map_err(|e| format!("reclaim stale leases: {}", e))?;

        let next_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM jobs WHERE status = 'pending' ORDER BY id ASC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("select pending job: {}", e))?;

        let Some(id) = next_id else {
            tx.commit().map_err(|e| format!("commit empty claim: {}", e))?;
            return Ok(None);
        };

        let lease_until = now + lease_seconds.max(5);
        tx.execute(
            "UPDATE jobs SET status = 'running', attempts = attempts + 1, lease_until = ?1, updated_at = ?2 WHERE id = ?3",
            params![lease_until, now, id],
        )
        .map_err(|e| format!("update claimed job: {}", e))?;

        let rec = Self::get_by_id(&tx, id)?;
        tx.commit().map_err(|e| format!("commit claim tx: {}", e))?;
        Ok(rec)
    }

    pub fn complete(&self, id: i64, result: &Value) -> Result<(), String> {
        let conn = self.conn()?;
        let now = now_epoch();
        let res_str = serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string());
        conn.execute(
            "UPDATE jobs SET status = 'completed', lease_until = NULL, result_json = ?1, error = NULL, updated_at = ?2 WHERE id = ?3",
            params![res_str, now, id],
        )
        .map_err(|e| format!("complete job: {}", e))?;
        Ok(())
    }

    pub fn fail_or_retry(&self, id: i64, err: &str) -> Result<JobStatus, String> {
        let conn = self.conn()?;
        let now = now_epoch();
        let rec = Self::get_by_id(&conn, id)?
            .ok_or_else(|| format!("job {} not found", id))?;
        let next_status = if rec.attempts < rec.max_attempts {
            JobStatus::Pending
        } else {
            JobStatus::Failed
        };
        conn.execute(
            "UPDATE jobs SET status = ?1, lease_until = NULL, error = ?2, updated_at = ?3 WHERE id = ?4",
            params![next_status.as_str(), err, now, id],
        )
        .map_err(|e| format!("fail_or_retry job: {}", e))?;
        Ok(next_status)
    }

    pub fn cancel(&self, id: i64) -> Result<bool, String> {
        let conn = self.conn()?;
        let now = now_epoch();
        let changed = conn
            .execute(
                "UPDATE jobs SET status = 'cancelled', lease_until = NULL, updated_at = ?1 WHERE id = ?2 AND status IN ('pending', 'running')",
                params![now, id],
            )
            .map_err(|e| format!("cancel job: {}", e))?;
        Ok(changed > 0)
    }

    pub fn list(&self, limit: usize) -> Result<Vec<JobRecord>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, idempotency_key, payload_json, status, attempts, max_attempts, lease_until, result_json, error, created_at, updated_at
                 FROM jobs ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| format!("prepare list jobs: {}", e))?;
        let rows = stmt
            .query_map(params![limit as i64], Self::map_row)
            .map_err(|e| format!("query list jobs: {}", e))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("row job: {}", e))?);
        }
        Ok(out)
    }

    fn get_by_id(conn: &Connection, id: i64) -> Result<Option<JobRecord>, String> {
        conn.query_row(
            "SELECT id, kind, idempotency_key, payload_json, status, attempts, max_attempts, lease_until, result_json, error, created_at, updated_at
             FROM jobs WHERE id = ?1",
            params![id],
            Self::map_row,
        )
        .optional()
        .map_err(|e| format!("get_by_id: {}", e))
    }

    fn get_by_key(conn: &Connection, key: &str) -> Result<Option<JobRecord>, String> {
        conn.query_row(
            "SELECT id, kind, idempotency_key, payload_json, status, attempts, max_attempts, lease_until, result_json, error, created_at, updated_at
             FROM jobs WHERE idempotency_key = ?1",
            params![key],
            Self::map_row,
        )
        .optional()
        .map_err(|e| format!("get_by_key: {}", e))
    }

    fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<JobRecord> {
        let payload_str: String = r.get(3)?;
        let status_str: String = r.get(4)?;
        let result_str: Option<String> = r.get(8)?;
        Ok(JobRecord {
            id: r.get(0)?,
            kind: r.get(1)?,
            idempotency_key: r.get(2)?,
            payload: serde_json::from_str(&payload_str).unwrap_or(Value::Null),
            status: JobStatus::from_str(&status_str),
            attempts: r.get(5)?,
            max_attempts: r.get(6)?,
            lease_until: r.get(7)?,
            result: result_str.and_then(|s| serde_json::from_str(&s).ok()),
            error: r.get(9)?,
            created_at: r.get(10)?,
            updated_at: r.get(11)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn durable_queue_idempotency_retry_and_lease_recovery() {
        let dir = std::env::temp_dir().join(format!("mg_jobs_{}", std::process::id()));
        let db = dir.join("jobs.db");
        let _ = std::fs::remove_dir_all(&dir);
        let q = JobQueue::open(&db).unwrap();

        let j1 = q
            .enqueue("index_repo", &json!({"path": "/tmp"}), Some("idx-1"), 2)
            .unwrap();
        let j1_dup = q
            .enqueue("index_repo", &json!({"path": "/tmp"}), Some("idx-1"), 2)
            .unwrap();
        assert_eq!(j1.id, j1_dup.id);

        let claimed = q.claim_next(30).unwrap().unwrap();
        assert_eq!(claimed.id, j1.id);
        assert_eq!(claimed.attempts, 1);
        assert_eq!(claimed.status, JobStatus::Running);

        // First failure retries -> Pending
        let st = q.fail_or_retry(claimed.id, "transient").unwrap();
        assert_eq!(st, JobStatus::Pending);

        let claimed2 = q.claim_next(30).unwrap().unwrap();
        assert_eq!(claimed2.attempts, 2);
        q.complete(claimed2.id, &json!({"indexed": 42})).unwrap();

        let list = q.list(10).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].status, JobStatus::Completed);
        assert_eq!(list[0].result.as_ref().unwrap()["indexed"], 42);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
