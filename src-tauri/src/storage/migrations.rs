use rusqlite::{Connection, Result};

pub const CURRENT_SCHEMA_VERSION: i32 = 2;

/// Configures SQLite pragmas for resilience, high throughput, and memory-mapped I/O
pub fn configure_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA foreign_keys = ON;
        PRAGMA mmap_size = 268435456;
        ",
    )?;
    Ok(())
}

/// Retrieves the current schema user_version from SQLite
pub fn get_user_version(conn: &Connection) -> Result<i32> {
    conn.query_row("PRAGMA user_version;", [], |row| row.get(0))
}

/// Runs atomic, non-destructive embedded migrations up to CURRENT_SCHEMA_VERSION
pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    configure_pragmas(conn)?;

    let mut version = get_user_version(conn)?;

    // Migration Step 1: Base Relational Schema & Composite Indices
    if version < 1 {
        let tx = conn.transaction()?;
        tx.execute_batch(
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
            CREATE INDEX IF NOT EXISTS idx_benchmarks_guild_active ON benchmarks(guild_id, is_active);
            CREATE INDEX IF NOT EXISTS idx_benchmarks_user ON benchmarks(user_id);

            CREATE TABLE IF NOT EXISTS incidents (
                id TEXT PRIMARY KEY,
                guild_id TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                suspect_user_id TEXT NOT NULL,
                suspect_username TEXT NOT NULL,
                suspect_nickname TEXT,
                suspect_avatar_url TEXT,
                suspect_account_age_hours INTEGER NOT NULL,
                matched_benchmark_id TEXT NOT NULL REFERENCES benchmarks(id),
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
            CREATE INDEX IF NOT EXISTS idx_incidents_guild_timestamp ON incidents(guild_id, timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_incidents_status ON incidents(status);
            CREATE INDEX IF NOT EXISTS idx_incidents_matched_bm ON incidents(matched_benchmark_id);

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
            CREATE INDEX IF NOT EXISTS idx_audit_guild_timestamp ON audit_logs(guild_id, timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_audit_operator ON audit_logs(operator_id);

            PRAGMA user_version = 1;
            ",
        )?;
        tx.commit()?;
        version = 1;
    }

    // Migration Step 2 (Schema Evolution): Non-destructive column additions
    if version < 2 {
        let tx = conn.transaction()?;
        tx.execute_batch(
            "
            ALTER TABLE benchmarks ADD COLUMN sensitivity_override REAL DEFAULT NULL;
            ALTER TABLE incidents ADD COLUMN composite_risk_score REAL DEFAULT 0.0;
            PRAGMA user_version = 2;
            ",
        )?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fresh_migration_to_latest_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();

        let version = get_user_version(&conn).unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // Verify composite index exists
        let index_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name='idx_incidents_guild_timestamp'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(index_exists);

        let bm_index_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name='idx_benchmarks_guild_active'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(bm_index_exists);
    }

    #[test]
    fn test_schema_evolution_v1_to_v2_preserves_operator_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        configure_pragmas(&conn).unwrap();

        // Step 1: Manually create v1 schema and set version = 1
        conn.execute_batch(
            "
            CREATE TABLE benchmarks (
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

            CREATE TABLE incidents (
                id TEXT PRIMARY KEY,
                guild_id TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                suspect_user_id TEXT NOT NULL,
                suspect_username TEXT NOT NULL,
                suspect_nickname TEXT,
                suspect_avatar_url TEXT,
                suspect_account_age_hours INTEGER NOT NULL,
                matched_benchmark_id TEXT NOT NULL REFERENCES benchmarks(id),
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

            PRAGMA user_version = 1;
            ",
        )
        .unwrap();

        // Step 2: Insert initial operator records into v1 database
        conn.execute(
            "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at)
             VALUES ('bm_pastor', 'guild_alpha', '1001', 'PastorDan', 'Executive Pastor', 1700000000, 1700000000);",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO incidents (id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name, string_similarity_score, homoglyph_detected, normalized_diff, risk_tier, status)
             VALUES ('inc_001', 'guild_alpha', 1700000100, '2001', 'PastorDan_', 2, 'bm_pastor', 'PastorDan', 0.95, 0, 'diff', 'critical', 'pending');",
            [],
        )
        .unwrap();

        assert_eq!(get_user_version(&conn).unwrap(), 1);

        // Step 3: Run migration runner to evolve schema to v2
        run_migrations(&mut conn).unwrap();

        // Step 4: Verify schema version is now 2
        assert_eq!(get_user_version(&conn).unwrap(), 2);

        // Step 5: Verify existing operator data is completely preserved
        let username: String = conn
            .query_row(
                "SELECT canonical_username FROM benchmarks WHERE id = 'bm_pastor'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(username, "PastorDan");

        // Step 6: Verify newly evolved columns are present and default-initialized
        let sensitivity: Option<f64> = conn
            .query_row(
                "SELECT sensitivity_override FROM benchmarks WHERE id = 'bm_pastor'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(sensitivity.is_none());

        let risk_score: f64 = conn
            .query_row(
                "SELECT composite_risk_score FROM incidents WHERE id = 'inc_001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(risk_score, 0.0);
    }

    #[test]
    fn test_foreign_key_enforcement() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();

        // Inserting an incident referencing a non-existent benchmark must fail with Foreign Key violation
        let res = conn.execute(
            "INSERT INTO incidents (id, guild_id, timestamp, suspect_user_id, suspect_username, suspect_account_age_hours, matched_benchmark_id, matched_benchmark_name, string_similarity_score, homoglyph_detected, normalized_diff, risk_tier, status)
             VALUES ('inc_invalid', 'guild_alpha', 1700000100, '2001', 'Clone', 2, 'non_existent_bm', 'PastorDan', 0.95, 0, 'diff', 'critical', 'pending');",
            [],
        );

        assert!(
            res.is_err(),
            "Foreign key constraint must prevent orphan incidents"
        );
    }
}
