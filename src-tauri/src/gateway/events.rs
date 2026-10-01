//! Discord Gateway Privileged Events & Pipeline Ingestion Router (Phase 14.2)
//!
//! Handles privileged intent routing for:
//! - `GUILD_MEMBER_ADD`: Evaluates incoming newcomers immediately upon join.
//! - `GUILD_MEMBER_UPDATE`: Evaluates nickname, avatar, or role updates on existing members.
//! - `USER_UPDATE`: Evaluates global username or avatar updates across active guilds.
//!
//! Extracted candidate profiles are dispatched directly to the `DetectionEngine` pipeline.

use crate::detection::snowflake::calculate_account_age_hours;
use crate::detection::snowflake::parse_snowflake_str;
use crate::detection::DetectionEngine;
use crate::models::{CanonicalBenchmark, IdentityDiscrepancy};
use crate::vault::VaultManager;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Privileged Intent flag for Guild Members (`1 << 1`)
pub const GUILD_MEMBERS_INTENT: u64 = 1 << 1;

#[derive(Debug, thiserror::Error)]
pub enum EventRoutingError {
    #[error("Failed to deserialize dispatch event '{0}': {1}")]
    Deserialization(String, String),

    #[error("Vault error: {0}")]
    Vault(String),

    #[error("Unhandled or ignored dispatch event: {0}")]
    IgnoredEvent(String),
}

/// Discord user structure within dispatch payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUserPayload {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: Option<bool>,
}

/// Payload received on `GUILD_MEMBER_ADD`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildMemberAddPayload {
    pub guild_id: String,
    pub user: DiscordUserPayload,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub joined_at: Option<String>,
}

/// Payload received on `GUILD_MEMBER_UPDATE`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildMemberUpdatePayload {
    pub guild_id: String,
    pub user: DiscordUserPayload,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub avatar: Option<String>,
}

/// Payload received on `USER_UPDATE`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdatePayload {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
}

/// Constructs a Discord CDN avatar URL given a user ID and avatar hash
pub fn format_avatar_url(user_id: &str, avatar_hash: Option<&str>) -> Option<String> {
    avatar_hash.map(|hash| {
        if hash.starts_with("a_") {
            format!(
                "https://cdn.discordapp.com/avatars/{}/{}.gif",
                user_id, hash
            )
        } else {
            format!(
                "https://cdn.discordapp.com/avatars/{}/{}.png",
                user_id, hash
            )
        }
    })
}

/// Event router connecting Discord Gateway dispatches to Detection Engine & Vault
pub struct GatewayEventRouter {
    engine: Arc<DetectionEngine>,
    vault: Arc<VaultManager>,
}

impl GatewayEventRouter {
    pub fn new(engine: Arc<DetectionEngine>, vault: Arc<VaultManager>) -> Self {
        Self { engine, vault }
    }

    /// Primary entry point: Route and evaluate a raw Gateway dispatch event
    pub fn route_dispatch(
        &self,
        event_name: &str,
        data: &serde_json::Value,
    ) -> Result<Vec<IdentityDiscrepancy>, EventRoutingError> {
        match event_name {
            "GUILD_MEMBER_ADD" => {
                let payload: GuildMemberAddPayload =
                    serde_json::from_value(data.clone()).map_err(|e| {
                        EventRoutingError::Deserialization(event_name.to_string(), e.to_string())
                    })?;
                let discrepancy = self.evaluate_member_add(&payload)?;
                Ok(discrepancy.into_iter().collect())
            }

            "GUILD_MEMBER_UPDATE" => {
                let payload: GuildMemberUpdatePayload = serde_json::from_value(data.clone())
                    .map_err(|e| {
                        EventRoutingError::Deserialization(event_name.to_string(), e.to_string())
                    })?;
                let discrepancy = self.evaluate_member_update(&payload)?;
                Ok(discrepancy.into_iter().collect())
            }

            "USER_UPDATE" => {
                let payload: UserUpdatePayload =
                    serde_json::from_value(data.clone()).map_err(|e| {
                        EventRoutingError::Deserialization(event_name.to_string(), e.to_string())
                    })?;
                self.evaluate_user_update(&payload)
            }

            other => Err(EventRoutingError::IgnoredEvent(other.to_string())),
        }
    }

