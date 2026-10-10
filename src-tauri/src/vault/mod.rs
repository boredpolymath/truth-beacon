//! Canonical Benchmark Identity Vault Manager & In-Memory Thread-Safe Cache.
//!
//! # Architecture & Persistence (Phase 12.1)
//! - **Thread-Safe In-Memory Cache**: `Arc<RwLock<HashMap<String, BenchmarkVault>>>` indexed by Guild ID
//!   for sub-millisecond evaluation lookups during ingress triage.
//! - **SQLite Backing Engine**: ACID persistence via `StorageManager`, synchronizing all additions,
//!   updates, and revocations with WAL mode and composite indices (`idx_benchmarks_guild_active`).
//! - **Multi-Guild Isolation & Status Filtering**: Independent partitioning per Discord server,
//!   with granular filtering by active status and community roles.

use crate::models::{CanonicalBenchmark, CreateBenchmarkInput, UpdateBenchmarkInput};
use crate::storage::StorageManager;
use rusqlite::params;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

pub mod avatar_sync;
pub mod ingestion;

pub use avatar_sync::*;
pub use ingestion::*;

static BENCHMARK_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Domain errors encountered during Vault operations.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Benchmark '{0}' not found")]
    NotFound(String),

    #[error("Duplicate benchmark: user '{0}' is already enrolled in guild '{1}'")]
    Duplicate(String, String),

    #[error("Unauthorized cross-guild access: benchmark '{0}' does not belong to guild '{1}'")]
    CrossGuildAccess(String, String),

    #[error("Lock contention or synchronization error: {0}")]
    Lock(String),
}

/// Container holding all cached canonical identity benchmarks for a single Discord Guild.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkVault {
    pub guild_id: String,
    pub benchmarks: HashMap<String, CanonicalBenchmark>,
}

impl BenchmarkVault {
    /// Create a new, empty benchmark vault for a specific Discord guild.
    pub fn new(guild_id: impl Into<String>) -> Self {
        Self {
            guild_id: guild_id.into(),
            benchmarks: HashMap::new(),
        }
    }

    /// Insert or update a benchmark in this guild's cache.
    pub fn insert(&mut self, benchmark: CanonicalBenchmark) {
        self.benchmarks.insert(benchmark.id.clone(), benchmark);
    }

    /// Remove a benchmark from this guild's cache by ID.
    pub fn remove(&mut self, id: &str) -> Option<CanonicalBenchmark> {
        self.benchmarks.remove(id)
    }

    /// Retrieve a reference to a benchmark by its internal ID.
    pub fn get(&self, id: &str) -> Option<&CanonicalBenchmark> {
        self.benchmarks.get(id)
    }

    /// Find a benchmark matching a specific Discord user Snowflake ID.
    pub fn get_by_user_id(&self, user_id: &str) -> Option<&CanonicalBenchmark> {
        self.benchmarks.values().find(|b| b.user_id == user_id)
    }

    /// Returns a list of benchmarks, optionally filtered by active status.
    pub fn list(&self, active_only: bool) -> Vec<CanonicalBenchmark> {
        let mut list: Vec<_> = self
            .benchmarks
            .values()
            .filter(|b| !active_only || b.is_active)
            .cloned()
            .collect();
        list.sort_by_key(|b| b.created_at);
        list
    }

    /// Total count of benchmarks in this guild vault.
    pub fn len(&self) -> usize {
        self.benchmarks.len()
    }

    /// Returns true if no benchmarks are cached for this guild.
    pub fn is_empty(&self) -> bool {
        self.benchmarks.is_empty()
    }
}

/// Thread-safe manager orchestrating SQLite persistent storage and the in-memory benchmark cache.
#[derive(Clone)]
pub struct VaultManager {
    cache: Arc<RwLock<HashMap<String, BenchmarkVault>>>,
    storage: Arc<StorageManager>,
}

