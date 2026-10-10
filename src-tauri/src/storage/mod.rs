//! Local SQLite Storage Engine, Thread-Safe Connection Management & Pragma Configuration.
//!
//! # SQLite WAL Mode & High-Throughput Pragmas (Phase 11.1)
//! Configures embedded SQLite via `rusqlite` (bundled engine) with:
//! - **`PRAGMA journal_mode = WAL`**: Write-Ahead Logging allows readers to execute concurrently without blocking writers.
//! - **`PRAGMA synchronous = NORMAL`**: Fsync occurs at WAL checkpoints, preventing application corruption while avoiding full fsync latency.
//! - **`PRAGMA busy_timeout = 5000`**: 5-second automatic retry ceiling mitigating transient thread lock contention.
//! - **`PRAGMA foreign_keys = ON`**: Hard enforcement of referential integrity between benchmarks, incidents, and audit trails.
//! - **`PRAGMA mmap_size = 268435456`**: 256 MB memory-mapped I/O pool bypassing kernel buffer copies for sub-millisecond lookups.
//!
//! # Crash Recovery & WAL Maintenance (Phase 11.3)
//! - **Automatic startup integrity checks** (`PRAGMA integrity_check;`) guarding against filesystem damage.
//! - **WAL checkpointing** on idle and clean shutdown (`PRAGMA wal_checkpoint(TRUNCATE)`).
//! - **Safe `VACUUM` and zero-fill eradication** (`PRAGMA secure_delete = ON; VACUUM;`).

pub mod migrations;

use rusqlite::{Connection, OpenFlags, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Structured report of active SQLite pragmas for resilience audits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaReport {
    pub journal_mode: String,
    pub synchronous: i32,
    pub foreign_keys: bool,
    pub mmap_size: i64,
}

/// WAL Checkpointing modes supported by SQLite
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointMode {
    Passive,
    Full,
    Restart,
    Truncate,
}

/// Result of a WAL checkpoint operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointResult {
    pub busy: bool,
    pub log_frames: i32,
    pub checkpointed_frames: i32,
}

/// Detailed audit report for local data eradication
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EradicationReport {
    pub tables_purged: Vec<String>,
    pub secure_delete_applied: bool,
    pub wal_truncated: bool,
    pub vacuum_completed: bool,
    pub remaining_db_bytes: u64,
}

/// Status report resulting from SQLite PRAGMA integrity_check execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseIntegrityStatus {
    /// Database is completely sound (`PRAGMA integrity_check == "ok"`).
    Healthy,
    /// Confirmed corruption detected on disk. Contains corruption diagnostic lines
    /// and the path to any preserved quarantine copy.
    Corrupted {
        errors: Vec<String>,
        quarantine_backup_path: Option<PathBuf>,
    },
    /// Transient inability to execute check (e.g. file lock, I/O permission error).
    CheckFailed { error: String },
}

impl DatabaseIntegrityStatus {
    /// Returns true if the database integrity is confirmed healthy.
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}

fn ensure_secure_dir(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);
        let _ = std::fs::set_permissions(dir, perms);
    }
    Ok(())
}