    /// Evaluates `GUILD_MEMBER_ADD` newcomer against the target guild's active benchmarks
    pub fn evaluate_member_add(
        &self,
        payload: &GuildMemberAddPayload,
    ) -> Result<Option<IdentityDiscrepancy>, EventRoutingError> {
        let benchmarks = self
            .vault
            .list_benchmarks(&payload.guild_id, true)
            .map_err(|e| EventRoutingError::Vault(e.to_string()))?;

        if benchmarks.is_empty() {
            return Ok(None);
        }

        let age_hours = calculate_snowflake_age_hours(&payload.user.id);
        let avatar_url = format_avatar_url(&payload.user.id, payload.user.avatar.as_deref());

        let discrepancy = self.engine.evaluate_candidate_with_avatar_hash(
            &payload.user.id,
            &payload.user.username,
            payload.nick.as_deref(),
            avatar_url.as_deref(),
            None,
            age_hours,
            &benchmarks,
        );

        Ok(discrepancy)
    }

    /// Evaluates `GUILD_MEMBER_UPDATE` nickname or avatar changes against the guild's benchmarks
    pub fn evaluate_member_update(
        &self,
        payload: &GuildMemberUpdatePayload,
    ) -> Result<Option<IdentityDiscrepancy>, EventRoutingError> {
        let benchmarks = self
            .vault
            .list_benchmarks(&payload.guild_id, true)
            .map_err(|e| EventRoutingError::Vault(e.to_string()))?;

        if benchmarks.is_empty() {
            return Ok(None);
        }

        let age_hours = calculate_snowflake_age_hours(&payload.user.id);
        let avatar_url = format_avatar_url(&payload.user.id, payload.user.avatar.as_deref());

        let discrepancy = self.engine.evaluate_candidate_with_avatar_hash(
            &payload.user.id,
            &payload.user.username,
            payload.nick.as_deref(),
            avatar_url.as_deref(),
            None,
            age_hours,
            &benchmarks,
        );

        Ok(discrepancy)
    }

    /// Evaluates `USER_UPDATE` global username or avatar changes across all active guild benchmarks
    pub fn evaluate_user_update(
        &self,
        payload: &UserUpdatePayload,
    ) -> Result<Vec<IdentityDiscrepancy>, EventRoutingError> {
        let all_benchmarks = self
            .vault
            .list_all_benchmarks(true)
            .map_err(|e| EventRoutingError::Vault(e.to_string()))?;

        if all_benchmarks.is_empty() {
            return Ok(Vec::new());
        }

        let age_hours = calculate_snowflake_age_hours(&payload.id);
        let avatar_url = format_avatar_url(&payload.id, payload.avatar.as_deref());

        // Group benchmarks by guild to evaluate accurately within guild context
        let mut discrepancies = Vec::new();
        let mut benchmarks_by_guild: std::collections::HashMap<String, Vec<CanonicalBenchmark>> =
            std::collections::HashMap::new();

        for bm in all_benchmarks {
            benchmarks_by_guild
                .entry(bm.guild_id.clone())
                .or_default()
                .push(bm);
        }

        for (_guild_id, guild_bms) in benchmarks_by_guild {
            if let Some(discrepancy) = self.engine.evaluate_candidate_with_avatar_hash(
                &payload.id,
                &payload.username,
                None,
                avatar_url.as_deref(),
                None,
                age_hours,
                &guild_bms,
            ) {
                discrepancies.push(discrepancy);
            }
        }

        Ok(discrepancies)
    }
}

