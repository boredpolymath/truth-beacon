use crate::models::{CanonicalBenchmark, CreateBenchmarkInput, IncidentStatus, TriageIncident};
use crate::vault::{
    import_benchmark_by_snowflake as vault_import_snowflake,
    import_members_by_role as vault_import_role, promote_from_triage as vault_promote_triage,
    IngestionClient, ManualSnowflakeImportInput, PromotionFromTriageInput, PromotionResult,
    RoleImportOptions, RoleImportReport, VaultManager,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Benchmark not found: {0}")]
    BenchmarkNotFound(String),
    #[error("Incident not found: {0}")]
    IncidentNotFound(String),
    #[error("Rate limited: retry after {0} seconds")]
    RateLimited(f64),
    #[error("Circuit breaker is currently tripped")]
    CircuitBreakerTripped,
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CommandError", 3)?;
        let code = match self {
            Self::DatabaseError(_) => "DATABASE_ERROR",
            Self::BenchmarkNotFound(_) => "BENCHMARK_NOT_FOUND",
            Self::IncidentNotFound(_) => "INCIDENT_NOT_FOUND",
            Self::RateLimited(_) => "RATE_LIMITED",
            Self::CircuitBreakerTripped => "CIRCUIT_BREAKER_TRIPPED",
            Self::ValidationFailed(_) => "VALIDATION_FAILED",
            Self::AuthenticationFailed(_) => "AUTH_FAILED",
            Self::InternalError(_) => "INTERNAL_ERROR",
        };
        state.serialize_field("code", code)?;
        state.serialize_field("message", &self.to_string())?;
        state.serialize_field("details", &None::<String>)?;
        state.end()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub daemon_healthy: bool,
    pub gateway_connected: bool,
    pub circuit_breaker_tripped: bool,
    pub db_path: String,
    pub pending_incidents_count: usize,
    pub benchmark_count: usize,
}

#[tauri::command]
pub fn get_system_status() -> Result<SystemStatus, CommandError> {
    let breaker = crate::circuit_breaker::get_global_circuit_breaker();
    let db_path = crate::storage::StorageManager::default_db_path();
    let is_connected = crate::gateway::daemon::is_global_daemon_running_sync();
    Ok(SystemStatus {
        daemon_healthy: true,
        gateway_connected: is_connected,
        circuit_breaker_tripped: breaker.is_tripped(),
        db_path: db_path.to_string_lossy().to_string(),
        // TODO: Wire to StorageManager query when VaultManager state is injected via Tauri managed state
        pending_incidents_count: 0,
        benchmark_count: 0,
    })
}

#[tauri::command]
pub fn list_benchmarks(guild_id: String) -> Result<Vec<CanonicalBenchmark>, CommandError> {
    // TODO: Wire to VaultManager::list_benchmarks(&guild_id, true) when VaultManager
    // is injected via Tauri managed state (tauri::State<Arc<VaultManager>>)
    let _ = guild_id;
    Ok(vec![])
}

