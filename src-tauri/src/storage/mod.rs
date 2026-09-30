pub mod migrations;

use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct StorageManager {
    db_path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl StorageManager {
    /// Initialize local SQLite storage with WAL mode and run embedded migrations
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        let conn = Connection::open(&db_path)?;

        migrations::run_migrations(&conn)?;

        Ok(Self {
            db_path,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Retrieve absolute database path for user data sovereignty audit panel
    pub fn get_path(&self) -> PathBuf {
        self.db_path.clone()
    }

    /// Safely purge all database records for one-click local data eradication
    pub fn eradicate_all_data(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            DELETE FROM incidents;
            DELETE FROM benchmarks;
            DELETE FROM audit_logs;
            VACUUM;
            ",
        )?;
        Ok(())
    }
}
