use rusqlite::{Connection, Result};

pub const SCHEMA_VERSION: i32 = 1;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Configure SQLite pragmas for Write-Ahead Logging, performance, and durability
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA foreign_keys = ON;
        ",
    )?;

    // Create benchmarks table
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS benchmarks (
            id TEXT PRIMARY KEY,
            guild_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            canonical_username TEXT NOT NULL,
            server_nickname TEXT,
            community_role TEXT NOT NULL,
            avatar_url TEXT,
            avatar_perceptual_hash TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            tags TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_benchmarks_guild ON benchmarks(guild_id);
        CREATE INDEX IF NOT EXISTS idx_benchmarks_user ON benchmarks(user_id);
        ",
    )?;

    // Create incidents table
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS incidents (
            id TEXT PRIMARY KEY,
            guild_id TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            suspect_user_id TEXT NOT NULL,
            suspect_username TEXT NOT NULL,
            suspect_nickname TEXT,
            suspect_avatar_url TEXT,
            suspect_account_age_hours INTEGER NOT NULL,
            matched_benchmark_id TEXT NOT NULL,
            matched_benchmark_name TEXT NOT NULL,
            string_similarity_score REAL NOT NULL,
            homoglyph_detected INTEGER NOT NULL,
            normalized_diff TEXT NOT NULL,
            avatar_hamming_distance INTEGER,
            risk_tier TEXT NOT NULL,
            status TEXT NOT NULL,
            resolution_notes TEXT,
            operator_id TEXT,
            resolved_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_incidents_guild ON incidents(guild_id);
        CREATE INDEX IF NOT EXISTS idx_incidents_status ON incidents(status);
        CREATE INDEX IF NOT EXISTS idx_incidents_timestamp ON incidents(timestamp DESC);
        ",
    )?;

    // Create audit logs table
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS audit_logs (
            id TEXT PRIMARY KEY,
            timestamp INTEGER NOT NULL,
            action TEXT NOT NULL,
            guild_id TEXT NOT NULL,
            operator_id TEXT NOT NULL,
            target_user_id TEXT,
            incident_id TEXT,
            reason TEXT NOT NULL,
            metadata_json TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_audit_guild ON audit_logs(guild_id);
        ",
    )?;

    Ok(())
}