/// Helper to compute account age in whole hours directly from a user Snowflake string
fn calculate_snowflake_age_hours(user_id: &str) -> u64 {
    if let Ok(snowflake) = parse_snowflake_str(user_id) {
        let now_ms = Utc::now().timestamp_millis() as u64;
        calculate_account_age_hours(snowflake, now_ms).floor() as u64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::snowflake::build_mock_snowflake;
    use crate::models::{CreateBenchmarkInput, RiskTier};
    use crate::storage::StorageManager;

    fn setup_test_router(guild_id: &str) -> (GatewayEventRouter, Arc<VaultManager>) {
        let storage = Arc::new(StorageManager::in_memory().unwrap());
        let vault = Arc::new(VaultManager::new(storage).unwrap());
        let engine = Arc::new(DetectionEngine::default());

        // Create a canonical benchmark for Pastor Dan
        vault
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_id.to_string(),
                    user_id: "100000000000000001".to_string(),
                    canonical_username: "Pastor Dan".to_string(),
                    server_nickname: Some("Dan | Senior Pastor".to_string()),
                    community_role: "Executive Pastor".to_string(),
                    avatar_url: Some("https://cdn.discordapp.com/avatars/101/a1.png".to_string()),
                    tags: vec!["Staff".to_string()],
                    sensitivity_override: None,
                },
                Some("d4c3b2a10000ffff".to_string()),
            )
            .unwrap();

        let router = GatewayEventRouter::new(engine, vault.clone());
        (router, vault)
    }

    #[test]
    fn test_intent_declaration_guild_members() {
        assert_eq!(GUILD_MEMBERS_INTENT, 1 << 1);
        assert_eq!(GUILD_MEMBERS_INTENT, 2);
    }

    #[test]
    fn test_ingest_guild_member_add_flags_impersonator() {
        let guild_id = "guild_church_1";
        let (router, _) = setup_test_router(guild_id);

        let now_ms = Utc::now().timestamp_millis() as u64;
        // Attacker creates account 2 hours ago
        let attacker_ts = now_ms - (2 * 3_600_000);
        let attacker_snowflake = build_mock_snowflake(attacker_ts).to_string();

        let member_add_json = serde_json::json!({
            "guild_id": guild_id,
            "user": {
                "id": attacker_snowflake,
                "username": "Pastor Dаn", // Cyrillic 'а'
                "global_name": "Pastor Dan",
                "avatar": "attacker_avatar_hash"
            },
            "nick": "Dan | Senior Pastor",
            "roles": [],
            "joined_at": "2026-10-01T12:00:00Z"
        });

        let discrepancies = router
            .route_dispatch("GUILD_MEMBER_ADD", &member_add_json)
            .unwrap();

        assert_eq!(discrepancies.len(), 1);
        let disp = &discrepancies[0];
        assert_eq!(disp.matched_benchmark_name, "Pastor Dan");
        assert_eq!(disp.risk_tier, RiskTier::Critical);
        assert!(disp.homoglyph_detected);
        assert!(disp.suspect_account_age_hours < 72);
    }

    #[test]
    fn test_ingest_guild_member_add_clean_member_passes() {
        let guild_id = "guild_church_1";
        let (router, _) = setup_test_router(guild_id);

        let member_add_clean = serde_json::json!({
            "guild_id": guild_id,
            "user": {
                "id": "200000000000000002",
                "username": "RegularMember",
                "global_name": "Regular Member",
                "avatar": null
            },
            "nick": null,
            "roles": [],
            "joined_at": "2026-10-01T12:00:00Z"
        });

        let discrepancies = router
            .route_dispatch("GUILD_MEMBER_ADD", &member_add_clean)
            .unwrap();

        assert!(discrepancies.is_empty());
    }

    #[test]
    fn test_ingest_guild_member_update_nickname_spoof() {
        let guild_id = "guild_church_1";
        let (router, _) = setup_test_router(guild_id);

        let update_json = serde_json::json!({
            "guild_id": guild_id,
            "user": {
                "id": "300000000000000003",
                "username": "innocent_username",
                "global_name": "Innocent User",
                "avatar": null
            },
            // Malicious member changes nickname to match Pastor Dan
            "nick": "Dan | Senior Pastor",
            "roles": [],
            "avatar": null
        });

        let discrepancies = router
            .route_dispatch("GUILD_MEMBER_UPDATE", &update_json)
            .unwrap();

        assert_eq!(discrepancies.len(), 1);
        let disp = &discrepancies[0];
        assert_eq!(disp.matched_benchmark_name, "Pastor Dan");
        assert_eq!(
            disp.suspect_nickname.as_deref(),
            Some("Dan | Senior Pastor")
        );
        assert_eq!(disp.string_similarity_score, 1.0);
        assert!(disp.normalized_diff.contains("NicknameVsNickname"));
    }

    #[test]
    fn test_ingest_user_update_global_name_spoof() {
        let guild_id = "guild_church_1";
        let (router, _) = setup_test_router(guild_id);

        let user_update_json = serde_json::json!({
            "id": "400000000000000004",
            // Attacker updates global username to a lookalike
            "username": "Pastor Dan",
            "global_name": "Pastor Dan",
            "avatar": null
        });

        let discrepancies = router
            .route_dispatch("USER_UPDATE", &user_update_json)
            .unwrap();

        assert_eq!(discrepancies.len(), 1);
        let disp = &discrepancies[0];
        assert_eq!(disp.matched_benchmark_name, "Pastor Dan");
        assert_eq!(disp.suspect_username, "Pastor Dan");
    }

    #[test]
    fn test_ignored_events_return_error() {
        let (router, _) = setup_test_router("guild_1");
        let data = serde_json::json!({});
        let result = router.route_dispatch("MESSAGE_CREATE", &data);
        assert!(matches!(result, Err(EventRoutingError::IgnoredEvent(_))));
    }
}
