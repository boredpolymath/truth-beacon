//! Diagnostics, Process Lifecycle & Log Sanitization (Phase 6).
//!
//! Provides:
//! - Log sanitization stripping Discord bot tokens, webhook secrets, and credentials.
//! - Comprehensive health diagnostics reporting across memory, storage, gateway, and circuit breaker.
//! - Clean graceful application shutdown flushing WAL, stopping daemons, and releasing power assertions.

use serde::{Deserialize, Serialize};

/// Redacts Discord bot tokens, webhook tokens, and sensitive credential patterns from log strings.
pub fn sanitize_log_message(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    let mut output = input.to_string();

    // 1. Redact Discord Webhook URLs:
    // https://discord.com/api/webhooks/{id}/{token} or https://discordapp.com/api/webhooks/{id}/{token}
    for prefix in &[
        "https://discord.com/api/webhooks/",
        "https://discordapp.com/api/webhooks/",
        "discord.com/api/webhooks/",
        "discordapp.com/api/webhooks/",
    ] {
        while let Some(start_pos) = output.find(prefix) {
            let after_prefix = &output[start_pos + prefix.len()..];
            // Look for {id}/{token}
            if let Some(slash_idx) = after_prefix.find('/') {
                let token_start = start_pos + prefix.len() + slash_idx + 1;
                let token_part = &output[token_start..];
                let token_len = token_part
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .count();
                if token_len > 0 {
                    output.replace_range(
                        token_start..token_start + token_len,
                        "[REDACTED_WEBHOOK_TOKEN]",
                    );
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    // 2. Redact standard 3-segment Discord Bot Tokens:
    // Format: [23-35 base64 chars].[6 base64 chars].[25-45 base64 chars]
    // Also covers "Bot <token>" headers.
    let tokens_to_redact: Vec<String> = output
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|c: char| {
                c == '"'
                    || c == '\''
                    || c == '`'
                    || c == '('
                    || c == ')'
                    || c == '['
                    || c == ']'
                    || c == '{'
                    || c == '}'
                    || c == ','
                    || c == ';'
                    || c == ':'
            })
            .to_string()
        })
        .filter(|clean_word| is_discord_token(clean_word))
        .collect();

    for token in tokens_to_redact {
        output = output.replace(&token, "[REDACTED_BOT_TOKEN]");
    }

    output
}

/// Helper to identify if a string conforms to the Discord bot token structure:
/// segment1.segment2.segment3
fn is_discord_token(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return false;
    }

    let is_b64 = |p: &str| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };

    let p0 = parts[0];
    let p1 = parts[1];
    let p2 = parts[2];

    // Segment 1 (Base64 User ID): typically 20-35 characters
    // Segment 2 (Timestamp): typically 4-10 characters
    // Segment 3 (HMAC signature): typically 25-50 characters
    p0.len() >= 18
        && p0.len() <= 38
        && is_b64(p0)
        && p1.len() >= 4
        && p1.len() <= 12
        && is_b64(p1)
        && p2.len() >= 24
        && p2.len() <= 55
        && is_b64(p2)
}

/// Initializes `env_logger` with automatic credential and bot token redaction.
pub fn init_sanitized_logger() {
    let mut builder = env_logger::Builder::from_default_env();
    builder.format(|buf, record| {
        use std::io::Write;
        let ts = buf.timestamp();
        let raw_msg = record.args().to_string();
        let sanitized = sanitize_log_message(&raw_msg);
        writeln!(
            buf,
            "[{} {:5} {}] {}",
            ts,
            record.level(),
            record.target(),
            sanitized
        )
    });
    let _ = builder.try_init();
}

/// Comprehensive health diagnostic report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDiagnosticReport {
    pub resident_memory_bytes: u64,
    pub resident_memory_mb: f64,
    pub memory_budget_bytes: u64,
    pub memory_budget_adhered: bool,
    pub database_path: String,
    pub database_healthy: bool,
    pub pending_incidents_count: usize,
    pub benchmark_count: usize,
    pub active_gateway_connections: usize,
    pub registered_guilds_count: usize,
    pub gateway_connected: bool,
    pub circuit_breaker_status: String,
    pub circuit_breaker_tripped: bool,
    pub os_name: String,
}