impl VaultManager {
    /// Initialize the Vault Manager with backing storage, hydrating the cache from SQLite.
    pub fn new(storage: Arc<StorageManager>) -> Result<Self, VaultError> {
        let manager = Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            storage,
        };
        manager.reload_cache()?;
        Ok(manager)
    }

    /// Convenience constructor initializing an ephemeral, in-memory vault for testing or sandbox simulations.
    pub fn in_memory() -> Result<Self, VaultError> {
        let storage = Arc::new(StorageManager::in_memory()?);
        Self::new(storage)
    }

    /// Resolves and initializes default local VaultManager using the primary local SQLite database.
    pub fn default_instance() -> Result<Self, VaultError> {
        let storage = Arc::new(StorageManager::default_instance()?);
        Self::new(storage)
    }

    /// Access the shared thread-safe in-memory cache (`Arc<RwLock<HashMap<String, BenchmarkVault>>>`).
    pub fn cache(&self) -> Arc<RwLock<HashMap<String, BenchmarkVault>>> {
        Arc::clone(&self.cache)
    }

    /// Access the underlying storage manager handle.
    pub fn storage(&self) -> Arc<StorageManager> {
        Arc::clone(&self.storage)
    }

    /// Reload the entire in-memory benchmark cache from SQLite persistence.
    pub fn reload_cache(&self) -> Result<(), VaultError> {
        let reader = self.storage.open_reader()?;
        let mut stmt = reader.prepare(
            "SELECT id, guild_id, user_id, canonical_username, server_nickname, community_role,
                    avatar_url, avatar_perceptual_hash, is_active, tags, created_at, updated_at,
                    sensitivity_override
             FROM benchmarks
             ORDER BY created_at ASC;",
        )?;

        let benchmark_iter = stmt.query_map([], row_to_benchmark)?;

        let mut fresh_cache: HashMap<String, BenchmarkVault> = HashMap::new();
        for b in benchmark_iter {
            let benchmark = b?;
            fresh_cache
                .entry(benchmark.guild_id.clone())
                .or_insert_with(|| BenchmarkVault::new(&benchmark.guild_id))
                .insert(benchmark);
        }

        let mut cache_lock = self
            .cache
            .write()
            .map_err(|e| VaultError::Lock(e.to_string()))?;
        *cache_lock = fresh_cache;
        Ok(())
    }

    /// Validate and persist a new canonical benchmark profile to SQLite and the in-memory cache.
    pub fn create_benchmark(
        &self,
        input: CreateBenchmarkInput,
        avatar_hash: Option<String>,
    ) -> Result<CanonicalBenchmark, VaultError> {
        let guild_id = input.guild_id.trim();
        if guild_id.is_empty() {
            return Err(VaultError::Validation(
                "Discord Guild ID cannot be empty".into(),
            ));
        }

        let user_id = input.user_id.trim();
        if user_id.is_empty() {
            return Err(VaultError::Validation(
                "Discord User Snowflake ID cannot be empty".into(),
            ));
        }
        if !user_id.chars().all(|c| c.is_ascii_digit()) {
            return Err(VaultError::Validation(
                "Discord User Snowflake ID must consist only of numeric digits".into(),
            ));
        }

        let canonical_username = input.canonical_username.trim();
        if canonical_username.is_empty() {
            return Err(VaultError::Validation(
                "Canonical username cannot be empty".into(),
            ));
        }

        // Prevent duplicate benchmark profile for the same user within the same guild
        if self.get_benchmark_by_user(guild_id, user_id)?.is_some() {
            return Err(VaultError::Duplicate(
                user_id.to_string(),
                guild_id.to_string(),
            ));
        }

        let now = chrono::Utc::now().timestamp();
        let counter = BENCHMARK_COUNTER.fetch_add(1, Ordering::Relaxed);
        let id = format!("bm_{}_{}", now, counter);
        let tags_json = serde_json::to_string(&input.tags)?;

        let benchmark = CanonicalBenchmark {
            id: id.clone(),
            guild_id: guild_id.to_string(),
            user_id: user_id.to_string(),
            canonical_username: canonical_username.to_string(),
            server_nickname: input.server_nickname.map(|s| s.trim().to_string()),
            community_role: input.community_role.trim().to_string(),
            avatar_url: input.avatar_url.map(|s| s.trim().to_string()),
            avatar_perceptual_hash: avatar_hash,
            is_active: true,
            tags: input.tags,
            created_at: now,
            updated_at: now,
            sensitivity_override: input.sensitivity_override,
        };

        // Persist to SQLite
        {
            let conn = self.storage.get_connection();
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO benchmarks (
                    id, guild_id, user_id, canonical_username, server_nickname, community_role,
                    avatar_url, avatar_perceptual_hash, is_active, tags, created_at, updated_at,
                    sensitivity_override
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);",
                params![
                    benchmark.id,
                    benchmark.guild_id,
                    benchmark.user_id,
                    benchmark.canonical_username,
                    benchmark.server_nickname,
                    benchmark.community_role,
                    benchmark.avatar_url,
                    benchmark.avatar_perceptual_hash,
                    if benchmark.is_active { 1 } else { 0 },
                    tags_json,
                    benchmark.created_at,
                    benchmark.updated_at,
                    benchmark.sensitivity_override,
                ],
            )?;
        }

        // Hydrate in-memory cache
        {
            let mut cache = self
                .cache
                .write()
                .map_err(|e| VaultError::Lock(e.to_string()))?;
            cache
                .entry(benchmark.guild_id.clone())
                .or_insert_with(|| BenchmarkVault::new(&benchmark.guild_id))
                .insert(benchmark.clone());
        }

        Ok(benchmark)
    }

    /// Retrieve a benchmark by ID, checking the in-memory cache before falling back to SQLite.
    pub fn get_benchmark(&self, id: &str) -> Result<Option<CanonicalBenchmark>, VaultError> {
        // Fast path: search in-memory cache
        {
            let cache = self
                .cache
                .read()
                .map_err(|e| VaultError::Lock(e.to_string()))?;
            for vault in cache.values() {
                if let Some(b) = vault.get(id) {
                    return Ok(Some(b.clone()));
                }
            }
        }

        // Fallback: query SQLite
        let reader = self.storage.open_reader()?;
        let mut stmt = reader.prepare(
            "SELECT id, guild_id, user_id, canonical_username, server_nickname, community_role,
                    avatar_url, avatar_perceptual_hash, is_active, tags, created_at, updated_at,
                    sensitivity_override
             FROM benchmarks
             WHERE id = ?1 LIMIT 1;",
        )?;

        let result = stmt.query_row([id], row_to_benchmark);
        match result {
            Ok(b) => Ok(Some(b)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(VaultError::Database(e)),
        }
    }

    /// Retrieve a benchmark for a specific user Snowflake in a given guild.
    pub fn get_benchmark_by_user(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<Option<CanonicalBenchmark>, VaultError> {
        // Fast path: check in-memory cache for this guild
        {
            let cache = self
                .cache
                .read()
                .map_err(|e| VaultError::Lock(e.to_string()))?;
            if let Some(vault) = cache.get(guild_id) {
                if let Some(b) = vault.get_by_user_id(user_id) {
                    return Ok(Some(b.clone()));
                }
            }
        }

        // Fallback: query SQLite
        let reader = self.storage.open_reader()?;
        let mut stmt = reader.prepare(
            "SELECT id, guild_id, user_id, canonical_username, server_nickname, community_role,
                    avatar_url, avatar_perceptual_hash, is_active, tags, created_at, updated_at,
                    sensitivity_override
             FROM benchmarks
             WHERE guild_id = ?1 AND user_id = ?2 LIMIT 1;",
        )?;

        let result = stmt.query_row([guild_id, user_id], row_to_benchmark);
        match result {
            Ok(b) => Ok(Some(b)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(VaultError::Database(e)),
        }
    }

    /// List benchmarks for a given guild ID, optionally filtered by active status.
    pub fn list_benchmarks(
        &self,
        guild_id: &str,
        active_only: bool,
    ) -> Result<Vec<CanonicalBenchmark>, VaultError> {
        let cache = self
            .cache
            .read()
            .map_err(|e| VaultError::Lock(e.to_string()))?;
        if let Some(vault) = cache.get(guild_id) {
            Ok(vault.list(active_only))
        } else {
            Ok(vec![])
        }
    }

    /// List all benchmarks across all guilds, optionally filtered by active status.
    pub fn list_all_benchmarks(
        &self,
        active_only: bool,
    ) -> Result<Vec<CanonicalBenchmark>, VaultError> {
        let cache = self
            .cache
            .read()
            .map_err(|e| VaultError::Lock(e.to_string()))?;
        let mut all = Vec::new();
        for vault in cache.values() {
            all.extend(vault.list(active_only));
        }
        all.sort_by_key(|b| b.created_at);
        Ok(all)
    }

    /// Update an existing benchmark profile, persisting changes to SQLite and refreshing the cache.
    pub fn update_benchmark(
        &self,
        id: &str,
        input: UpdateBenchmarkInput,
    ) -> Result<CanonicalBenchmark, VaultError> {
        let mut current = self
            .get_benchmark(id)?
            .ok_or_else(|| VaultError::NotFound(id.to_string()))?;

        if let Some(nick) = input.server_nickname {
            current.server_nickname = if nick.trim().is_empty() {
                None
            } else {
                Some(nick.trim().to_string())
            };
        }
        if let Some(role) = input.community_role {
            current.community_role = role.trim().to_string();
        }
        if let Some(url) = input.avatar_url {
            current.avatar_url = if url.trim().is_empty() {
                None
            } else {
                Some(url.trim().to_string())
            };
        }
        if let Some(hash) = input.avatar_perceptual_hash {
            current.avatar_perceptual_hash = if hash.trim().is_empty() {
                None
            } else {
                Some(hash.trim().to_string())
            };
        }
        if let Some(active) = input.is_active {
            current.is_active = active;
        }
        if let Some(tags) = input.tags {
            current.tags = tags;
        }
        if let Some(sensitivity) = input.sensitivity_override {
            current.sensitivity_override = Some(sensitivity);
        }

        current.updated_at = chrono::Utc::now().timestamp();
        let tags_json = serde_json::to_string(&current.tags)?;

        // Update in SQLite
        {
            let conn = self.storage.get_connection();
            let conn = conn.lock().unwrap();
            conn.execute(
                "UPDATE benchmarks SET
                    server_nickname = ?1,
                    community_role = ?2,
                    avatar_url = ?3,
                    avatar_perceptual_hash = ?4,
                    is_active = ?5,
                    tags = ?6,
                    updated_at = ?7,
                    sensitivity_override = ?8
                 WHERE id = ?9;",
                params![
                    current.server_nickname,
                    current.community_role,
                    current.avatar_url,
                    current.avatar_perceptual_hash,
                    if current.is_active { 1 } else { 0 },
                    tags_json,
                    current.updated_at,
                    current.sensitivity_override,
                    current.id,
                ],
            )?;
        }

        // Update in cache
        {
            let mut cache = self
                .cache
                .write()
                .map_err(|e| VaultError::Lock(e.to_string()))?;
            if let Some(vault) = cache.get_mut(&current.guild_id) {
                vault.insert(current.clone());
            }
        }

        Ok(current)
    }

    /// Toggle or set the active status of a benchmark profile.
    pub fn set_active_status(
        &self,
        id: &str,
        is_active: bool,
    ) -> Result<CanonicalBenchmark, VaultError> {
        self.update_benchmark(
            id,
            UpdateBenchmarkInput {
                is_active: Some(is_active),
                ..Default::default()
            },
        )
    }

    /// Delete a benchmark from SQLite persistence and the in-memory cache.
    pub fn delete_benchmark(&self, id: &str) -> Result<bool, VaultError> {
        let existing = self.get_benchmark(id)?;
        let benchmark = match existing {
            Some(b) => b,
            None => return Ok(false),
        };

        // Delete from SQLite
        {
            let conn = self.storage.get_connection();
            let conn = conn.lock().unwrap();
            let rows_affected = conn.execute("DELETE FROM benchmarks WHERE id = ?1;", [id])?;
            if rows_affected == 0 {
                return Ok(false);
            }
        }

        // Delete from in-memory cache
        {
            let mut cache = self
                .cache
                .write()
                .map_err(|e| VaultError::Lock(e.to_string()))?;
            if let Some(vault) = cache.get_mut(&benchmark.guild_id) {
                vault.remove(id);
            }
        }

        Ok(true)
    }

    /// Retrieve a benchmark by ID strictly scoped to a specific guild, preventing unauthorized cross-guild access.
    pub fn get_benchmark_in_guild(
        &self,
        guild_id: &str,
        id: &str,
    ) -> Result<CanonicalBenchmark, VaultError> {
        let benchmark = self
            .get_benchmark(id)?
            .ok_or_else(|| VaultError::NotFound(id.to_string()))?;

        if benchmark.guild_id != guild_id {
            return Err(VaultError::CrossGuildAccess(
                id.to_string(),
                guild_id.to_string(),
            ));
        }

        Ok(benchmark)
    }

    /// Update a benchmark by ID strictly scoped to a specific guild, preventing unauthorized cross-guild access.
    pub fn update_benchmark_in_guild(
        &self,
        guild_id: &str,
        id: &str,
        input: UpdateBenchmarkInput,
    ) -> Result<CanonicalBenchmark, VaultError> {
        let existing = self
            .get_benchmark(id)?
            .ok_or_else(|| VaultError::NotFound(id.to_string()))?;

        if existing.guild_id != guild_id {
            return Err(VaultError::CrossGuildAccess(
                id.to_string(),
                guild_id.to_string(),
            ));
        }

        self.update_benchmark(id, input)
    }

    /// Delete a benchmark by ID strictly scoped to a specific guild, preventing unauthorized cross-guild access.
    pub fn delete_benchmark_in_guild(&self, guild_id: &str, id: &str) -> Result<bool, VaultError> {
        let existing = match self.get_benchmark(id)? {
            Some(b) => b,
            None => return Ok(false),
        };

        if existing.guild_id != guild_id {
            return Err(VaultError::CrossGuildAccess(
                id.to_string(),
                guild_id.to_string(),
            ));
        }

        self.delete_benchmark(id)
    }

    /// Count benchmarks in a guild matching the active filter.
    pub fn count(&self, guild_id: &str, active_only: bool) -> usize {
        let cache = match self.cache.read() {
            Ok(c) => c,
            Err(_) => return 0,
        };
        cache
            .get(guild_id)
            .map(|v| v.list(active_only).len())
            .unwrap_or(0)
    }

    /// Add an official taxonomy tag to a benchmark profile and persist the update (Phase 12.3).
    pub fn add_taxonomy_tag(
        &self,
        id: &str,
        tag: crate::models::TaxonomyTag,
    ) -> Result<CanonicalBenchmark, VaultError> {
        let current = self
            .get_benchmark(id)?
            .ok_or_else(|| VaultError::NotFound(id.to_string()))?;

        let mut tags = current.tags;
        let tag_str = tag.as_str().to_string();
        if !tags.contains(&tag_str) {
            tags.push(tag_str);
        }

        self.update_benchmark(
            id,
            UpdateBenchmarkInput {
                tags: Some(tags),
                ..Default::default()
            },
        )
    }

    /// Link a whitelisted alt account Discord snowflake directly to a primary benchmark (Phase 12.3).
    pub fn link_whitelisted_alt(
        &self,
        primary_benchmark_id: &str,
        alt_user_id: &str,
    ) -> Result<CanonicalBenchmark, VaultError> {
        let current = self
            .get_benchmark(primary_benchmark_id)?
            .ok_or_else(|| VaultError::NotFound(primary_benchmark_id.to_string()))?;

        let mut tags = current.tags;
        let alt_tag = format!("Whitelisted Alt: {}", alt_user_id.trim());
        if !tags.contains(&alt_tag) {
            tags.push(alt_tag);
        }

        self.update_benchmark(
            primary_benchmark_id,
            UpdateBenchmarkInput {
                tags: Some(tags),
                ..Default::default()
            },
        )
    }

    /// Filter benchmarks for a given guild ID by a specific canonical taxonomy tag (Phase 12.3).
    pub fn list_by_taxonomy_tag(
        &self,
        guild_id: &str,
        tag: crate::models::TaxonomyTag,
        active_only: bool,
    ) -> Result<Vec<CanonicalBenchmark>, VaultError> {
        let list = self.list_benchmarks(guild_id, active_only)?;
        Ok(list
            .into_iter()
            .filter(|b| b.has_taxonomy_tag(tag))
            .collect())
    }

    /// List all benchmarks in a guild that are automated exemption targets (Authorized Alt or Approved Satire).
    pub fn list_exempt_benchmarks(
        &self,
        guild_id: &str,
    ) -> Result<Vec<CanonicalBenchmark>, VaultError> {
        let list = self.list_benchmarks(guild_id, true)?;
        Ok(list.into_iter().filter(|b| b.is_exempt()).collect())
    }
}

/// Helper mapping an SQLite row into a `CanonicalBenchmark` model instance.
fn row_to_benchmark(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalBenchmark> {
    let id: String = row.get(0)?;
    let guild_id: String = row.get(1)?;
    let user_id: String = row.get(2)?;
    let canonical_username: String = row.get(3)?;
    let server_nickname: Option<String> = row.get(4)?;
    let community_role: String = row.get(5)?;
    let avatar_url: Option<String> = row.get(6)?;
    let avatar_perceptual_hash: Option<String> = row.get(7)?;
    let is_active_int: i64 = row.get(8)?;
    let tags_raw: Option<String> = row.get(9)?;
    let created_at: i64 = row.get(10)?;
    let updated_at: i64 = row.get(11)?;
    let sensitivity_override: Option<f64> = row.get(12)?;

    let tags: Vec<String> = tags_raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    Ok(CanonicalBenchmark {
        id,
        guild_id,
        user_id,
        canonical_username,
        server_nickname,
        community_role,
        avatar_url,
        avatar_perceptual_hash,
        is_active: is_active_int != 0,
        tags,
        created_at,
        updated_at,
        sensitivity_override,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_manager_crud_and_sqlite_persistence() {
        let vault_mgr = VaultManager::in_memory().expect("Failed to initialize in-memory vault");

        // 1. Create a benchmark
        let input = CreateBenchmarkInput {
            guild_id: "guild_123456789".into(),
            user_id: "987654321098765432".into(),
            canonical_username: "PastorJohn".into(),
            server_nickname: Some("John | Pastor".into()),
            community_role: "Executive Leadership".into(),
            avatar_url: Some("https://cdn.discordapp.com/avatars/1/a.png".into()),
            tags: vec!["Core Staff".into(), "Verified VIP".into()],
            sensitivity_override: Some(0.95),
        };

        let created = vault_mgr
            .create_benchmark(input, Some("abcdef1234567890".into()))
            .expect("Failed to create benchmark");

        assert_eq!(created.guild_id, "guild_123456789");
        assert_eq!(created.user_id, "987654321098765432");
        assert_eq!(created.canonical_username, "PastorJohn");
        assert_eq!(created.server_nickname.as_deref(), Some("John | Pastor"));
        assert_eq!(
            created.avatar_perceptual_hash.as_deref(),
            Some("abcdef1234567890")
        );
        assert!(created.is_active);
        assert_eq!(created.sensitivity_override, Some(0.95));

        // 2. Read from in-memory cache
        let cached = vault_mgr
            .get_benchmark(&created.id)
            .expect("Lookup error")
            .expect("Benchmark should be cached");
        assert_eq!(cached.id, created.id);

        let by_user = vault_mgr
            .get_benchmark_by_user("guild_123456789", "987654321098765432")
            .expect("Lookup error")
            .expect("Benchmark should be found by user ID");
        assert_eq!(by_user.id, created.id);

        // 3. Verify SQLite persistence by directly querying a new reader
        let reader = vault_mgr.storage.open_reader().unwrap();
        let db_active: i64 = reader
            .query_row(
                "SELECT is_active FROM benchmarks WHERE id = ?1",
                [&created.id],
                |r| r.get(0),
            )
            .expect("Row should exist in SQLite");
        assert_eq!(db_active, 1);

        // 4. Update the benchmark
        let updated = vault_mgr
            .update_benchmark(
                &created.id,
                UpdateBenchmarkInput {
                    community_role: Some("Senior Pastor".into()),
                    is_active: Some(false),
                    tags: Some(vec!["Core Staff".into(), "Emeritus".into()]),
                    ..Default::default()
                },
            )
            .expect("Update failed");

        assert_eq!(updated.community_role, "Senior Pastor");
        assert!(!updated.is_active);
        assert_eq!(updated.tags, vec!["Core Staff", "Emeritus"]);

        // Verify SQLite row was updated
        let reader_updated = vault_mgr.storage.open_reader().unwrap();
        let (db_role, db_active): (String, i64) = reader_updated
            .query_row(
                "SELECT community_role, is_active FROM benchmarks WHERE id = ?1",
                [&created.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("Query failed");
        assert_eq!(db_role, "Senior Pastor");
        assert_eq!(db_active, 0);

        // 5. Delete benchmark
        let deleted = vault_mgr
            .delete_benchmark(&created.id)
            .expect("Delete error");
        assert!(deleted);

        // Verify benchmark is absent from both cache and SQLite
        assert!(vault_mgr
            .get_benchmark(&created.id)
            .expect("Lookup error")
            .is_none());
        let reader_deleted = vault_mgr.storage.open_reader().unwrap();
        let exists: bool = reader_deleted
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM benchmarks WHERE id = ?1)",
                [&created.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!exists);
    }

    #[test]
    fn test_vault_filtering_by_guild_id_and_active_status() {
        let vault_mgr = VaultManager::in_memory().unwrap();
        let guild_a = "guild_alpha";
        let guild_b = "guild_beta";

        // Create 2 active, 1 inactive in Guild A
        let bm1 = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_a.into(),
                    user_id: "1001".into(),
                    canonical_username: "UserA1".into(),
                    server_nickname: None,
                    community_role: "Mod".into(),
                    avatar_url: None,
                    tags: vec![],
                    sensitivity_override: None,
                },
                None,
            )
            .unwrap();

        let _bm2 = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_a.into(),
                    user_id: "1002".into(),
                    canonical_username: "UserA2".into(),
                    server_nickname: None,
                    community_role: "Admin".into(),
                    avatar_url: None,
                    tags: vec![],
                    sensitivity_override: None,
                },
                None,
            )
            .unwrap();

        let bm3 = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_a.into(),
                    user_id: "1003".into(),
                    canonical_username: "UserA3".into(),
                    server_nickname: None,
                    community_role: "Staff".into(),
                    avatar_url: None,
                    tags: vec![],
                    sensitivity_override: None,
                },
                None,
            )
            .unwrap();

        // Deactivate bm1
        vault_mgr.set_active_status(&bm1.id, false).unwrap();

        // Create 1 active in Guild B
        let _bm_b1 = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_b.into(),
                    user_id: "2001".into(),
                    canonical_username: "UserB1".into(),
                    server_nickname: None,
                    community_role: "Lead".into(),
                    avatar_url: None,
                    tags: vec![],
                    sensitivity_override: None,
                },
                None,
            )
            .unwrap();

        // Test filtering Guild A
        let active_a = vault_mgr.list_benchmarks(guild_a, true).unwrap();
        assert_eq!(active_a.len(), 2);
        assert!(active_a.iter().all(|b| b.is_active));
        assert!(active_a.iter().any(|b| b.id == bm3.id));

        let all_a = vault_mgr.list_benchmarks(guild_a, false).unwrap();
        assert_eq!(all_a.len(), 3);

        // Test filtering Guild B
        let active_b = vault_mgr.list_benchmarks(guild_b, true).unwrap();
        assert_eq!(active_b.len(), 1);
        assert_eq!(active_b[0].user_id, "2001");

        // Test non-existent guild
        let active_empty = vault_mgr.list_benchmarks("guild_none", true).unwrap();
        assert!(active_empty.is_empty());

        // Test list_all_benchmarks
        let all_active = vault_mgr.list_all_benchmarks(true).unwrap();
        assert_eq!(all_active.len(), 3); // 2 in A + 1 in B
    }

    #[test]
    fn test_vault_cache_hydration_from_storage() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tb_vault_test_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let db_path = temp_dir.join("vault_hydrate.db");

        // Phase 1: Initialize vault on disk and add a benchmark
        {
            let storage = Arc::new(StorageManager::init(&db_path).unwrap());
            let vault_mgr = VaultManager::new(storage).unwrap();

            vault_mgr
                .create_benchmark(
                    CreateBenchmarkInput {
                        guild_id: "guild_persistent".into(),
                        user_id: "777888999".into(),
                        canonical_username: "PersistentLeader".into(),
                        server_nickname: Some("Leader".into()),
                        community_role: "Owner".into(),
                        avatar_url: None,
                        tags: vec!["VIP".into()],
                        sensitivity_override: Some(0.85),
                    },
                    Some("hash123".into()),
                )
                .unwrap();
        }

        // Phase 2: Open a fresh VaultManager on the same SQLite file (simulating app restart)
        {
            let storage2 = Arc::new(StorageManager::init(&db_path).unwrap());
            let vault_mgr2 = VaultManager::new(storage2).unwrap();

            // Cache should be automatically hydrated on init
            let list = vault_mgr2
                .list_benchmarks("guild_persistent", true)
                .unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].canonical_username, "PersistentLeader");
            assert_eq!(list[0].user_id, "777888999");
            assert_eq!(list[0].avatar_perceptual_hash.as_deref(), Some("hash123"));
            assert_eq!(list[0].sensitivity_override, Some(0.85));
        }

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_vault_validation_failures() {
        let vault_mgr = VaultManager::in_memory().unwrap();

        // Empty guild ID
        let res = vault_mgr.create_benchmark(
            CreateBenchmarkInput {
                guild_id: "   ".into(),
                user_id: "123".into(),
                canonical_username: "Name".into(),
                server_nickname: None,
                community_role: "Role".into(),
                avatar_url: None,
                tags: vec![],
                sensitivity_override: None,
            },
            None,
        );
        assert!(matches!(res, Err(VaultError::Validation(msg)) if msg.contains("Guild ID")));

        // Non-numeric user snowflake
        let res = vault_mgr.create_benchmark(
            CreateBenchmarkInput {
                guild_id: "guild_1".into(),
                user_id: "not_a_snowflake".into(),
                canonical_username: "Name".into(),
                server_nickname: None,
                community_role: "Role".into(),
                avatar_url: None,
                tags: vec![],
                sensitivity_override: None,
            },
            None,
        );
        assert!(matches!(res, Err(VaultError::Validation(msg)) if msg.contains("numeric digits")));

        // Empty canonical username
        let res = vault_mgr.create_benchmark(
            CreateBenchmarkInput {
                guild_id: "guild_1".into(),
                user_id: "123456789".into(),
                canonical_username: "".into(),
                server_nickname: None,
                community_role: "Role".into(),
                avatar_url: None,
                tags: vec![],
                sensitivity_override: None,
            },
            None,
        );
        assert!(
            matches!(res, Err(VaultError::Validation(msg)) if msg.contains("Canonical username"))
        );
    }

    #[test]
    fn test_taxonomy_tagging_and_alt_linking() {
        use crate::models::TaxonomyTag;

        let vault_mgr = VaultManager::in_memory().unwrap();
        let guild_id = "guild_tax_test";

        // Create initial benchmark
        let bm = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_id.into(),
                    user_id: "111222333".into(),
                    canonical_username: "CreatorSam".into(),
                    server_nickname: None,
                    community_role: "Streamer".into(),
                    avatar_url: None,
                    tags: vec![],
                    sensitivity_override: None,
                },
                None,
            )
            .unwrap();

        assert!(!bm.has_taxonomy_tag(TaxonomyTag::PublicCreator));
        assert!(!bm.is_exempt());

        // 1. Add Taxonomy Tag: Public Creator
        let updated = vault_mgr
            .add_taxonomy_tag(&bm.id, TaxonomyTag::PublicCreator)
            .unwrap();
        assert!(updated.has_taxonomy_tag(TaxonomyTag::PublicCreator));
        assert!(updated.tags.contains(&"Public Creator".to_string()));
        assert!(!updated.is_exempt()); // Public Creator is protected, not exempted

        // 2. Link Whitelisted Alt
        let linked = vault_mgr.link_whitelisted_alt(&bm.id, "999000111").unwrap();
        assert!(linked.whitelists_alt_user("999000111"));
        assert!(!linked.whitelists_alt_user("888000222"));

        // 3. Create an exempt Authorized Alt benchmark
        let alt_bm = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_id.into(),
                    user_id: "999000111".into(),
                    canonical_username: "CreatorSam Alt".into(),
                    server_nickname: None,
                    community_role: "Authorized Alt".into(),
                    avatar_url: None,
                    tags: vec!["Authorized Alt".into()],
                    sensitivity_override: None,
                },
                None,
            )
            .unwrap();

        assert!(alt_bm.is_exempt());
        assert!(alt_bm.has_taxonomy_tag(TaxonomyTag::AuthorizedAlt));

        // 4. Test filtering by taxonomy tag
        let creators = vault_mgr
            .list_by_taxonomy_tag(guild_id, TaxonomyTag::PublicCreator, true)
            .unwrap();
        assert_eq!(creators.len(), 1);
        assert_eq!(creators[0].id, bm.id);

        let alts = vault_mgr
            .list_by_taxonomy_tag(guild_id, TaxonomyTag::AuthorizedAlt, true)
            .unwrap();
        assert_eq!(alts.len(), 1);
        assert_eq!(alts[0].id, alt_bm.id);

        // 5. Test listing exempt benchmarks
        let exempt_list = vault_mgr.list_exempt_benchmarks(guild_id).unwrap();
        assert_eq!(exempt_list.len(), 1);
        assert_eq!(exempt_list[0].id, alt_bm.id);
    }

    #[test]
    fn test_cross_guild_access_prevention() {
        let vault_mgr = VaultManager::in_memory().unwrap();
        let guild_a = "guild_security_a";
        let guild_b = "guild_security_b";

        let created_a = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_a.into(),
                    user_id: "555111".into(),
                    canonical_username: "GuildA_Admin".into(),
                    server_nickname: None,
                    community_role: "Admin".into(),
                    avatar_url: None,
                    tags: vec![],
                    sensitivity_override: None,
                },
                None,
            )
            .unwrap();

        // 1. Guild A can retrieve its own benchmark
        let retrieved = vault_mgr
            .get_benchmark_in_guild(guild_a, &created_a.id)
            .unwrap();
        assert_eq!(retrieved.id, created_a.id);

        // 2. Guild B cannot retrieve Guild A's benchmark
        let err_get = vault_mgr.get_benchmark_in_guild(guild_b, &created_a.id);
        assert!(matches!(
            err_get,
            Err(VaultError::CrossGuildAccess(id, gid)) if id == created_a.id && gid == guild_b
        ));

        // 3. Guild B cannot update Guild A's benchmark
        let err_up = vault_mgr.update_benchmark_in_guild(
            guild_b,
            &created_a.id,
            UpdateBenchmarkInput::default(),
        );
        assert!(matches!(err_up, Err(VaultError::CrossGuildAccess(..))));

        // 4. Guild B cannot delete Guild A's benchmark
        let err_del = vault_mgr.delete_benchmark_in_guild(guild_b, &created_a.id);
        assert!(matches!(err_del, Err(VaultError::CrossGuildAccess(..))));

        // Benchmark must still exist and be intact
        assert!(vault_mgr.get_benchmark(&created_a.id).unwrap().is_some());

        // 5. Deletion in authorized Guild A succeeds
        assert!(vault_mgr
            .delete_benchmark_in_guild(guild_a, &created_a.id)
            .unwrap());
        assert!(vault_mgr.get_benchmark(&created_a.id).unwrap().is_none());
    }

    #[test]
    fn test_duplicate_benchmark_rejection() {
        let vault_mgr = VaultManager::in_memory().unwrap();
        let guild_id = "guild_dup_test";

        let input = CreateBenchmarkInput {
            guild_id: guild_id.into(),
            user_id: "999888".into(),
            canonical_username: "Original".into(),
            server_nickname: None,
            community_role: "Staff".into(),
            avatar_url: None,
            tags: vec![],
            sensitivity_override: None,
        };

        // First creation succeeds
        assert!(vault_mgr.create_benchmark(input.clone(), None).is_ok());

        // Second creation for same user in same guild must fail with Duplicate error
        let err = vault_mgr.create_benchmark(input, None);
        assert!(matches!(
            err,
            Err(VaultError::Duplicate(u, g)) if u == "999888" && g == guild_id
        ));
    }
}