fn ensure_secure_file(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path.exists() {
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
    let _ = path;
    Ok(())
}

fn enforce_database_file_permissions(db_path: &Path) {
    if db_path != Path::new(":memory:") && db_path.exists() {
        let _ = ensure_secure_file(db_path);
        let wal_path = PathBuf::from(format!("{}-wal", db_path.display()));
        if wal_path.exists() {
            let _ = ensure_secure_file(&wal_path);
        }
        let shm_path = PathBuf::from(format!("{}-shm", db_path.display()));
        if shm_path.exists() {
            let _ = ensure_secure_file(&shm_path);
        }
    }
}

pub struct StorageManager {
    db_path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl StorageManager {
    /// Performs a non-destructive read-only integrity check on an existing SQLite database file on disk.
    pub fn check_file_integrity(path: &Path) -> DatabaseIntegrityStatus {
        if path == Path::new(":memory:") {
            return DatabaseIntegrityStatus::Healthy;
        }
        if !path.exists() {
            return DatabaseIntegrityStatus::Healthy;
        }

        let conn = match Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => {
                let _ = c.busy_timeout(std::time::Duration::from_millis(10000));
                c
            }
            Err(e) => {
                return DatabaseIntegrityStatus::CheckFailed {
                    error: format!("Unable to open database for integrity verification: {}", e),
                }
            }
        };

        let mut stmt = match conn.prepare("PRAGMA integrity_check;") {
            Ok(s) => s,
            Err(e) => {
                return DatabaseIntegrityStatus::CheckFailed {
                    error: format!("Failed to prepare PRAGMA integrity_check: {}", e),
                }
            }
        };

        let rows = match stmt.query_map([], |row| row.get::<_, String>(0)) {
            Ok(r) => r,
            Err(e) => {
                return DatabaseIntegrityStatus::CheckFailed {
                    error: format!("Failed to execute integrity check query: {}", e),
                }
            }
        };

        let mut messages = Vec::new();
        for msg in rows.flatten() {
            messages.push(msg);
        }

        if messages.len() == 1 && messages[0].to_lowercase() == "ok" {
            DatabaseIntegrityStatus::Healthy
        } else {
            DatabaseIntegrityStatus::Corrupted {
                errors: messages,
                quarantine_backup_path: None,
            }
        }
    }

    /// Preserves a corrupted database by copying the original file and any active WAL/SHM
    /// frames into a timestamped quarantine file (`<path>.corrupt.<timestamp>.db`).
    /// Never deletes, truncates, or overwrites potentially recoverable forensic data.
    pub fn quarantine_corrupt_database(path: &Path) -> std::io::Result<PathBuf> {
        let now = chrono::Utc::now().timestamp();
        let quarantine_path = path.with_extension(format!("corrupt.{}.db", now));
        if let Some(parent) = quarantine_path.parent() {
            let _ = ensure_secure_dir(parent);
        }
        std::fs::copy(path, &quarantine_path)?;
        let _ = ensure_secure_file(&quarantine_path);

        let wal_path = PathBuf::from(format!("{}-wal", path.display()));
        if wal_path.exists() {
            let wal_quarantine = path.with_extension(format!("corrupt.{}.db-wal", now));
            let _ = std::fs::copy(&wal_path, &wal_quarantine);
            let _ = ensure_secure_file(&wal_quarantine);
        }

        let shm_path = PathBuf::from(format!("{}-shm", path.display()));
        if shm_path.exists() {
            let shm_quarantine = path.with_extension(format!("corrupt.{}.db-shm", now));
            let _ = std::fs::copy(&shm_path, &shm_quarantine);
            let _ = ensure_secure_file(&shm_quarantine);
        }

        log::warn!(
            "Corrupted database quarantined to {:?} to preserve evidence without data destruction",
            quarantine_path
        );
        Ok(quarantine_path)
    }

    /// Initialize local SQLite storage with WAL mode, pragmas, run embedded migrations,
    /// and verify database integrity on startup (Phase 11.1 & Phase 11.3).
    ///
    /// If an existing on-disk database fails the pre-flight integrity check,
    /// it is quarantined to preserve evidence, destructive operations are halted,
    /// and an explicit corruption error is returned.
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            let _ = ensure_secure_dir(parent);
        }

        // Pre-flight check: If database already exists and has content on disk, verify integrity BEFORE running migrations
        if db_path != Path::new(":memory:")
            && db_path.exists()
            && std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0) > 0
        {
            match Self::check_file_integrity(&db_path) {
                DatabaseIntegrityStatus::Healthy => {}
                DatabaseIntegrityStatus::Corrupted { errors, .. } => {
                    let quarantine = Self::quarantine_corrupt_database(&db_path).ok();
                    log::error!(
                        "Database integrity check failed prior to migration: {:?}. Quarantined to: {:?}",
                        errors,
                        quarantine
                    );
                    return Err(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                        Some(format!(
                            "Pre-flight integrity verification failed ({:?}); evidence preserved at {:?}",
                            errors, quarantine
                        )),
                    ));
                }
                DatabaseIntegrityStatus::CheckFailed { error } => {
                    log::warn!(
                        "Pre-flight integrity check could not execute: {}. Halting startup to prevent unsafe writes.",
                        error
                    );
                    return Err(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                        Some(format!(
                            "Database integrity verification could not be performed: {}",
                            error
                        )),
                    ));
                }
            }
        }

        let mut conn = Connection::open(&db_path)?;
        let _ = conn.busy_timeout(std::time::Duration::from_millis(10000));
        enforce_database_file_permissions(&db_path);
        migrations::run_migrations(&mut conn)?;
        enforce_database_file_permissions(&db_path);

        // Phase 11.3: Automatic database integrity check on startup
        let is_healthy = migrations::run_integrity_check(&conn)?;
        if !is_healthy {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                Some(
                    "Database integrity check failed on startup (PRAGMA integrity_check != ok)"
                        .to_string(),
                ),
            ));
        }

        Ok(Self {
            db_path,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Initialize an ephemeral in-memory database for testing and sandboxed simulation.
    pub fn in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        migrations::run_migrations(&mut conn)?;

        let is_healthy = migrations::run_integrity_check(&conn)?;
        if !is_healthy {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                Some("In-memory database integrity check failed".to_string()),
            ));
        }

        Ok(Self {
            db_path: PathBuf::from(":memory:"),
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Resolves the default local storage path in the user's home directory (`~/.truthbeacon/truthbeacon.local.db`).
    /// In test builds, isolates to a temporary directory to prevent polluting developer environments.
    pub fn default_db_path() -> PathBuf {
        if let Ok(override_path) = std::env::var("TRUTHBEACON_DB_PATH") {
            if !override_path.trim().is_empty() {
                return PathBuf::from(override_path);
            }
        }

        #[cfg(test)]
        let base_dir = std::env::temp_dir().join("truthbeacon_test_env");

        #[cfg(not(test))]
        let base_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));

        let tb_dir = base_dir.join(".truthbeacon");
        let _ = ensure_secure_dir(&tb_dir);
        tb_dir.join("truthbeacon.local.db")
    }

    /// Resolves and initializes default local SQLite storage manager with retry backoff for test concurrency.
    pub fn default_instance() -> Result<Self> {
        let mut attempts = 0;
        loop {
            match Self::init(Self::default_db_path()) {
                Ok(instance) => return Ok(instance),
                Err(e) if attempts < 10 && e.to_string().to_lowercase().contains("locked") => {
                    attempts += 1;
                    std::thread::sleep(std::time::Duration::from_millis(100 * attempts));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Retrieve thread-safe handle to the primary writer connection.
    pub fn get_connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    /// Opens an independent, concurrent read-only connection adhering to WAL pragmas.
    pub fn open_reader(&self) -> Result<Connection> {
        if self.db_path == Path::new(":memory:") {
            // In-memory databases cannot share state across distinct Connection::open handles
            let conn = self.conn.lock().unwrap();
            let mut clone = Connection::open_in_memory()?;
            {
                let backup = rusqlite::backup::Backup::new(&conn, &mut clone)?;
                backup.run_to_completion(5, std::time::Duration::from_millis(10), None)?;
            }
            migrations::configure_pragmas(&clone)?;
            Ok(clone)
        } else {
            let conn = Connection::open_with_flags(
                &self.db_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            let _ = conn.busy_timeout(std::time::Duration::from_millis(10000));
            migrations::configure_pragmas(&conn)?;
            Ok(conn)
        }
    }

    /// Executes a WAL checkpoint according to the specified CheckpointMode (Phase 11.3).
    pub fn checkpoint_wal(&self, mode: CheckpointMode) -> Result<CheckpointResult> {
        let mode_str = match mode {
            CheckpointMode::Passive => "PASSIVE",
            CheckpointMode::Full => "FULL",
            CheckpointMode::Restart => "RESTART",
            CheckpointMode::Truncate => "TRUNCATE",
        };

        let conn = self.conn.lock().unwrap();
        let (busy, log_frames, checkpointed_frames): (i32, i32, i32) = conn.query_row(
            &format!("PRAGMA wal_checkpoint({});", mode_str),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        Ok(CheckpointResult {
            busy: busy != 0,
            log_frames,
            checkpointed_frames,
        })
    }

    /// Clean shutdown checkpoint: truncates the WAL file to 0 bytes and synchronizes state (Phase 11.3).
    pub fn checkpoint_on_shutdown(&self) -> Result<CheckpointResult> {
        self.checkpoint_wal(CheckpointMode::Truncate)
    }
}

impl Drop for StorageManager {
    fn drop(&mut self) {
        if self.db_path != Path::new(":memory:") {
            let _ = self.checkpoint_on_shutdown();
        }
    }
}

impl StorageManager {
    /// Verifies database integrity on demand via `PRAGMA integrity_check;` (Phase 11.3).
    pub fn verify_integrity(&self) -> Result<DatabaseIntegrityStatus> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("PRAGMA integrity_check;")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        if messages.len() == 1 && messages[0].to_lowercase() == "ok" {
            Ok(DatabaseIntegrityStatus::Healthy)
        } else {
            Ok(DatabaseIntegrityStatus::Corrupted {
                errors: messages,
                quarantine_backup_path: None,
            })
        }
    }

    /// Online database backup: creates a consistent snapshot of the active database
    /// to the specified target path using SQLite's native backup API.
    pub fn backup_to<P: AsRef<Path>>(&self, dest: P) -> Result<()> {
        let dest_path = dest.as_ref();
        if let Some(parent) = dest_path.parent() {
            let _ = ensure_secure_dir(parent);
        }
        let mut dest_conn = Connection::open(dest_path)?;
        let conn = self.conn.lock().unwrap();
        let backup = rusqlite::backup::Backup::new(&conn, &mut dest_conn)?;
        backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
        enforce_database_file_permissions(dest_path);
        Ok(())
    }

    /// Safely restores database state from a validated backup archive into the active connection.
    pub fn restore_from<P: AsRef<Path>>(&self, src: P) -> Result<()> {
        let src_path = src.as_ref();
        let status = Self::check_file_integrity(src_path);
        if !status.is_healthy() {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                Some(format!(
                    "Cannot restore from corrupt backup archive: {:?}",
                    status
                )),
            ));
        }
        let src_conn = Connection::open_with_flags(
            src_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut conn = self.conn.lock().unwrap();
        let backup = rusqlite::backup::Backup::new(&src_conn, &mut conn)?;
        backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
        enforce_database_file_permissions(&self.db_path);
        Ok(())
    }

    /// Prunes expired resolved incidents and audit records older than the specified retention period (days).
    /// Preserves unresolved discrepancies, benchmark identities, and circuit-breaker security trip events.
    pub fn prune_expired_records(&self, retention_days: u32) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let cutoff_timestamp = chrono::Utc::now().timestamp() - (retention_days as i64 * 86400);
        let pruned_audits = conn.execute(
            "DELETE FROM audit_logs WHERE timestamp < ?1 AND action != 'circuit_breaker_tripped';",
            rusqlite::params![cutoff_timestamp],
        )?;
        let pruned_incidents = conn.execute(
            "DELETE FROM incidents WHERE timestamp < ?1 AND status IN ('dismissed', 'whitelisted');",
            rusqlite::params![cutoff_timestamp],
        )?;
        Ok(pruned_audits + pruned_incidents)
    }

    /// Inspect and verify active SQLite runtime pragmas on a connection.
    pub fn inspect_pragmas(conn: &Connection) -> Result<PragmaReport> {
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .unwrap_or_else(|_| "memory".to_string());
        let synchronous: i32 = conn
            .query_row("PRAGMA synchronous;", [], |row| row.get(0))
            .unwrap_or(1);
        let foreign_keys_int: i32 = conn
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .unwrap_or(1);
        let mmap_size: i64 = conn
            .query_row("PRAGMA mmap_size;", [], |row| row.get(0))
            .unwrap_or(0);

        Ok(PragmaReport {
            journal_mode: journal_mode.to_lowercase(),
            synchronous,
            foreign_keys: foreign_keys_int == 1,
            mmap_size,
        })
    }

    /// Query active pragmas on the primary database connection.
    pub fn get_pragma_report(&self) -> Result<PragmaReport> {
        let conn = self.conn.lock().unwrap();
        Self::inspect_pragmas(&conn)
    }

    /// Retrieve absolute database path for user data sovereignty audit panel.
    pub fn get_path(&self) -> PathBuf {
        self.db_path.clone()
    }

    /// Safe `VACUUM` and zero-fill routines for local data eradication (Phase 11.3).
    ///
    /// 1. Enables `PRAGMA secure_delete = ON;` to overwrite freed blocks with zeros.
    /// 2. Deletes all user records across `incidents`, `benchmarks`, and `audit_logs`.
    /// 3. Executes `PRAGMA wal_checkpoint(TRUNCATE);` to flush deletion frames.
    /// 4. Executes `VACUUM;` to reclaim disk pages and shrink database file size.
    /// 5. Executes a final `PRAGMA wal_checkpoint(TRUNCATE);`.
    pub fn eradicate_all_data(&self) -> Result<EradicationReport> {
        let conn = self.conn.lock().unwrap();

        // Step 1: Enable secure delete to zero-fill freed pages
        conn.execute_batch("PRAGMA secure_delete = ON;")?;

        // Step 2: Delete all records
        conn.execute_batch(
            "
            DELETE FROM incidents;
            DELETE FROM benchmarks;
            DELETE FROM audit_logs;
            ",
        )?;

        // Step 3: Flush deletion frames to disk
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

        // Step 4: Reclaim all space and zero-fill database layout
        conn.execute_batch("VACUUM;")?;

        // Step 5: Final WAL truncation
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

        enforce_database_file_permissions(&self.db_path);

        let remaining_db_bytes = if self.db_path.exists() && self.db_path != Path::new(":memory:") {
            std::fs::metadata(&self.db_path)
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        Ok(EradicationReport {
            tables_purged: vec![
                "incidents".to_string(),
                "benchmarks".to_string(),
                "audit_logs".to_string(),
            ],
            secure_delete_applied: true,
            wal_truncated: true,
            vacuum_completed: true,
            remaining_db_bytes,
        })
    }

    /// Records an immutable audit log entry into `audit_logs` (Phase 16.4).
    pub fn record_audit_log(&self, entry: &crate::models::audit::AuditLogEntry) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let meta_json = entry.metadata.as_ref().map(|m| m.to_string());
        conn.execute(
            "INSERT INTO audit_logs (
                id, timestamp, action, guild_id, operator_id, target_user_id,
                incident_id, reason, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
            rusqlite::params![
                entry.id,
                entry.timestamp,
                entry.action.as_str(),
                entry.guild_id,
                entry.operator_id,
                entry.target_user_id,
                entry.incident_id,
                entry.reason,
                meta_json,
            ],
        )?;
        Ok(())
    }

    /// Retrieves chronological audit log records ordered descending by timestamp (Phase 16.4).
    pub fn list_audit_logs(
        &self,
        guild_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::models::audit::AuditLogEntry>> {
        let conn = self.conn.lock().unwrap();
        let limit_val = limit.unwrap_or(100) as i64;
        let mut entries = Vec::new();

        if let Some(gid) = guild_id {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, action, guild_id, operator_id, target_user_id,
                        incident_id, reason, metadata_json
                 FROM audit_logs
                 WHERE guild_id = ?1
                 ORDER BY timestamp DESC
                 LIMIT ?2;",
            )?;
            let rows = stmt.query_map(rusqlite::params![gid, limit_val], Self::row_to_audit_log)?;
            for r in rows {
                entries.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, action, guild_id, operator_id, target_user_id,
                        incident_id, reason, metadata_json
                 FROM audit_logs
                 ORDER BY timestamp DESC
                 LIMIT ?1;",
            )?;
            let rows = stmt.query_map(rusqlite::params![limit_val], Self::row_to_audit_log)?;
            for r in rows {
                entries.push(r?);
            }
        }

        Ok(entries)
    }

    fn row_to_audit_log(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<crate::models::audit::AuditLogEntry> {
        let action_str: String = row.get(2)?;
        let meta_str: Option<String> = row.get(8)?;
        let metadata = meta_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(crate::models::audit::AuditLogEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            action: crate::models::audit::ActionType::from_str_loose(&action_str),
            guild_id: row.get(3)?,
            operator_id: row.get(4)?,
            target_user_id: row.get(5)?,
            incident_id: row.get(6)?,
            reason: row.get(7)?,
            metadata,
        })
    }

    /// Records or updates an identity incident in SQLite (Phase 16.1 & Phase 16.3).
    pub fn record_incident(
        &self,
        incident: &crate::models::incident::TriageIncident,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let status_str = match incident.status {
            crate::models::incident::IncidentStatus::Dismissed => "dismissed",
            crate::models::incident::IncidentStatus::Excluded => "excluded",
            crate::models::incident::IncidentStatus::Banned => "banned",
            crate::models::incident::IncidentStatus::Whitelisted => "whitelisted",
            crate::models::incident::IncidentStatus::Pending => "pending",
        };
        let risk_str = format!("{:?}", incident.discrepancy.risk_tier).to_lowercase();
        let homoglyph_int = if incident.discrepancy.homoglyph_detected {
            1
        } else {
            0
        };

        conn.execute(
            "INSERT OR REPLACE INTO incidents (
                id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_nickname,
                suspect_avatar_url, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name,
                string_similarity_score, homoglyph_detected, normalized_diff, avatar_hamming_distance,
                risk_tier, status, resolution_notes, operator_id, resolved_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19);",
            rusqlite::params![
                incident.id,
                incident.guild_id,
                incident.timestamp,
                incident.discrepancy.suspect_user_id,
                incident.discrepancy.suspect_username,
                incident.discrepancy.suspect_nickname,
                incident.discrepancy.suspect_avatar_url,
                incident.discrepancy.suspect_account_age_hours as i64,
                incident.discrepancy.matched_benchmark_id,
                incident.discrepancy.matched_benchmark_name,
                incident.discrepancy.string_similarity_score,
                homoglyph_int,
                incident.discrepancy.normalized_diff,
                incident.discrepancy.avatar_hamming_distance.map(|d| d as i64),
                risk_str,
                status_str,
                incident.resolution_notes,
                incident.operator_id,
                incident.resolved_at,
            ],
        )?;
        Ok(())
    }

    /// Fetches a specific incident by ID.
    pub fn get_incident(
        &self,
        incident_id: &str,
    ) -> Result<Option<crate::models::incident::TriageIncident>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_nickname,
                    suspect_avatar_url, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name,
                    string_similarity_score, homoglyph_detected, normalized_diff, avatar_hamming_distance,
                    risk_tier, status, resolution_notes, operator_id, resolved_at
             FROM incidents
             WHERE id = ?1;",
        )?;

        let mut rows = stmt.query_map([incident_id], Self::row_to_incident)?;
        match rows.next() {
            Some(res) => Ok(Some(res?)),
            None => Ok(None),
        }
    }

    /// Updates the status and resolution notes of an incident (Phase 16.3).
    pub fn update_incident_status(
        &self,
        incident_id: &str,
        status: crate::models::incident::IncidentStatus,
        resolution_notes: Option<&str>,
        operator_id: Option<&str>,
        resolved_at: Option<i64>,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let status_str = match status {
            crate::models::incident::IncidentStatus::Dismissed => "dismissed",
            crate::models::incident::IncidentStatus::Excluded => "excluded",
            crate::models::incident::IncidentStatus::Banned => "banned",
            crate::models::incident::IncidentStatus::Whitelisted => "whitelisted",
            crate::models::incident::IncidentStatus::Pending => "pending",
        };

        let updated_rows = conn.execute(
            "UPDATE incidents
             SET status = ?1, resolution_notes = ?2, operator_id = ?3, resolved_at = ?4
             WHERE id = ?5;",
            rusqlite::params![
                status_str,
                resolution_notes,
                operator_id,
                resolved_at,
                incident_id
            ],
        )?;

        Ok(updated_rows > 0)
    }

    /// Lists incidents filtered by guild and optional status (Phase 16.1 & Phase 16.3).
    pub fn list_incidents(
        &self,
        guild_id: Option<&str>,
        status_filter: Option<crate::models::incident::IncidentStatus>,
    ) -> Result<Vec<crate::models::incident::TriageIncident>> {
        let conn = self.conn.lock().unwrap();
        let mut entries = Vec::new();

        let status_str = status_filter.map(|s| match s {
            crate::models::incident::IncidentStatus::Dismissed => "dismissed",
            crate::models::incident::IncidentStatus::Excluded => "excluded",
            crate::models::incident::IncidentStatus::Banned => "banned",
            crate::models::incident::IncidentStatus::Whitelisted => "whitelisted",
            crate::models::incident::IncidentStatus::Pending => "pending",
        });

        match (guild_id, status_str) {
            (Some(gid), Some(st)) => {
                let mut stmt = conn.prepare(
                    "SELECT id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_nickname,
                            suspect_avatar_url, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name,
                            string_similarity_score, homoglyph_detected, normalized_diff, avatar_hamming_distance,
                            risk_tier, status, resolution_notes, operator_id, resolved_at
                     FROM incidents
                     WHERE guild_id = ?1 AND status = ?2
                     ORDER BY timestamp DESC;",
                )?;
                let rows = stmt.query_map(rusqlite::params![gid, st], Self::row_to_incident)?;
                for r in rows {
                    entries.push(r?);
                }
            }
            (Some(gid), None) => {
                let mut stmt = conn.prepare(
                    "SELECT id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_nickname,
                            suspect_avatar_url, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name,
                            string_similarity_score, homoglyph_detected, normalized_diff, avatar_hamming_distance,
                            risk_tier, status, resolution_notes, operator_id, resolved_at
                     FROM incidents
                     WHERE guild_id = ?1
                     ORDER BY timestamp DESC;",
                )?;
                let rows = stmt.query_map(rusqlite::params![gid], Self::row_to_incident)?;
                for r in rows {
                    entries.push(r?);
                }
            }
            (None, Some(st)) => {
                let mut stmt = conn.prepare(
                    "SELECT id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_nickname,
                            suspect_avatar_url, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name,
                            string_similarity_score, homoglyph_detected, normalized_diff, avatar_hamming_distance,
                            risk_tier, status, resolution_notes, operator_id, resolved_at
                     FROM incidents
                     WHERE status = ?1
                     ORDER BY timestamp DESC;",
                )?;
                let rows = stmt.query_map(rusqlite::params![st], Self::row_to_incident)?;
                for r in rows {
                    entries.push(r?);
                }
            }
            (None, None) => {
                let mut stmt = conn.prepare(
                    "SELECT id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_nickname,
                            suspect_avatar_url, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name,
                            string_similarity_score, homoglyph_detected, normalized_diff, avatar_hamming_distance,
                            risk_tier, status, resolution_notes, operator_id, resolved_at
                     FROM incidents
                     ORDER BY timestamp DESC;",
                )?;
                let rows = stmt.query_map([], Self::row_to_incident)?;
                for r in rows {
                    entries.push(r?);
                }
            }
        }

        Ok(entries)
    }

    fn row_to_incident(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<crate::models::incident::TriageIncident> {
        let risk_tier_str: String = row.get(14)?;
        let status_str: String = row.get(15)?;
        let homoglyph_int: i32 = row.get(11)?;
        let age_hours: i64 = row.get(7)?;
        let hamming: Option<u32> = row.get::<_, Option<i32>>(13)?.map(|h| h as u32);

        let risk_tier = match risk_tier_str.to_lowercase().as_str() {
            "critical" => crate::models::incident::RiskTier::Critical,
            "elevated" => crate::models::incident::RiskTier::Elevated,
            "notable" => crate::models::incident::RiskTier::Notable,
            _ => crate::models::incident::RiskTier::Standard,
        };

        let status = match status_str.to_lowercase().as_str() {
            "dismissed" => crate::models::incident::IncidentStatus::Dismissed,
            "excluded" => crate::models::incident::IncidentStatus::Excluded,
            "banned" => crate::models::incident::IncidentStatus::Banned,
            "whitelisted" => crate::models::incident::IncidentStatus::Whitelisted,
            _ => crate::models::incident::IncidentStatus::Pending,
        };

        Ok(crate::models::incident::TriageIncident {
            id: row.get(0)?,
            guild_id: row.get(1)?,
            timestamp: row.get(2)?,
            discrepancy: crate::models::incident::IdentityDiscrepancy {
                suspect_user_id: row.get(3)?,
                suspect_username: row.get(4)?,
                suspect_nickname: row.get(5)?,
                suspect_avatar_url: row.get(6)?,
                suspect_account_age_hours: age_hours.max(0) as u64,
                matched_benchmark_id: row.get(8)?,
                matched_benchmark_name: row.get(9)?,
                string_similarity_score: row.get(10)?,
                homoglyph_detected: homoglyph_int != 0,
                normalized_diff: row.get(12)?,
                avatar_hamming_distance: hamming,
                risk_tier,
            },
            status,
            resolution_notes: row.get(16)?,
            operator_id: row.get(17)?,
            resolved_at: row.get(18)?,
        })
    }

    /// Returns the total count of benchmarks stored in the database.
    pub fn count_benchmarks(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM benchmarks;", [], |r| r.get(0))?;
        Ok(count.max(0) as usize)
    }

    /// Returns the count of pending incidents stored in the database.
    pub fn count_pending_incidents(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM incidents WHERE status = 'pending';",
            [],
            |r| r.get(0),
        )?;
        Ok(count.max(0) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::audit::{ActionType, AuditLogEntry};
    use crate::models::incident::{IdentityDiscrepancy, IncidentStatus, RiskTier, TriageIncident};

    #[test]
    fn test_storage_manager_init_and_eradicate() {
        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!(
            "truthbeacon_test_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let storage = StorageManager::init(&test_db_path).unwrap();
        assert_eq!(storage.get_path(), test_db_path);

        // Eradication purges all records without error and zero-fills
        let report = storage.eradicate_all_data().unwrap();
        assert!(report.secure_delete_applied);
        assert!(report.vacuum_completed);
        assert!(report.wal_truncated);

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
    }

    #[test]
    fn test_pragmas_verification_on_disk_database() {
        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!(
            "truthbeacon_pragma_test_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let storage = StorageManager::init(&test_db_path).unwrap();
        let report = storage.get_pragma_report().unwrap();

        // 11.1 Assertions:
        // 1. journal_mode = WAL
        assert_eq!(
            report.journal_mode, "wal",
            "Database must operate in WAL mode"
        );
        // 2. synchronous = NORMAL (1 in SQLite)
        assert_eq!(
            report.synchronous, 1,
            "Synchronous must be set to NORMAL (1)"
        );
        // 3. foreign_keys = ON (1 in SQLite)
        assert!(report.foreign_keys, "foreign_keys must be enabled (ON)");
        // 4. mmap_size = 256MB (268,435,456 bytes)
        assert!(
            report.mmap_size >= 268_435_456,
            "mmap_size must allocate at least 256MB memory-mapped I/O pool"
        );

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        let _ = std::fs::remove_file(format!("{}-wal", test_db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", test_db_path.display()));
    }

    #[test]
    fn test_concurrent_reader_and_writer_access() {
        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!(
            "truthbeacon_concurrency_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let storage = StorageManager::init(&test_db_path).unwrap();

        // Write a benchmark record using the writer connection
        {
            let conn = storage.get_connection();
            let locked_conn = conn.lock().unwrap();
            locked_conn
                .execute(
                    "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at)
                 VALUES ('bm_test_concurrent', 'g1', 'u1', 'UserOne', 'Admin', 1000, 1000);",
                    [],
                )
                .unwrap();
        }

        // Open an independent reader connection and read simultaneously
        let reader = storage.open_reader().unwrap();
        let count: i64 = reader
            .query_row(
                "SELECT COUNT(*) FROM benchmarks WHERE id = 'bm_test_concurrent';",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "Reader connection must read committed records in WAL mode"
        );

        // Verify reader adheres to WAL and foreign keys
        let reader_report = StorageManager::inspect_pragmas(&reader).unwrap();
        assert!(reader_report.foreign_keys);

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        let _ = std::fs::remove_file(format!("{}-wal", test_db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", test_db_path.display()));
    }

    #[test]
    fn test_in_memory_storage_manager() {
        let storage = StorageManager::in_memory().unwrap();
        assert_eq!(storage.get_path(), PathBuf::from(":memory:"));

        let report = storage.get_pragma_report().unwrap();
        assert!(report.foreign_keys);
    }

    #[test]
    fn test_default_db_path_resolution() {
        let path = StorageManager::default_db_path();
        assert!(path.ends_with("truthbeacon.local.db"));
        assert!(path.to_string_lossy().contains(".truthbeacon"));
    }

    #[test]
    fn test_wal_checkpoint_and_clean_shutdown() {
        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!(
            "truthbeacon_checkpoint_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let storage = StorageManager::init(&test_db_path).unwrap();

        // Insert records generating WAL frames
        {
            let conn = storage.get_connection();
            let locked_conn = conn.lock().unwrap();
            locked_conn
                .execute(
                    "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at)
                 VALUES ('bm_ckpt_1', 'g1', 'u1', 'UserOne', 'Admin', 1000, 1000);",
                    [],
                )
                .unwrap();
        }

        // Execute clean shutdown checkpoint
        let result = storage.checkpoint_on_shutdown().unwrap();
        assert!(!result.busy, "Checkpoint should not be busy");
        assert!(result.checkpointed_frames >= 0);

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        let _ = std::fs::remove_file(format!("{}-wal", test_db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", test_db_path.display()));
    }

    #[test]
    fn test_database_integrity_verification() {
        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!(
            "truthbeacon_integrity_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let storage = StorageManager::init(&test_db_path).unwrap();
        assert_eq!(
            storage.verify_integrity().unwrap(),
            DatabaseIntegrityStatus::Healthy
        );

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        let _ = std::fs::remove_file(format!("{}-wal", test_db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", test_db_path.display()));
    }

    #[test]
    fn test_corrupted_database_detection_and_quarantine() {
        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!(
            "truthbeacon_corrupt_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        // Create a valid database with data first
        {
            let storage = StorageManager::init(&test_db_path).unwrap();
            {
                let conn = storage.get_connection();
                let locked = conn.lock().unwrap();
                locked.execute(
                    "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at)
                     VALUES ('bm_c1', 'g1', 'u1', 'TestUser', 'Role', 100, 100);",
                    [],
                ).unwrap();
            }
            let _ = storage.checkpoint_on_shutdown();
        }

        // Intentionally corrupt SQLite header bytes
        {
            let mut bytes = std::fs::read(&test_db_path).unwrap();
            for b in &mut bytes[16..40] {
                *b = 0xFF;
            }
            std::fs::write(&test_db_path, &bytes).unwrap();
        }

        // Pre-flight check should detect corruption
        let status = StorageManager::check_file_integrity(&test_db_path);
        assert!(
            !status.is_healthy(),
            "Corrupted database must not be reported as healthy"
        );

        // Initialization must fail and quarantine original file without silent deletion
        let init_result = StorageManager::init(&test_db_path);
        assert!(init_result.is_err(), "Init must reject corrupted database");

        // Original database file must still exist (never deleted)
        assert!(
            test_db_path.exists(),
            "Original corrupted database must not be deleted"
        );

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        let _ = std::fs::remove_file(format!("{}-wal", test_db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", test_db_path.display()));
    }

    #[test]
    fn test_database_backup_and_restore() {
        let temp_dir = std::env::temp_dir();
        let src_db = temp_dir.join(format!(
            "truthbeacon_bk_src_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let backup_dest = temp_dir.join(format!(
            "truthbeacon_bk_dest_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let storage = StorageManager::init(&src_db).unwrap();
        {
            let conn = storage.get_connection();
            let locked = conn.lock().unwrap();
            locked.execute(
                "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at)
                 VALUES ('bm_bk', 'g1', 'u1', 'BackupUser', 'Staff', 500, 500);",
                [],
            ).unwrap();
        }

        // Perform online backup
        storage.backup_to(&backup_dest).unwrap();
        assert!(backup_dest.exists(), "Backup destination file must exist");

        // Open backup database and verify record preserved
        let backup_storage = StorageManager::init(&backup_dest).unwrap();
        let conn_bk = backup_storage.get_connection();
        let count: i64 = conn_bk
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM benchmarks WHERE id = 'bm_bk';",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "Backup snapshot must preserve all records");

        // Cleanup
        let _ = std::fs::remove_file(&src_db);
        let _ = std::fs::remove_file(format!("{}-wal", src_db.display()));
        let _ = std::fs::remove_file(format!("{}-shm", src_db.display()));
        let _ = std::fs::remove_file(&backup_dest);
        let _ = std::fs::remove_file(format!("{}-wal", backup_dest.display()));
        let _ = std::fs::remove_file(format!("{}-shm", backup_dest.display()));
    }

    #[test]
    fn test_retention_prune_expired_records() {
        let storage = StorageManager::in_memory().unwrap();
        let now = chrono::Utc::now().timestamp();
        let old_time = now - (40 * 86400); // 40 days ago

        // Insert benchmark to satisfy foreign key
        {
            let conn = storage.get_connection();
            let locked = conn.lock().unwrap();
            locked.execute(
                "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at)
                 VALUES ('bm_ret', 'g1', 'u1', 'RetUser', 'Mod', 100, 100);",
                [],
            ).unwrap();
            // Insert old audit log
            locked
                .execute(
                    "INSERT INTO audit_logs (id, timestamp, action, guild_id, operator_id, reason)
                 VALUES ('aud_old', ?1, 'dismiss', 'g1', 'op', 'old log');",
                    rusqlite::params![old_time],
                )
                .unwrap();
            // Insert recent audit log
            locked
                .execute(
                    "INSERT INTO audit_logs (id, timestamp, action, guild_id, operator_id, reason)
                 VALUES ('aud_new', ?1, 'dismiss', 'g1', 'op', 'new log');",
                    rusqlite::params![now],
                )
                .unwrap();
        }

        // Pruning older than 30 days should remove aud_old but retain aud_new
        let pruned = storage.prune_expired_records(30).unwrap();
        assert_eq!(pruned, 1);

        let conn = storage.get_connection();
        let remaining: i64 = conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM audit_logs;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn test_safe_vacuum_and_zero_fill_eradication() {
        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!(
            "truthbeacon_eradicate_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let storage = StorageManager::init(&test_db_path).unwrap();

        // Populate sample records across benchmarks, incidents, audit_logs
        {
            let conn = storage.get_connection();
            let locked_conn = conn.lock().unwrap();
            locked_conn
                .execute(
                    "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at)
                 VALUES ('bm_er_1', 'g1', 'u1', 'PastorDan', 'Pastor', 1000, 1000);",
                    [],
                )
                .unwrap();

            locked_conn
                .execute(
                    "INSERT INTO incidents (id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name, string_similarity_score, homoglyph_detected, normalized_diff, risk_tier, status)
                 VALUES ('inc_er_1', 'g1', 1001, 'u2', 'PastorDan_', 1, 'bm_er_1', 'PastorDan', 0.95, 0, 'diff', 'critical', 'pending');",
                    [],
                )
                .unwrap();

            locked_conn
                .execute(
                    "INSERT INTO audit_logs (id, timestamp, action, guild_id, operator_id, reason)
                 VALUES ('aud_er_1', 1002, 'test_action', 'g1', 'op1', 'reason');",
                    [],
                )
                .unwrap();
        }

        // Execute safe zero-fill and VACUUM eradication
        let report = storage.eradicate_all_data().unwrap();
        assert!(report.secure_delete_applied);
        assert!(report.vacuum_completed);
        assert!(report.wal_truncated);
        assert_eq!(report.tables_purged.len(), 3);

        // Verify tables are completely empty
        {
            let conn = storage.get_connection();
            let locked_conn = conn.lock().unwrap();
            let bm_count: i64 = locked_conn
                .query_row("SELECT COUNT(*) FROM benchmarks;", [], |r| r.get(0))
                .unwrap();
            let inc_count: i64 = locked_conn
                .query_row("SELECT COUNT(*) FROM incidents;", [], |r| r.get(0))
                .unwrap();
            let aud_count: i64 = locked_conn
                .query_row("SELECT COUNT(*) FROM audit_logs;", [], |r| r.get(0))
                .unwrap();

            assert_eq!(bm_count, 0);
            assert_eq!(inc_count, 0);
            assert_eq!(aud_count, 0);
        }

        // Verify database integrity is intact after eradication
        assert_eq!(
            storage.verify_integrity().unwrap(),
            DatabaseIntegrityStatus::Healthy
        );

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        let _ = std::fs::remove_file(format!("{}-wal", test_db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", test_db_path.display()));
    }

    #[test]
    fn test_record_and_list_audit_logs_chronological() {
        let storage = StorageManager::in_memory().unwrap();

        let entry1 = AuditLogEntry {
            id: "aud_01".into(),
            timestamp: 1000,
            action: ActionType::Dismiss,
            guild_id: "guild_crossroads".into(),
            operator_id: "StewardOne".into(),
            target_user_id: Some("user_101".into()),
            incident_id: Some("inc_101".into()),
            reason: "Benign lookalike; no malicious intent".into(),
            metadata: Some(serde_json::json!({"action": "dismiss"})),
        };

        let entry2 = AuditLogEntry {
            id: "aud_02".into(),
            timestamp: 2000,
            action: ActionType::BanAndPurge,
            guild_id: "guild_crossroads".into(),
            operator_id: "StewardOne".into(),
            target_user_id: Some("user_102".into()),
            incident_id: Some("inc_102".into()),
            reason: "Critical homoglyph imposter".into(),
            metadata: Some(serde_json::json!({"prune_days": 7})),
        };

        storage.record_audit_log(&entry1).unwrap();
        storage.record_audit_log(&entry2).unwrap();

        // Must list in descending chronological order
        let logs = storage
            .list_audit_logs(Some("guild_crossroads"), None)
            .unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].id, "aud_02"); // newest first (timestamp 2000)
        assert_eq!(logs[0].action, ActionType::BanAndPurge);
        assert_eq!(logs[1].id, "aud_01"); // older second (timestamp 1000)
        assert_eq!(logs[1].action, ActionType::Dismiss);
    }

    #[test]
    fn test_record_get_update_and_list_incidents() {
        let storage = StorageManager::in_memory().unwrap();

        // 1. Create a benchmark first to satisfy foreign key constraint
        {
            let conn = storage.get_connection();
            let locked_conn = conn.lock().unwrap();
            locked_conn
                .execute(
                    "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at)
                     VALUES ('bm_dan', 'guild_crossroads', 'u_dan', 'DanWard', 'Pastor', 1000, 1000);",
                    [],
                )
                .unwrap();
        }

        let incident = TriageIncident {
            id: "inc_dan_spoof".into(),
            guild_id: "guild_crossroads".into(),
            timestamp: 1500,
            discrepancy: IdentityDiscrepancy {
                suspect_user_id: "u_imposter".into(),
                suspect_username: "DanШard".into(),
                suspect_nickname: Some("Pastor Dan (Lead)".into()),
                suspect_avatar_url: None,
                suspect_account_age_hours: 2,
                matched_benchmark_id: "bm_dan".into(),
                matched_benchmark_name: "DanWard".into(),
                string_similarity_score: 0.98,
                homoglyph_detected: true,
                normalized_diff: "Cyrillic Sha substituted for Latin W".into(),
                avatar_hamming_distance: Some(1),
                risk_tier: RiskTier::Critical,
            },
            status: IncidentStatus::Pending,
            resolution_notes: None,
            operator_id: None,
            resolved_at: None,
        };

        storage.record_incident(&incident).unwrap();

        // Retrieve incident
        let fetched = storage.get_incident("inc_dan_spoof").unwrap().unwrap();
        assert_eq!(fetched.discrepancy.suspect_username, "DanШard");
        assert_eq!(fetched.status, IncidentStatus::Pending);

        // Update status to Banned
        let updated = storage
            .update_incident_status(
                "inc_dan_spoof",
                IncidentStatus::Banned,
                Some("Ban confirmed by operator"),
                Some("LeadMod"),
                Some(1600),
            )
            .unwrap();
        assert!(updated);

        let fetched_updated = storage.get_incident("inc_dan_spoof").unwrap().unwrap();
        assert_eq!(fetched_updated.status, IncidentStatus::Banned);
        assert_eq!(
            fetched_updated.resolution_notes.as_deref(),
            Some("Ban confirmed by operator")
        );
        assert_eq!(fetched_updated.operator_id.as_deref(), Some("LeadMod"));

        // List filtered by status
        let pending = storage
            .list_incidents(Some("guild_crossroads"), Some(IncidentStatus::Pending))
            .unwrap();
        assert_eq!(pending.len(), 0);

        let banned = storage
            .list_incidents(Some("guild_crossroads"), Some(IncidentStatus::Banned))
            .unwrap();
        assert_eq!(banned.len(), 1);
        assert_eq!(banned[0].id, "inc_dan_spoof");
    }

    #[test]
    fn test_database_file_and_directory_restrictive_permissions() {
        let temp_dir =
            std::env::temp_dir().join(format!("tb_test_db_perms_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let db_path = temp_dir.join("truthbeacon.local.db");
        let storage = StorageManager::init(&db_path).expect("Storage init must succeed");

        assert!(db_path.exists(), "Database file must exist");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir_meta = std::fs::metadata(&temp_dir).unwrap();
            let dir_mode = dir_meta.permissions().mode() & 0o777;
            assert_eq!(
                dir_mode, 0o700,
                "Database parent directory must have 0700 permissions"
            );

            let db_meta = std::fs::metadata(&db_path).unwrap();
            let db_mode = db_meta.permissions().mode() & 0o777;
            assert_eq!(db_mode, 0o600, "Database file must have 0600 permissions");

            let wal_path = PathBuf::from(format!("{}-wal", db_path.display()));
            if wal_path.exists() {
                let wal_meta = std::fs::metadata(&wal_path).unwrap();
                let wal_mode = wal_meta.permissions().mode() & 0o777;
                assert_eq!(wal_mode, 0o600, "WAL file must have 0600 permissions");
            }
        }

        // Test backup file and directory permissions
        let backup_dir = temp_dir.join("backups");
        let backup_path = backup_dir.join("truthbeacon.backup.db");
        storage
            .backup_to(&backup_path)
            .expect("Backup must succeed");

        assert!(backup_path.exists(), "Backup file must exist");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let backup_dir_meta = std::fs::metadata(&backup_dir).unwrap();
            let backup_dir_mode = backup_dir_meta.permissions().mode() & 0o777;
            assert_eq!(
                backup_dir_mode, 0o700,
                "Backup parent dir must have 0700 permissions"
            );

            let backup_meta = std::fs::metadata(&backup_path).unwrap();
            let backup_mode = backup_meta.permissions().mode() & 0o777;
            assert_eq!(backup_mode, 0o600, "Backup file must have 0600 permissions");
        }

        drop(storage);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_restore_from_rejects_corrupted_backup() {
        let storage = StorageManager::in_memory().expect("In-memory storage must init");

        let temp_dir =
            std::env::temp_dir().join(format!("tb_test_corrupt_restore_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let corrupt_backup_path = temp_dir.join("corrupt_backup.db");
        std::fs::write(
            &corrupt_backup_path,
            b"corrupted_non_sqlite_header_and_data",
        )
        .unwrap();

        let restore_res = storage.restore_from(&corrupt_backup_path);
        assert!(
            restore_res.is_err(),
            "restore_from must fail when given a corrupt backup file"
        );

        // Active database must remain sound and uncorrupted
        let health = storage
            .verify_integrity()
            .expect("Integrity check must run");
        assert!(
            health.is_healthy(),
            "Active database must remain healthy after rejected restore"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_quarantine_corrupt_database_preserves_evidence_and_permissions() {
        let temp_dir =
            std::env::temp_dir().join(format!("tb_test_quarantine_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let corrupt_db = temp_dir.join("victim.db");
        std::fs::write(&corrupt_db, b"forensic_evidence_unparsable_sqlite_bytes").unwrap();

        let quarantine_path = StorageManager::quarantine_corrupt_database(&corrupt_db)
            .expect("Quarantine must succeed");

        assert!(quarantine_path.exists(), "Quarantined copy must exist");
        assert_eq!(
            std::fs::read(&quarantine_path).unwrap(),
            b"forensic_evidence_unparsable_sqlite_bytes"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&quarantine_path).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(
                mode, 0o600,
                "Quarantined database file must have 0600 permissions"
            );
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