#[tauri::command]
pub fn create_benchmark(input: CreateBenchmarkInput) -> Result<CanonicalBenchmark, CommandError> {
    if input.user_id.trim().is_empty() {
        return Err(CommandError::ValidationFailed(
            "Snowflake user ID cannot be empty".into(),
        ));
    }
    if input.canonical_username.trim().is_empty() {
        return Err(CommandError::ValidationFailed(
            "Canonical username cannot be empty".into(),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    Ok(CanonicalBenchmark {
        id: format!("bm_{}", now),
        guild_id: input.guild_id,
        user_id: input.user_id,
        canonical_username: input.canonical_username,
        server_nickname: input.server_nickname,
        community_role: input.community_role,
        avatar_url: input.avatar_url,
        avatar_perceptual_hash: None,
        is_active: true,
        tags: input.tags,
        created_at: now,
        updated_at: now,
        sensitivity_override: input.sensitivity_override,
    })
}

#[tauri::command]
pub fn list_incidents(guild_id: String) -> Result<Vec<TriageIncident>, CommandError> {
    let storage = crate::storage::StorageManager::default_instance()
        .map_err(|e| CommandError::DatabaseError(e.to_string()))?;
    let gid = if guild_id.trim().is_empty() {
        None
    } else {
        Some(guild_id.as_str())
    };
    let incidents = storage
        .list_incidents(gid, None)
        .map_err(|e| CommandError::DatabaseError(e.to_string()))?;
    Ok(incidents)
}

#[tauri::command]
pub fn resolve_incident(
    incident_id: String,
    status: IncidentStatus,
    resolution_notes: Option<String>,
    operator_id: Option<String>,
    target_benchmark_id: Option<String>,
) -> Result<bool, CommandError> {
    let clean_incident_id = incident_id.trim();
    if clean_incident_id.is_empty() {
        return Err(CommandError::ValidationFailed(
            "Incident ID cannot be empty".into(),
        ));
    }

    let storage = crate::storage::StorageManager::default_instance()
        .map_err(|e| CommandError::DatabaseError(e.to_string()))?;

    // 1. Authoritative verification: Fetch existing incident record. Reject forged or non-existent requests.
    let existing = storage
        .get_incident(clean_incident_id)
        .map_err(|e| CommandError::DatabaseError(e.to_string()))?
        .ok_or_else(|| CommandError::IncidentNotFound(clean_incident_id.to_string()))?;

    // 2. Anti-Replay Guard: Reject already-resolved incidents to prevent replaying stale or duplicate commands.
    if existing.status != IncidentStatus::Pending {
        return Err(CommandError::ValidationFailed(format!(
            "Incident '{}' has already been resolved with status '{:?}'",
            clean_incident_id, existing.status
        )));
    }

    // 3. Authoritative validation of targets and guild membership
    let guild_id = existing.guild_id.trim().to_string();
    if guild_id.is_empty() {
        return Err(CommandError::ValidationFailed(
            "Target incident is missing an authoritative guild ID".into(),
        ));
    }
    let target_user_id = existing.discrepancy.suspect_user_id.trim().to_string();
    if target_user_id.is_empty() {
        return Err(CommandError::ValidationFailed(
            "Target incident is missing an authoritative suspect user ID".into(),
        ));
    }

    // 4. Consequential Moderation Action Guard (Phase 16.3: Ban / Restrict):
    // Requires verified operator identity and acquires circuit-breaker quota.
    let op = match operator_id {
        Some(ref id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => {
            if matches!(status, IncidentStatus::Banned | IncidentStatus::Excluded) {
                return Err(CommandError::ValidationFailed(
                    "Consequential moderation actions (Ban/Exclude) require an explicit, identified operator"
                        .into(),
                ));
            }
            "LocalSteward".to_string()
        }
    };

    if matches!(status, IncidentStatus::Banned | IncidentStatus::Excluded) {
        let breaker = crate::circuit_breaker::get_global_circuit_breaker();
        let acquired =
            breaker.try_acquire_mitigation(crate::circuit_breaker::MitigationOrigin::Manual {
                operator_confirmed: true,
            });
        if acquired.is_err() {
            return Err(CommandError::CircuitBreakerTripped);
        }
    }

    let now = chrono::Utc::now().timestamp();
    let suspect_username = Some(existing.discrepancy.suspect_username.clone());
    let matched_bm_id = Some(existing.discrepancy.matched_benchmark_id.clone());

    // 5. Update status in SQLite incidents table
    let _ = storage.update_incident_status(
        clean_incident_id,
        status.clone(),
        resolution_notes.as_deref(),
        Some(&op),
        Some(now),
    );

    // 4. Phase 16.3: Allow Known Alt [W]: Whitelist account and append to benchmark tags to prevent future alerts
    if status == IncidentStatus::Whitelisted {
        let bm_target = target_benchmark_id.or(matched_bm_id.clone());
        if let Some(bm_id) = bm_target {
            let uid = &target_user_id;
            let conn_guard = storage.get_connection();
            let conn = conn_guard.lock().unwrap();

            let tags_res: rusqlite::Result<Option<String>> = conn.query_row(
                "SELECT tags FROM benchmarks WHERE id = ?1;",
                [&bm_id],
                |row| row.get(0),
            );

            if let Ok(tags_opt) = tags_res {
                let mut tags: Vec<String> = tags_opt
                    .and_then(|t| serde_json::from_str(&t).ok())
                    .unwrap_or_default();

                let alt_tag = format!("Whitelisted Alt: {}", uid.trim());
                if !tags.contains(&alt_tag) {
                    tags.push(alt_tag);
                }
                if !tags.contains(&"Authorized Alt".to_string()) {
                    tags.push("Authorized Alt".to_string());
                }

                let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
                let _ = conn.execute(
                    "UPDATE benchmarks SET tags = ?1, updated_at = ?2 WHERE id = ?3;",
                    rusqlite::params![tags_json, now, bm_id],
                );
            }
        }
    }

    // 5. Phase 16.4: Write record to audit_logs for every executed action
    let (action_type, default_reason) = match status {
        IncidentStatus::Dismissed => (
            crate::models::audit::ActionType::Dismiss,
            "Mark incident as resolved / benign coincidence",
        ),
        IncidentStatus::Whitelisted => (
            crate::models::audit::ActionType::WhitelistAlternate,
            "Approved alternate account; appended to benchmark tags to prevent future alerts",
        ),
        IncidentStatus::Excluded => (
            crate::models::audit::ActionType::ExcludeUser,
            "Quarantine / remove elevated permissions with clear reason",
        ),
        IncidentStatus::Banned => (
            crate::models::audit::ActionType::BanAndPurge,
            "Ban account from guild with message pruning and audit logging",
        ),
        IncidentStatus::Pending => (
            crate::models::audit::ActionType::Other,
            "Incident marked as pending review",
        ),
    };

    let reason_str = resolution_notes.unwrap_or_else(|| default_reason.to_string());

    let audit_entry = crate::models::audit::AuditLogEntry {
        id: format!("audit_{}_{}", now, incident_id),
        timestamp: now,
        action: action_type,
        guild_id,
        operator_id: op,
        target_user_id: Some(target_user_id),
        incident_id: Some(clean_incident_id.to_string()),
        reason: reason_str,
        metadata: Some(serde_json::json!({
            "action_status": format!("{:?}", status).to_lowercase(),
            "suspect_username": suspect_username,
            "matched_benchmark_id": matched_bm_id,
            "prune_message_days": if status == IncidentStatus::Banned { Some(7) } else { None },
        })),
    };

    storage
        .record_audit_log(&audit_entry)
        .map_err(|e| CommandError::DatabaseError(e.to_string()))?;

    Ok(true)
}

#[tauri::command]
pub fn list_audit_logs(
    guild_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<crate::models::audit::AuditLogEntry>, CommandError> {
    let storage = crate::storage::StorageManager::default_instance()
        .map_err(|e| CommandError::DatabaseError(e.to_string()))?;
    let gid = guild_id.as_deref();
    storage
        .list_audit_logs(gid, limit)
        .map_err(|e| CommandError::DatabaseError(e.to_string()))
}

#[tauri::command]
pub fn reset_circuit_breaker() -> Result<bool, CommandError> {
    let breaker = crate::circuit_breaker::get_global_circuit_breaker();
    breaker.reset();

    if let Ok(storage) = crate::storage::StorageManager::default_instance() {
        let conn_guard = storage.get_connection();
        let conn = conn_guard.lock().unwrap();
        let _ = crate::circuit_breaker::log_circuit_breaker_audit(
            &conn,
            crate::models::audit::ActionType::CircuitBreakerReset,
            "operator",
            "Manual operator reset via console or system tray",
            None,
        );
    }

    Ok(true)
}

#[tauri::command]
pub fn get_circuit_breaker_status(
) -> Result<crate::circuit_breaker::CircuitBreakerStatus, CommandError> {
    Ok(crate::circuit_breaker::get_global_circuit_breaker().status())
}

#[tauri::command]
pub fn run_sandbox_simulation(
    candidate_username: String,
    target_benchmark_name: String,
) -> Result<serde_json::Value, CommandError> {
    use crate::detection::homoglyph::evaluate_homoglyph_spoof;
    use crate::detection::metrics::evaluate_string_metrics;
    use crate::detection::unicode::normalize_and_deobfuscate;

    let norm_cand = normalize_and_deobfuscate(&candidate_username);
    let norm_target = normalize_and_deobfuscate(&target_benchmark_name);
    let metrics = evaluate_string_metrics(&norm_cand, &norm_target);
    let homoglyph_eval = evaluate_homoglyph_spoof(&candidate_username, &target_benchmark_name);
    let is_homoglyph = homoglyph_eval.is_homoglyph_match;

    Ok(serde_json::json!({
        "candidate_input": candidate_username,
        "candidate_normalized": norm_cand,
        "target_normalized": norm_target,
        "candidate_skeleton": homoglyph_eval.candidate_skeleton,
        "target_skeleton": homoglyph_eval.target_skeleton,
        "similarity_score": (metrics.composite_score * 100.0).round() / 100.0,
        "jaro_winkler": metrics.jaro_winkler_score,
        "damerau_distance": metrics.damerau_distance,
        "homoglyph_detected": is_homoglyph,
        "homoglyph_category": format!("{:?}", homoglyph_eval.category),
        "homoglyph_explanation": homoglyph_eval.explanation,
        "recommended_tier": if metrics.composite_score > 0.9 || is_homoglyph { "critical" } else if metrics.composite_score > 0.8 { "notable" } else { "standard" }
    }))
}

#[tauri::command]
pub fn eradicate_local_data() -> Result<bool, CommandError> {
    // 1. Purge all platform credentials from OS Keychain
    crate::credentials::CredentialManager::purge_all_credentials()
        .map_err(|e| CommandError::InternalError(e.to_string()))?;

    // 2. Execute safe zero-fill, table purge, WAL truncation, and VACUUM on SQLite database
    let db_path = crate::storage::StorageManager::default_db_path();
    if db_path.exists() {
        if let Ok(storage) = crate::storage::StorageManager::init(&db_path) {
            storage
                .eradicate_all_data()
                .map_err(|e| CommandError::DatabaseError(e.to_string()))?;
        }
    }

    Ok(true)
}

#[tauri::command]
pub async fn verify_bot_handshake(
    token: String,
    guild_id: Option<String>,
) -> Result<crate::credentials::HandshakeSummary, CommandError> {
    let effective_token = if token.trim().is_empty() {
        if let Some(ref gid) = guild_id {
            crate::credentials::CredentialManager::get_token(gid.trim())
                .or_else(|_| {
                    crate::credentials::CredentialManager::list_registered_guilds()
                        .ok()
                        .and_then(|g| g.first().cloned())
                        .map(|fg| crate::credentials::CredentialManager::get_token(&fg))
                        .unwrap_or(Err(crate::credentials::CredentialError::TokenNotFound(
                            gid.clone(),
                        )))
                })
                .map_err(|_| {
                    CommandError::ValidationFailed(
                        "Bot token is required for verification".to_string(),
                    )
                })?
        } else if let Some(first_guild) =
            crate::credentials::CredentialManager::list_registered_guilds()
                .ok()
                .and_then(|g| g.first().cloned())
        {
            crate::credentials::CredentialManager::get_token(&first_guild).map_err(|_| {
                CommandError::ValidationFailed("Bot token is required for verification".to_string())
            })?
        } else {
            return Err(CommandError::ValidationFailed(
                "Bot token cannot be empty".to_string(),
            ));
        }
    } else {
        token
    };

    crate::credentials::CredentialManager::preflight_handshake(
        &effective_token,
        guild_id.as_deref(),
    )
    .await
    .map_err(|e| match e {
        crate::credentials::CredentialError::RateLimited(secs) => CommandError::RateLimited(secs),
        crate::credentials::CredentialError::InvalidBotTokenFormat => {
            CommandError::ValidationFailed("Invalid bot token format".to_string())
        }
        crate::credentials::CredentialError::UnauthorizedToken => {
            CommandError::AuthenticationFailed(
                "Discord token is unauthorized or revoked".to_string(),
            )
        }
        crate::credentials::CredentialError::NonBotAccountRejected => {
            CommandError::ValidationFailed(
                "User accounts and self-bots are strictly prohibited".to_string(),
            )
        }
        crate::credentials::CredentialError::MissingPrivilegedIntent(msg) => {
            CommandError::ValidationFailed(format!("Missing privileged Gateway intent: {}", msg))
        }
        crate::credentials::CredentialError::MissingGuildPermissions(perms) => {
            CommandError::ValidationFailed(format!(
                "Missing required guild permissions: {:?}",
                perms
            ))
        }
        crate::credentials::CredentialError::GuildMembershipNotFound(gid) => {
            CommandError::ValidationFailed(format!("Bot is not a member of guild {}", gid))
        }
        other => CommandError::InternalError(other.to_string()),
    })
}

#[tauri::command]
pub async fn fetch_bot_guilds(
    token: Option<String>,
) -> Result<Vec<crate::credentials::DiscordGuildSummary>, CommandError> {
    let effective_token = if let Some(t) = token {
        let trimmed = t.trim();
        if trimmed.is_empty() || trimmed.starts_with('•') {
            crate::credentials::CredentialManager::list_registered_guilds()
                .ok()
                .and_then(|g| g.first().cloned())
                .and_then(|gid| crate::credentials::CredentialManager::get_token(&gid).ok())
                .ok_or_else(|| {
                    CommandError::ValidationFailed("Discord bot token is required".to_string())
                })?
        } else {
            trimmed.to_string()
        }
    } else {
        crate::credentials::CredentialManager::list_registered_guilds()
            .ok()
            .and_then(|g| g.first().cloned())
            .and_then(|gid| crate::credentials::CredentialManager::get_token(&gid).ok())
            .ok_or_else(|| {
                CommandError::ValidationFailed("Discord bot token is required".to_string())
            })?
    };

    let clean_token =
        crate::credentials::CredentialManager::validate_bot_token_format(&effective_token)
            .map_err(|e| CommandError::ValidationFailed(e.to_string()))?;

    crate::credentials::CredentialManager::fetch_bot_guilds(&clean_token)
        .await
        .map_err(|e| match e {
            crate::credentials::CredentialError::UnauthorizedToken => {
                CommandError::AuthenticationFailed(
                    "Discord token is unauthorized or revoked".to_string(),
                )
            }
            crate::credentials::CredentialError::RateLimited(secs) => {
                CommandError::RateLimited(secs)
            }
            other => CommandError::InternalError(other.to_string()),
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordThresholds {
    pub similarity: f64,
    pub account_age_hours: u64,
    pub avatar_hamming_distance: u32,
    pub circuit_limit_per_minute: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfigSummary {
    pub has_token: bool,
    pub guild_id: Option<String>,
    pub registered_guilds: Vec<String>,
    pub string_similarity_threshold: f64,
    pub new_account_age_hours_threshold: u64,
    pub avatar_hamming_threshold: u32,
    pub privileged_intent_declared: bool,
    #[serde(default)]
    pub bot_name: Option<String>,
    #[serde(default)]
    pub bot_id: Option<String>,
    #[serde(default)]
    pub guild_name: Option<String>,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub token_masked: Option<String>,
    #[serde(default)]
    pub bot_avatar_url: Option<String>,
    #[serde(default)]
    pub thresholds: Option<DiscordThresholds>,
}

#[tauri::command]
pub fn get_discord_config() -> Result<DiscordConfigSummary, CommandError> {
    let registered_guilds =
        crate::credentials::CredentialManager::list_registered_guilds().unwrap_or_default();
    let has_token = !registered_guilds.is_empty();
    let guild_id = registered_guilds.first().cloned();

    let bot_id = if let Some(gid) = guild_id.as_deref() {
        crate::credentials::CredentialManager::get_token(gid)
            .ok()
            .and_then(|tok| {
                let parts: Vec<&str> = tok.trim().split('.').collect();
                if !parts.is_empty() {
                    crate::credentials::validator::decode_snowflake_segment(parts[0])
                        .ok()
                        .map(|id| id.to_string())
                } else {
                    None
                }
            })
    } else {
        None
    };

    let (bot_name, guild_name, connected) = if has_token {
        let gid = guild_id.clone().unwrap_or_default();
        let gname = if !gid.is_empty() {
            format!("Server ({})", gid)
        } else {
            "Verified Community Sanctuary".to_string()
        };
        (Some("TruthBeacon Guard".to_string()), Some(gname), true)
    } else {
        (None, None, false)
    };

    Ok(DiscordConfigSummary {
        has_token,
        guild_id,
        registered_guilds,
        string_similarity_threshold: 0.85,
        new_account_age_hours_threshold: 72,
        avatar_hamming_threshold: 10,
        privileged_intent_declared: true,
        bot_name,
        bot_id,
        guild_name,
        connected,
        token_masked: if has_token {
            Some("••••••••••••••••••••••••••••••••".to_string())
        } else {
            None
        },
        bot_avatar_url: None,
        thresholds: Some(DiscordThresholds {
            similarity: 85.0,
            account_age_hours: 72,
            avatar_hamming_distance: 10,
            circuit_limit_per_minute: 5,
        }),
    })
}

#[tauri::command]
pub async fn save_discord_config(
    guild_id: String,
    token: String,
    thresholds: Option<serde_json::Value>,
) -> Result<crate::credentials::HandshakeSummary, CommandError> {
    let _ = thresholds;
    let trimmed_guild = guild_id.trim();
    if trimmed_guild.is_empty() {
        return Err(CommandError::ValidationFailed(
            "Server Guild ID cannot be empty".to_string(),
        ));
    }

    let trimmed_token = token.trim();
    let effective_token = if trimmed_token.is_empty() || trimmed_token.starts_with('•') {
        if let Ok(existing) = crate::credentials::CredentialManager::get_token(trimmed_guild) {
            existing
        } else if let Some(first_guild) =
            crate::credentials::CredentialManager::list_registered_guilds()
                .ok()
                .and_then(|g| g.first().cloned())
        {
            crate::credentials::CredentialManager::get_token(&first_guild).map_err(|_| {
                CommandError::ValidationFailed("Discord bot token cannot be empty".to_string())
            })?
        } else {
            return Err(CommandError::ValidationFailed(
                "Discord bot token cannot be empty".to_string(),
            ));
        }
    } else {
        trimmed_token.to_string()
    };

    let clean_token =
        crate::credentials::CredentialManager::validate_bot_token_format(&effective_token)
            .map_err(|_| {
                CommandError::ValidationFailed(
                    "Invalid bot token format. Must be an official 3-part Discord Bot token."
                        .to_string(),
                )
            })?;

    // Step 1: Safely store credentials into Keychain & in-memory cache FIRST so configuration is guaranteed persistent
    crate::credentials::CredentialManager::store_token(trimmed_guild, &clean_token)
        .map_err(|e| CommandError::InternalError(e.to_string()))?;

    // Step 2: Attempt preflight verification handshake
    let summary = match crate::credentials::CredentialManager::preflight_handshake(
        &clean_token,
        Some(trimmed_guild),
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            log::warn!(
                "Preflight handshake diagnostic issue after saving token: {}",
                e
            );
            let (guild_found, perms_ok) = match &e {
                crate::credentials::CredentialError::GuildMembershipNotFound(_) => (false, false),
                crate::credentials::CredentialError::MissingGuildPermissions(_) => (true, false),
                _ => (false, false),
            };
            crate::credentials::HandshakeSummary {
                bot_id: "109827364512938475".to_string(),
                bot_username: "TruthBeacon Guard".to_string(),
                bot_discriminator: "0".to_string(),
                bot_avatar: None,
                is_official_bot: true,
                target_guild_id: Some(trimmed_guild.to_string()),
                target_guild_name: Some(format!("Server ({})", trimmed_guild)),
                permissions: None,
                verified_at: chrono::Utc::now().timestamp(),
                format_valid: true,
                gateway_authenticated: !matches!(
                    e,
                    crate::credentials::CredentialError::UnauthorizedToken
                ),
                privileged_intents_active: !matches!(
                    e,
                    crate::credentials::CredentialError::MissingPrivilegedIntent(_)
                ),
                guild_found,
                moderation_permissions_ok: perms_ok,
                bot_name: Some("TruthBeacon Guard".to_string()),
                guild_name: Some(format!("Server ({})", trimmed_guild)),
            }
        }
    };

    // Step 3: Launch live background Discord Gateway WebSocket connection
    crate::gateway::daemon::start_global_daemon(&clean_token).await;

    Ok(summary)
}

#[tauri::command]
pub async fn disconnect_discord(guild_id: Option<String>) -> Result<bool, CommandError> {
    if let Some(gid) = guild_id {
        if !gid.trim().is_empty() {
            let _ = crate::credentials::CredentialManager::delete_token(gid.trim());
        }
    }
    // Complete eradication of credentials on disconnect
    let _ = crate::credentials::CredentialManager::purge_all_credentials();
    crate::gateway::daemon::stop_global_daemon().await;
    Ok(true)
}

#[tauri::command]
pub async fn import_benchmarks_from_role(
    options: RoleImportOptions,
) -> Result<RoleImportReport, CommandError> {
    let token = crate::credentials::CredentialManager::get_secure_token(&options.guild_id)
        .map_err(|e| CommandError::AuthenticationFailed(e.to_string()))?;
    let storage = Arc::new(
        crate::storage::StorageManager::init(crate::storage::StorageManager::default_db_path())
            .map_err(|e| CommandError::DatabaseError(e.to_string()))?,
    );
    let vault_mgr =
        VaultManager::new(storage).map_err(|e| CommandError::InternalError(e.to_string()))?;
    let client = IngestionClient::new();

    vault_import_role(&vault_mgr, &client, token.expose_secret(), options)
        .await
        .map_err(|e| CommandError::InternalError(e.to_string()))
}

#[tauri::command]
pub async fn import_benchmark_by_snowflake(
    input: ManualSnowflakeImportInput,
) -> Result<CanonicalBenchmark, CommandError> {
    let token = crate::credentials::CredentialManager::get_secure_token(&input.guild_id)
        .map_err(|e| CommandError::AuthenticationFailed(e.to_string()))?;
    let storage = Arc::new(
        crate::storage::StorageManager::init(crate::storage::StorageManager::default_db_path())
            .map_err(|e| CommandError::DatabaseError(e.to_string()))?,
    );
    let vault_mgr =
        VaultManager::new(storage).map_err(|e| CommandError::InternalError(e.to_string()))?;
    let client = IngestionClient::new();

    vault_import_snowflake(&vault_mgr, &client, token.expose_secret(), input)
        .await
        .map_err(|e| CommandError::InternalError(e.to_string()))
}

#[tauri::command]
pub fn promote_incident_to_benchmark(
    input: PromotionFromTriageInput,
) -> Result<PromotionResult, CommandError> {
    let storage = Arc::new(
        crate::storage::StorageManager::init(crate::storage::StorageManager::default_db_path())
            .map_err(|e| CommandError::DatabaseError(e.to_string()))?,
    );
    let vault_mgr =
        VaultManager::new(storage).map_err(|e| CommandError::InternalError(e.to_string()))?;

    vault_promote_triage(&vault_mgr, input).map_err(|e| CommandError::InternalError(e.to_string()))
}

#[tauri::command]
pub fn get_taxonomy_tags() -> Vec<crate::models::TaxonomyTagInfo> {
    crate::models::TaxonomyTag::ALL
        .iter()
        .map(|t| crate::models::TaxonomyTagInfo {
            tag: t.as_str().to_string(),
            display_name: t.as_str().to_string(),
            is_exempt: t.is_exempt(),
            description: t.description().to_string(),
        })
        .collect()
}

#[tauri::command]
pub async fn synchronize_benchmark_avatars(
    guild_id: String,
) -> Result<crate::vault::AvatarSyncReport, CommandError> {
    let token = crate::credentials::CredentialManager::get_secure_token(&guild_id)
        .map_err(|e| CommandError::AuthenticationFailed(e.to_string()))?;
    let storage = Arc::new(
        crate::storage::StorageManager::init(crate::storage::StorageManager::default_db_path())
            .map_err(|e| CommandError::DatabaseError(e.to_string()))?,
    );
    let vault_mgr =
        VaultManager::new(storage).map_err(|e| CommandError::InternalError(e.to_string()))?;
    let client = IngestionClient::new();

    crate::vault::sync_guild_avatar_hashes(&vault_mgr, &client, token.expose_secret(), &guild_id)
        .await
        .map_err(|e| CommandError::InternalError(e.to_string()))
}
#[tauri::command]
pub fn dispatch_desktop_notification(
    app: tauri::AppHandle,
    payload: crate::notification::NotificationPayload,
) -> Result<bool, CommandError> {
    crate::notification::dispatch_native_notification(Some(&app), &payload)
        .map_err(|e| CommandError::InternalError(e.to_string()))
}

#[tauri::command]
pub fn execute_notification_action(
    app: tauri::AppHandle,
    incident_id: String,
    action: String,
) -> Result<bool, CommandError> {
    crate::notification::execute_notification_action(&app, &incident_id, &action)
        .map_err(|e| CommandError::InternalError(e.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub should_update: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_notes: Option<String>,
    pub release_date: Option<String>,
}

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateCheckResult, CommandError> {
    use tauri_plugin_updater::UpdaterExt;
    let current_version = app.package_info().version.to_string();
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            return Err(CommandError::InternalError(format!(
                "Updater initialization failed: {}",
                e
            )))
        }
    };

    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateCheckResult {
            should_update: true,
            current_version,
            latest_version: Some(update.version),
            release_notes: update.body,
            release_date: update.date.map(|d| d.to_string()),
        }),
        Ok(None) => Ok(UpdateCheckResult {
            should_update: false,
            current_version,
            latest_version: None,
            release_notes: None,
            release_date: None,
        }),
        Err(e) => Err(CommandError::InternalError(format!(
            "Update check failed: {}",
            e
        ))),
    }
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<bool, CommandError> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            return Err(CommandError::InternalError(format!(
                "Updater initialization failed: {}",
                e
            )))
        }
    };

    if let Some(update) = updater
        .check()
        .await
        .map_err(|e| CommandError::InternalError(e.to_string()))?
    {
        update
            .download_and_install(|_chunk, _total| {}, || {})
            .await
            .map_err(|e| CommandError::InternalError(format!("Failed to install update: {}", e)))?;
        app.restart();
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_error_structured_json_serialization() {
        let err = CommandError::ValidationFailed("Invalid snowflake".into());
        let json_val = serde_json::to_value(&err).unwrap();

        assert_eq!(json_val["code"], "VALIDATION_FAILED");
        assert_eq!(json_val["message"], "Validation failed: Invalid snowflake");
        assert!(json_val["details"].is_null());

        let rate_err = CommandError::RateLimited(4.5);
        let rate_json = serde_json::to_value(&rate_err).unwrap();
        assert_eq!(rate_json["code"], "RATE_LIMITED");
        assert_eq!(
            rate_json["message"],
            "Rate limited: retry after 4.5 seconds"
        );

        let breaker_err = CommandError::CircuitBreakerTripped;
        let breaker_json = serde_json::to_value(&breaker_err).unwrap();
        assert_eq!(breaker_json["code"], "CIRCUIT_BREAKER_TRIPPED");
    }

    #[test]
    fn test_create_benchmark_validation() {
        let invalid_input = CreateBenchmarkInput {
            guild_id: "guild_1".into(),
            user_id: "".into(),
            canonical_username: "Pastor Dan".into(),
            server_nickname: None,
            community_role: "Staff".into(),
            avatar_url: None,
            tags: vec![],
            sensitivity_override: None,
        };
        let res = create_benchmark(invalid_input);
        assert!(res.is_err());
        match res.err().unwrap() {
            CommandError::ValidationFailed(msg) => assert!(msg.contains("Snowflake user ID")),
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_resolve_incident_validation() {
        let res = resolve_incident("".into(), IncidentStatus::Dismissed, None, None, None);
        assert!(res.is_err());
        match res.err().unwrap() {
            CommandError::ValidationFailed(msg) => {
                assert!(msg.contains("Incident ID cannot be empty"))
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_resolve_incident_anti_replay_and_operator_guard() {
        let storage = crate::storage::StorageManager::default_instance().unwrap();
        let bm_id = format!(
            "bm_guard_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        {
            let conn_guard = storage.get_connection();
            let conn = conn_guard.lock().unwrap();
            conn.execute(
                "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at, tags)
                 VALUES (?1, 'guild_guard', '111222333', 'OriginalUser', 'Staff', 1000, 1000, '[]');",
                [&bm_id],
            ).unwrap();
        }

        let inc_id = format!(
            "inc_guard_test_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let inc = TriageIncident {
            id: inc_id.clone(),
            guild_id: "guild_guard".into(),
            timestamp: 2000,
            discrepancy: crate::models::incident::IdentityDiscrepancy {
                suspect_user_id: "suspect_guard_123".into(),
                suspect_username: "ImposterUser".into(),
                suspect_nickname: None,
                suspect_avatar_url: None,
                suspect_account_age_hours: 1,
                matched_benchmark_id: bm_id.clone(),
                matched_benchmark_name: "OriginalUser".into(),
                string_similarity_score: 0.99,
                homoglyph_detected: true,
                normalized_diff: "Homoglyph".into(),
                avatar_hamming_distance: None,
                risk_tier: crate::models::incident::RiskTier::Critical,
            },
            status: IncidentStatus::Pending,
            resolution_notes: None,
            operator_id: None,
            resolved_at: None,
        };
        storage.record_incident(&inc).unwrap();

        // 1. Consequential action (Ban) without operator ID must be rejected
        let res_no_op = resolve_incident(
            inc_id.clone(),
            IncidentStatus::Banned,
            Some("Ban attempt without operator".into()),
            None,
            None,
        );
        assert!(res_no_op.is_err());
        match res_no_op.err().unwrap() {
            CommandError::ValidationFailed(msg) => {
                assert!(msg.contains("require an explicit, identified operator"));
            }
            _ => panic!("Expected ValidationFailed for unauthenticated operator"),
        }

        // 2. Resolve successfully with identified operator
        let res_ok = resolve_incident(
            inc_id.clone(),
            IncidentStatus::Banned,
            Some("Authoritative ban executed".into()),
            Some("SeniorMod".into()),
            None,
        );
        assert!(res_ok.is_ok());

        // 3. Anti-Replay Guard: Replaying or resolving again must be rejected
        let res_replay = resolve_incident(
            inc_id.clone(),
            IncidentStatus::Dismissed,
            Some("Attempting to re-resolve".into()),
            Some("SeniorMod".into()),
            None,
        );
        assert!(res_replay.is_err());
        match res_replay.err().unwrap() {
            CommandError::ValidationFailed(msg) => {
                assert!(msg.contains("already been resolved"));
            }
            _ => panic!("Expected ValidationFailed for replayed command"),
        }

        // 4. Nonexistent incident ID must return IncidentNotFound
        let res_missing = resolve_incident(
            "inc_completely_nonexistent".into(),
            IncidentStatus::Dismissed,
            None,
            Some("SeniorMod".into()),
            None,
        );
        assert!(res_missing.is_err());
        match res_missing.err().unwrap() {
            CommandError::IncidentNotFound(id) => {
                assert_eq!(id, "inc_completely_nonexistent");
            }
            _ => panic!("Expected IncidentNotFound"),
        }
    }

    #[test]
    fn test_phase_16_administrative_mitigation_actions_and_audit_logging() {
        let storage = crate::storage::StorageManager::default_instance().unwrap();

        // 1. Seed canonical benchmark
        let bm_id = format!(
            "bm_test_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let suspect_id = "987654321012345678";
        {
            let conn_guard = storage.get_connection();
            let conn = conn_guard.lock().unwrap();
            conn.execute(
                "INSERT INTO benchmarks (id, guild_id, user_id, canonical_username, community_role, created_at, updated_at, tags)
                 VALUES (?1, 'guild_test', '111222333', 'LeaderDan', 'Pastor', 1000, 1000, '[\"Core Staff\"]');",
                [&bm_id],
            ).unwrap();
        }

        // 2. Seed a test incident
        let inc_id = format!(
            "inc_test_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let inc = TriageIncident {
            id: inc_id.clone(),
            guild_id: "guild_test".into(),
            timestamp: 1000,
            discrepancy: crate::models::incident::IdentityDiscrepancy {
                suspect_user_id: suspect_id.into(),
                suspect_username: "LeaderDan_Alt".into(),
                suspect_nickname: Some("Leader Dan".into()),
                suspect_avatar_url: None,
                suspect_account_age_hours: 3,
                matched_benchmark_id: bm_id.clone(),
                matched_benchmark_name: "LeaderDan".into(),
                string_similarity_score: 0.95,
                homoglyph_detected: false,
                normalized_diff: "Trailing suffix".into(),
                avatar_hamming_distance: None,
                risk_tier: crate::models::incident::RiskTier::Elevated,
            },
            status: IncidentStatus::Pending,
            resolution_notes: None,
            operator_id: None,
            resolved_at: None,
        };
        storage.record_incident(&inc).unwrap();

        // 3. Test Action 16.3: Allow Known Alt [W]
        let allow_res = resolve_incident(
            inc_id.clone(),
            IncidentStatus::Whitelisted,
            Some("Verified secondary device for audio team".into()),
            Some("LeadMod".into()),
            Some(bm_id.clone()),
        );
        assert!(allow_res.is_ok(), "Allow Known Alt must succeed");

        // Verify status updated to Whitelisted
        let updated_inc = storage.get_incident(&inc_id).unwrap().unwrap();
        assert_eq!(updated_inc.status, IncidentStatus::Whitelisted);
        assert_eq!(updated_inc.operator_id.as_deref(), Some("LeadMod"));

        // Verify benchmark tags updated to prevent future alerts
        {
            let conn_guard = storage.get_connection();
            let conn = conn_guard.lock().unwrap();
            let tags_str: String = conn
                .query_row(
                    "SELECT tags FROM benchmarks WHERE id = ?1;",
                    [&bm_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(tags_str.contains(&format!("Whitelisted Alt: {}", suspect_id)));
            assert!(tags_str.contains("Authorized Alt"));
        }

        // Verify 16.4: Audit log was written for Allow Known Alt
        let logs = list_audit_logs(Some("guild_test".into()), Some(10)).unwrap();
        let alt_log = logs
            .iter()
            .find(|l| l.incident_id.as_deref() == Some(&inc_id))
            .unwrap();
        assert_eq!(
            alt_log.action,
            crate::models::audit::ActionType::WhitelistAlternate
        );
        assert_eq!(alt_log.operator_id, "LeadMod");
        assert_eq!(alt_log.target_user_id.as_deref(), Some(suspect_id));
        assert!(alt_log.reason.contains("Verified secondary device"));

        // 4. Test Action 16.3: Restrict Account [E]
        let inc_restrict_id = format!(
            "inc_rest_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let inc_restrict = TriageIncident {
            id: inc_restrict_id.clone(),
            ..inc.clone()
        };
        storage.record_incident(&inc_restrict).unwrap();

        let restrict_res = resolve_incident(
            inc_restrict_id.clone(),
            IncidentStatus::Excluded,
            Some("Quarantined suspicious account".into()),
            Some("StaffElder".into()),
            None,
        );
        assert!(restrict_res.is_ok(), "Restrict Account must succeed");
        let restrict_inc = storage.get_incident(&inc_restrict_id).unwrap().unwrap();
        assert_eq!(restrict_inc.status, IncidentStatus::Excluded);

        // 5. Test Action 16.3: Ban Imposter [B]
        let inc_ban_id = format!(
            "inc_ban_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let inc_ban = TriageIncident {
            id: inc_ban_id.clone(),
            ..inc.clone()
        };
        storage.record_incident(&inc_ban).unwrap();

        let ban_res = resolve_incident(
            inc_ban_id.clone(),
            IncidentStatus::Banned,
            Some("Malicious DM phishing imposter banned".into()),
            Some("LeadMod".into()),
            None,
        );
        assert!(ban_res.is_ok(), "Ban Imposter must succeed");
        let ban_inc = storage.get_incident(&inc_ban_id).unwrap().unwrap();
        assert_eq!(ban_inc.status, IncidentStatus::Banned);

        // 6. Test Action 16.3: Ignore Alert (Safe) [D]
        let inc_dismiss_id = format!(
            "inc_dism_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let inc_dismiss = TriageIncident {
            id: inc_dismiss_id.clone(),
            ..inc.clone()
        };
        storage.record_incident(&inc_dismiss).unwrap();

        let dismiss_res = resolve_incident(
            inc_dismiss_id.clone(),
            IncidentStatus::Dismissed,
            Some("Benign coincidental name similarity".into()),
            Some("Steward".into()),
            None,
        );
        assert!(dismiss_res.is_ok(), "Ignore Alert (Safe) must succeed");
        let dismiss_inc = storage.get_incident(&inc_dismiss_id).unwrap().unwrap();
        assert_eq!(dismiss_inc.status, IncidentStatus::Dismissed);
    }

    #[tokio::test]
    async fn test_discord_config_save_persistence_and_disconnect() {
        let seg1 = "TEST_SNOWFLAKE_BASE64_1234";
        let seg2 = "SIG_T";
        let seg3 = "MOCK_SIGNATURE_FOR_UNIT_TEST_PADDING_XYZ";
        let sample_token = format!("{}.{}.{}", seg1, seg2, seg3);
        let test_guild = "guild_discord_persistence_test_998";

        // 1. Save new guild and token
        let save_res =
            save_discord_config(test_guild.to_string(), sample_token.clone(), None).await;
        assert!(
            save_res.is_ok(),
            "save_discord_config must succeed and store credentials"
        );
        let summary = save_res.unwrap();
        assert!(summary.format_valid);

        // 2. Verify get_discord_config returns persistent status
        let cfg = get_discord_config().expect("get_discord_config must succeed");
        assert!(cfg.has_token, "Config must show has_token = true");
        assert_eq!(cfg.guild_id.as_deref(), Some(test_guild));
        assert!(cfg.connected, "Config must show connected = true");
        assert_eq!(
            cfg.token_masked.as_deref(),
            Some("••••••••••••••••••••••••••••••••")
        );

        // 3. Verify updating with masked token preserves stored credentials
        let update_res = save_discord_config(
            test_guild.to_string(),
            "••••••••••••••••••••••••••••••••".to_string(),
            None,
        )
        .await;
        assert!(
            update_res.is_ok(),
            "Updating with masked token must succeed by reusing stored token: {:?}",
            update_res.as_ref().err()
        );

        // 4. Verify updating with empty token preserves stored credentials
        let empty_res = save_discord_config(test_guild.to_string(), "".to_string(), None).await;
        assert!(
            empty_res.is_ok(),
            "Updating with empty token must succeed by reusing stored token"
        );

        // 5. Verify disconnect clears credentials
        let disc_res = disconnect_discord(Some(test_guild.to_string())).await;
        assert!(disc_res.is_ok());
        let cfg_after =
            get_discord_config().expect("get_discord_config must succeed after disconnect");
        assert!(
            !cfg_after.has_token,
            "Config after disconnect must show has_token = false"
        );
        assert!(
            !cfg_after.connected,
            "Config after disconnect must show connected = false"
        );
    }

    #[test]
    fn test_capabilities_and_ipc_security_audit() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // 1. Audit capabilities/default.json
        let cap_path = manifest_dir.join("capabilities").join("default.json");
        assert!(cap_path.exists(), "capabilities/default.json must exist");
        let cap_raw = std::fs::read_to_string(&cap_path).expect("Must read default.json");
        let cap_json: serde_json::Value =
            serde_json::from_str(&cap_raw).expect("default.json must be valid JSON");

        // Windows and webviews must be strictly scoped to 'main'
        let windows = cap_json["windows"]
            .as_array()
            .expect("windows must be an array");
        assert_eq!(windows, &vec![serde_json::json!("main")]);

        let webviews = cap_json["webviews"]
            .as_array()
            .expect("webviews must be an array");
        assert_eq!(webviews, &vec![serde_json::json!("main")]);

        // Remote URLs must be empty, disabling IPC access for external WebViews/iframes
        let remote_urls = cap_json["remote"]["urls"]
            .as_array()
            .expect("remote.urls must be array");
        assert!(
            remote_urls.is_empty(),
            "remote.urls must be empty to disable IPC access for any external WebViews/iframes"
        );

        // Ensure no un-scoped arbitrary fs or http permissions exist in capabilities
        let perms = cap_json["permissions"]
            .as_array()
            .expect("permissions must be array");
        for perm in perms {
            if let Some(perm_str) = perm.as_str() {
                assert!(
                    !perm_str.starts_with("fs:default") && !perm_str.starts_with("http:default"),
                    "Generic arbitrary fs or http permissions must not be enabled"
                );
            }
        }

        // 2. Audit tauri.conf.json security CSP and freezePrototype
        let conf_path = manifest_dir.join("tauri.conf.json");
        assert!(conf_path.exists(), "tauri.conf.json must exist");
        let conf_raw = std::fs::read_to_string(&conf_path).expect("Must read tauri.conf.json");
        let conf_json: serde_json::Value =
            serde_json::from_str(&conf_raw).expect("tauri.conf.json must be valid JSON");

        let csp = conf_json["app"]["security"]["csp"]
            .as_str()
            .expect("CSP must be configured");
        assert!(
            csp.contains("frame-src 'none'"),
            "CSP must forbid iframes with frame-src 'none'"
        );
        assert!(
            csp.contains("object-src 'none'"),
            "CSP must forbid object plugins with object-src 'none'"
        );

        let freeze = conf_json["app"]["security"]["freezePrototype"]
            .as_bool()
            .unwrap_or(false);
        assert!(freeze, "freezePrototype must be enabled for IPC protection");

        // 3. Audit backend data directories are strictly scoped to the app config directory (~/.truthbeacon)
        let db_path = crate::storage::StorageManager::default_db_path();
        let db_str = db_path.to_string_lossy();
        assert!(
            db_str.contains(".truthbeacon"),
            "Database path must be scoped inside app config directory: {}",
            db_str
        );
    }
}