/// Generates a live diagnostic health report across all subsystem states.
pub fn generate_health_diagnostics() -> HealthDiagnosticReport {
    let mem_bytes =
        crate::gateway::resilience::MemoryMonitor::get_resident_memory_bytes().unwrap_or(0);
    let mem_mb = crate::gateway::resilience::MemoryMonitor::get_resident_memory_mb().unwrap_or(0.0);
    let budget = crate::gateway::resilience::MAX_BACKGROUND_IDLE_RAM_BYTES;

    let storage_res = crate::storage::StorageManager::default_instance();
    let (db_path, db_healthy, pending_count, bm_count) = match storage_res {
        Ok(s) => {
            let path = s.get_path().to_string_lossy().to_string();
            let is_healthy = matches!(
                s.verify_integrity(),
                Ok(crate::storage::DatabaseIntegrityStatus::Healthy)
            );
            let pending = s.count_pending_incidents().unwrap_or(0);
            let bm = s.count_benchmarks().unwrap_or(0);
            (path, is_healthy, pending, bm)
        }
        Err(_) => (
            crate::storage::StorageManager::default_db_path()
                .to_string_lossy()
                .to_string(),
            false,
            0,
            0,
        ),
    };

    let registered_guilds = crate::credentials::CredentialManager::list_registered_guilds()
        .map(|g| g.len())
        .unwrap_or(0);

    let is_connected = crate::gateway::daemon::is_global_daemon_running_sync();
    let breaker = crate::circuit_breaker::get_global_circuit_breaker();

    HealthDiagnosticReport {
        resident_memory_bytes: mem_bytes,
        resident_memory_mb: mem_mb,
        memory_budget_bytes: budget,
        memory_budget_adhered: mem_bytes <= budget,
        database_path: db_path,
        database_healthy: db_healthy,
        pending_incidents_count: pending_count,
        benchmark_count: bm_count,
        active_gateway_connections: if is_connected {
            registered_guilds.max(1)
        } else {
            0
        },
        registered_guilds_count: registered_guilds,
        gateway_connected: is_connected,
        circuit_breaker_status: format!("{:?}", breaker.status().state),
        circuit_breaker_tripped: breaker.is_tripped(),
        os_name: std::env::consts::OS.to_string(),
    }
}

/// Executes clean application shutdown:
/// 1. Stops all active Discord Gateway daemons and releases OS power assertions.
/// 2. Flushes and truncates SQLite WAL to ensure zero data loss.
pub fn graceful_shutdown() {
    log::info!("Initiating TruthBeacon clean graceful application shutdown...");

    // 1. Terminate all active Discord Gateway connections
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| {
            handle.block_on(async {
                crate::gateway::daemon::stop_global_daemon().await;
            });
        });
    } else {
        tauri::async_runtime::block_on(async {
            crate::gateway::daemon::stop_global_daemon().await;
        });
    }

    // 2. Checkpoint and truncate SQLite WAL file to ensure zero data corruption
    if let Ok(storage) = crate::storage::StorageManager::default_instance() {
        match storage.checkpoint_on_shutdown() {
            Ok(res) => {
                log::info!(
                    "Clean shutdown WAL checkpoint complete: {} frames synchronized",
                    res.checkpointed_frames
                );
            }
            Err(e) => {
                log::error!("Error executing clean shutdown WAL checkpoint: {}", e);
            }
        }
    }

    log::info!("TruthBeacon clean application shutdown finished successfully.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_log_message_redacts_discord_bot_tokens() {
        // Construct synthetic test token dynamically so static secret scanners do not flag test fixtures
        let part1 = "SAMPLE_MOCK_USER_ID_TOKEN";
        let part2 = "SAMPLE";
        let part3 = "SAMPLE_HMAC_SIGNATURE_KEY_12345";
        let raw_token = format!("{}.{}.{}", part1, part2, part3);
        let log_line = format!("Connecting to Gateway with token: {}", raw_token);

        let sanitized = sanitize_log_message(&log_line);
        assert!(!sanitized.contains(&raw_token));
        assert!(sanitized.contains("[REDACTED_BOT_TOKEN]"));
        assert_eq!(
            sanitized,
            "Connecting to Gateway with token: [REDACTED_BOT_TOKEN]"
        );
    }

    #[test]
    fn test_sanitize_log_message_redacts_webhook_urls() {
        let sample_token = "SAMPLE_MOCK_WEBHOOK_SECRET_KEY";
        let webhook = format!(
            "https://discord.com/api/webhooks/123456789/{}",
            sample_token
        );
        let log_line = format!("Dispatched alert to webhook: {}", webhook);

        let sanitized = sanitize_log_message(&log_line);
        assert!(!sanitized.contains(sample_token));
        assert!(sanitized.contains("[REDACTED_WEBHOOK_TOKEN]"));
        assert!(sanitized
            .contains("https://discord.com/api/webhooks/123456789/[REDACTED_WEBHOOK_TOKEN]"));
    }

    #[test]
    fn test_sanitize_log_message_preserves_benign_logs() {
        let benign = "Processed GuildMemberUpdate event for user 111222333 in guild 444555666";
        let sanitized = sanitize_log_message(benign);
        assert_eq!(sanitized, benign);
    }

    #[test]
    fn test_generate_health_diagnostics_report() {
        let diag = generate_health_diagnostics();
        assert!(!diag.database_path.is_empty());
        assert!(!diag.os_name.is_empty());
        assert!(diag.memory_budget_bytes > 0);
    }

    #[test]
    fn test_graceful_shutdown_idempotent() {
        // Must execute without panic or error even if daemons are not running
        graceful_shutdown();
    }
}
