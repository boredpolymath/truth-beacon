//! Ingestion & Import Mechanisms for Canonical Benchmark Identities (Phase 12.2).
//!
//! Provides:
//! - **Batch Role Ingestion**: Import all guild members assigned a specific Discord role (e.g., "Admin", "Moderator").
//! - **Manual Snowflake Ingestion**: Real-time Discord REST fetching and perceptual avatar hashing for specific user IDs.
//! - **Triage Stream Promotion**: One-click promotion from the incident stream directly into the benchmark vault,
//!   synchronizing incident resolution and audit logs.

use crate::detection::perceptual_hash::compute_perceptual_hash;
use crate::models::{CanonicalBenchmark, CreateBenchmarkInput, UpdateBenchmarkInput};
use crate::vault::{VaultError, VaultManager};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Error types occurring during ingestion and import workflows.
#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("Vault error: {0}")]
    Vault(#[from] VaultError),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Discord user '{0}' not found")]
    UserNotFound(String),

    #[error("Discord API error: HTTP {0} - {1}")]
    DiscordApi(u16, String),
}

/// Official Discord Guild Member payload returned by `/guilds/{guild_id}/members`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordGuildMember {
    pub user: DiscordMemberUser,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub joined_at: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
}

/// Discord User object embedded inside Member or retrieved from `/users/{user_id}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMemberUser {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: Option<bool>,
}

/// Options configured when batch-importing members assigned a Discord role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleImportOptions {
    pub guild_id: String,
    pub role_id: String,
    #[serde(default)]
    pub role_name: Option<String>,
    #[serde(default)]
    pub community_role: Option<String>,
    #[serde(default)]
    pub additional_tags: Vec<String>,
    #[serde(default)]
    pub compute_avatar_hashes: bool,
    #[serde(default)]
    pub overwrite_existing: bool,
}

/// Structured execution summary for role-based batch ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleImportReport {
    pub guild_id: String,
    pub role_id: String,
    pub total_members_scanned: usize,
    pub matching_role_members: usize,
    pub imported_count: usize,
    pub updated_count: usize,
    pub skipped_count: usize,
    pub benchmarks: Vec<CanonicalBenchmark>,
}

/// Payload for manual Snowflake ID entry with live REST profile lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualSnowflakeImportInput {
    pub guild_id: String,
    pub user_id: String,
    #[serde(default)]
    pub community_role: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub sensitivity_override: Option<f64>,
    #[serde(default)]
    pub compute_avatar_hash: bool,
}

/// Payload for promoting an incident suspect/candidate from the Triage Stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionFromTriageInput {
    pub guild_id: String,
    pub user_id: String,
    pub canonical_username: String,
    pub server_nickname: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_perceptual_hash: Option<String>,
    pub community_role: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sensitivity_override: Option<f64>,
    pub incident_id: Option<String>,
    pub operator_id: Option<String>,
    pub resolution_notes: Option<String>,
}

/// Audit report returned when one-click promotion succeeds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionResult {
    pub benchmark: CanonicalBenchmark,
    pub incident_id: Option<String>,
    pub incident_status_updated: bool,
    pub audit_log_id: Option<String>,
}

/// HTTP REST Client for Discord API member and profile ingestion.
#[derive(Clone)]
pub struct IngestionClient {
    client: reqwest::Client,
    api_base_url: String,
    cdn_base_url: Option<String>,
}

impl Default for IngestionClient {
    fn default() -> Self {
        Self::new()
    }
}

impl IngestionClient {
    /// Initialize with production Discord v10 REST endpoint.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_base_url: "https://discord.com/api/v10".to_string(),
            cdn_base_url: None,
        }
    }

    /// Custom base URL constructor for offline testing and mock HTTP servers.
    pub fn with_base_url(url: impl Into<String>) -> Self {
        let base = url.into();
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_base_url: base.clone(),
            cdn_base_url: Some(base),
        }
    }

    /// Returns the configured CDN base URL if overridden for offline testing.
    /// Returns the configured CDN base URL if overridden for offline testing.
    pub fn cdn_base_url(&self) -> Option<&str> {
        self.cdn_base_url.as_deref()
    }

    /// Fetch a single guild member via `GET /guilds/{guild_id}/members/{target_snowflake}`.
    pub async fn fetch_guild_member(
        &self,
        bot_token: &str,
        guild_id: &str,
        target_snowflake: &str,
    ) -> Result<DiscordGuildMember, IngestionError> {
        let clean_token = bot_token.trim().strip_prefix("Bot ").unwrap_or(bot_token);
        let validated_snowflake: u64 = target_snowflake.trim().parse().map_err(|_| {
            IngestionError::Validation("Target snowflake must be a numeric ID".into())
        })?;
        let url = format!(
            "{}/guilds/{}/members/{}",
            self.api_base_url, guild_id, validated_snowflake
        );

        if !is_secure_endpoint(&url) {
            return Err(IngestionError::Validation(
                "Cleartext HTTP transmission is disabled; endpoints must use HTTPS".into(),
            ));
        }

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bot {}", clean_token))
            .header("User-Agent", "TruthBeacon/0.1.0")
            .send()
            .await?;

        let status = res.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(IngestionError::UserNotFound(validated_snowflake.to_string()));
        } else if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(IngestionError::DiscordApi(status.as_u16(), body));
        }

        let member = res.json::<DiscordGuildMember>().await?;
        Ok(member)
    }

    /// Fetch a user profile fallback via `GET /users/{target_snowflake}`.
    pub async fn fetch_user(
        &self,
        bot_token: &str,
        target_snowflake: &str,
    ) -> Result<DiscordMemberUser, IngestionError> {
        let clean_token = bot_token.trim().strip_prefix("Bot ").unwrap_or(bot_token);
        let validated_snowflake: u64 = target_snowflake.trim().parse().map_err(|_| {
            IngestionError::Validation("Target snowflake must be a numeric ID".into())
        })?;
        let url = format!("{}/users/{}", self.api_base_url, validated_snowflake);

        if !is_secure_endpoint(&url) {
            return Err(IngestionError::Validation(
                "Cleartext HTTP transmission is disabled; endpoints must use HTTPS".into(),
            ));
        }

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bot {}", clean_token))
            .header("User-Agent", "TruthBeacon/0.1.0")
            .send()
            .await?;

        let status = res.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(IngestionError::UserNotFound(validated_snowflake.to_string()));
        } else if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(IngestionError::DiscordApi(status.as_u16(), body));
        }

        let user = res.json::<DiscordMemberUser>().await?;
        Ok(user)
    }

    /// Fetch guild members via `GET /guilds/{guild_id}/members?limit={limit}`.
    pub async fn fetch_guild_members(
        &self,
        bot_token: &str,
        guild_id: &str,
        limit: u32,
    ) -> Result<Vec<DiscordGuildMember>, IngestionError> {
        let clean_token = bot_token.trim().strip_prefix("Bot ").unwrap_or(bot_token);
        let url = format!(
            "{}/guilds/{}/members?limit={}",
            self.api_base_url, guild_id, limit
        );

        if !is_secure_endpoint(&url) {
            return Err(IngestionError::Validation(
                "Cleartext HTTP transmission is disabled; endpoints must use HTTPS".into(),
            ));
        }

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bot {}", clean_token))
            .header("User-Agent", "TruthBeacon/0.1.0")
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(IngestionError::DiscordApi(status.as_u16(), body));
        }

        let members = res.json::<Vec<DiscordGuildMember>>().await?;
        Ok(members)
    }

    /// Downloads image bytes from avatar URL and calculates a 64-bit DCT perceptual hash.
    pub async fn compute_avatar_hash(&self, avatar_url: &str) -> Option<String> {
        if !is_secure_endpoint(avatar_url) {
            log::warn!("Rejected insecure cleartext avatar URL: {}", avatar_url);
            return None;
        }
        let res = self.client.get(avatar_url).send().await.ok()?;
        if !res.status().is_success() {
            return None;
        }
        let bytes = res.bytes().await.ok()?;
        compute_perceptual_hash(&bytes).ok()
    }
}

/// Validates that an endpoint URL is transmitted securely over TLS (HTTPS),
/// or loopback addresses exclusively for offline integration tests.
pub fn is_secure_endpoint(endpoint_url: &str) -> bool {
    let lower = endpoint_url.to_ascii_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://127.0.0.1")
        || lower.starts_with("http://localhost")
        || lower.starts_with("http://[::1]")
}

/// Resolves standard Discord CDN Avatar URL for a guild member or user.
pub fn resolve_discord_avatar_url(
    user: &DiscordMemberUser,
    guild_id: Option<&str>,
    member_avatar: Option<&str>,
) -> Option<String> {
    resolve_discord_avatar_url_with_cdn(None, user, guild_id, member_avatar)
}

/// Resolves Discord CDN Avatar URL, allowing a custom CDN base for offline testing.
pub fn resolve_discord_avatar_url_with_cdn(
    cdn_base: Option<&str>,
    user: &DiscordMemberUser,
    guild_id: Option<&str>,
    member_avatar: Option<&str>,
) -> Option<String> {
    let base = cdn_base.unwrap_or("https://cdn.discordapp.com");
    let snowflake_num: u64 = user.id.trim().parse().unwrap_or(0);
    if let (Some(gid), Some(m_av)) = (guild_id, member_avatar) {
        return Some(format!(
            "{}/guilds/{}/users/{}/avatars/{}.png?size=256",
            base, gid, snowflake_num, m_av
        ));
    }

    if let Some(ref av) = user.avatar {
        return Some(format!("{}/avatars/{}/{}.png?size=256", base, snowflake_num, av));
    }

    // Default avatar
    let discriminator_num: u64 = user.discriminator.parse().unwrap_or(0);
    let index = if discriminator_num != 0 {
        discriminator_num % 5
    } else {
        (snowflake_num >> 22) % 6
    };
    Some(format!("{}/embed/avatars/{}.png", base, index))
}

/// 12.2.1: Import from Existing Discord Server Roles (In-memory slice execution).
///
/// Filters the provided list of guild members for those holding `role_id`, converts each
/// to a CanonicalBenchmark, handles duplicate resolution, and writes to SQLite & cache.
pub fn import_members_from_list(
    vault_mgr: &VaultManager,
    members: &[DiscordGuildMember],
    options: RoleImportOptions,
) -> Result<RoleImportReport, IngestionError> {
    let mut imported_benchmarks = Vec::new();
    let mut matching_count = 0;
    let mut imported_count = 0;
    let mut updated_count = 0;
    let mut skipped_count = 0;

    let role_display = options
        .role_name
        .clone()
        .unwrap_or_else(|| format!("Role {}", options.role_id));
    let community_role = options
        .community_role
        .clone()
        .unwrap_or_else(|| role_display.clone());

    for member in members {
        if !member.roles.iter().any(|r| r == &options.role_id) {
            continue;
        }
        matching_count += 1;

        let user_id = &member.user.id;
        let existing = vault_mgr.get_benchmark_by_user(&options.guild_id, user_id)?;

        let mut tags = options.additional_tags.clone();
        let role_tag = format!("Role: {}", role_display);
        if !tags.contains(&role_tag) {
            tags.push(role_tag);
        }

        let avatar_url = resolve_discord_avatar_url_with_cdn(
            None,
            &member.user,
            Some(&options.guild_id),
            member.avatar.as_deref(),
        );

        if let Some(existing_bm) = existing {
            if options.overwrite_existing {
                let updated = vault_mgr.update_benchmark(
                    &existing_bm.id,
                    UpdateBenchmarkInput {
                        server_nickname: member.nick.clone(),
                        community_role: Some(community_role.clone()),
                        avatar_url,
                        tags: Some(tags),
                        is_active: Some(true),
                        ..Default::default()
                    },
                )?;
                imported_benchmarks.push(updated);
                updated_count += 1;
            } else {
                skipped_count += 1;
            }
        } else {
            let canonical_name = member
                .user
                .global_name
                .clone()
                .unwrap_or_else(|| member.user.username.clone());

            let created = vault_mgr.create_benchmark(
                CreateBenchmarkInput {
                    guild_id: options.guild_id.clone(),
                    user_id: user_id.clone(),
                    canonical_username: canonical_name,
                    server_nickname: member.nick.clone(),
                    community_role: community_role.clone(),
                    avatar_url,
                    tags,
                    sensitivity_override: None,
                },
                None, // Avatar hash can be computed or updated in background
            )?;
            imported_benchmarks.push(created);
            imported_count += 1;
        }
    }

    Ok(RoleImportReport {
        guild_id: options.guild_id,
        role_id: options.role_id,
        total_members_scanned: members.len(),
        matching_role_members: matching_count,
        imported_count,
        updated_count,
        skipped_count,
        benchmarks: imported_benchmarks,
    })
}

/// 12.2.1: Full asynchronous role import fetching live members via Discord REST.
pub async fn import_members_by_role(
    vault_mgr: &VaultManager,
    ingestion_client: &IngestionClient,
    bot_token: &str,
    options: RoleImportOptions,
) -> Result<RoleImportReport, IngestionError> {
    let members = ingestion_client
        .fetch_guild_members(bot_token, &options.guild_id, 1000)
        .await?;

    let mut report = import_members_from_list(vault_mgr, &members, options.clone())?;

    // Optional perceptual hash computation for imported members with avatars
    if options.compute_avatar_hashes {
        for bm in &mut report.benchmarks {
            if let Some(ref url) = bm.avatar_url {
                if let Some(hash) = ingestion_client.compute_avatar_hash(url).await {
                    let updated = vault_mgr.update_benchmark(
                        &bm.id,
                        UpdateBenchmarkInput {
                            avatar_perceptual_hash: Some(hash),
                            ..Default::default()
                        },
                    )?;
                    *bm = updated;
                }
            }
        }
    }

    Ok(report)
}

/// 12.2.2: Manual entry by Discord Snowflake ID with live profile fetching via Discord REST API.
pub async fn import_benchmark_by_snowflake(
    vault_mgr: &VaultManager,
    ingestion_client: &IngestionClient,
    bot_token: &str,
    input: ManualSnowflakeImportInput,
) -> Result<CanonicalBenchmark, IngestionError> {
    let snowflake_val: u64 = input
        .user_id
        .trim()
        .parse()
        .map_err(|_| IngestionError::Validation("Discord User ID must be a numeric Snowflake ID".into()))?;
    let target_snowflake = snowflake_val.to_string();

    // First attempt to fetch guild member to retrieve guild nickname and guild avatar
    let (canonical_name, nickname, avatar_url) = match ingestion_client
        .fetch_guild_member(bot_token, &input.guild_id, &target_snowflake)
        .await
    {
        Ok(member) => {
            let name = member
                .user
                .global_name
                .clone()
                .unwrap_or_else(|| member.user.username.clone());
            let av = resolve_discord_avatar_url_with_cdn(
                ingestion_client.cdn_base_url(),
                &member.user,
                Some(&input.guild_id),
                member.avatar.as_deref(),
            );
            (name, member.nick, av)
        }
        Err(IngestionError::UserNotFound(_)) | Err(IngestionError::DiscordApi(404, _)) => {
            // Fallback: Query global user profile
            let user = ingestion_client.fetch_user(bot_token, &target_snowflake).await?;
            let name = user
                .global_name
                .clone()
                .unwrap_or_else(|| user.username.clone());
            let av = resolve_discord_avatar_url_with_cdn(
                ingestion_client.cdn_base_url(),
                &user,
                None,
                None,
            );
            (name, None, av)
        }
        Err(e) => return Err(e),
    };

    // Calculate avatar hash if requested
    let mut avatar_hash = None;
    if input.compute_avatar_hash {
        if let Some(ref url) = avatar_url {
            avatar_hash = ingestion_client.compute_avatar_hash(url).await;
        }
    }

    let community_role = input
        .community_role
        .unwrap_or_else(|| "Protected Benchmark".into());

    let mut tags = input.tags;
    if tags.is_empty() {
        tags.push("Manual Entry".into());
    }

    let created = vault_mgr.create_benchmark(
        CreateBenchmarkInput {
            guild_id: input.guild_id,
            user_id: target_snowflake,
            canonical_username: canonical_name,
            server_nickname: nickname,
            community_role,
            avatar_url,
            tags,
            sensitivity_override: input.sensitivity_override,
        },
        avatar_hash,
    )?;

    Ok(created)
}

/// 12.2.3: One-click promotion from Triage Stream context menu ("Add as Protected Benchmark").
///
/// Promotes a suspect or candidate user identified in an incident stream into a canonical benchmark,
/// automatically whitelisting the triggering incident and generating an immutable audit trail.
pub fn promote_from_triage(
    vault_mgr: &VaultManager,
    input: PromotionFromTriageInput,
) -> Result<PromotionResult, IngestionError> {
    let mut tags = input.tags;
    if !tags.iter().any(|t| t == "Promoted from Triage") {
        tags.push("Promoted from Triage".into());
    }

    // 1. Create canonical benchmark in vault & SQLite
    let benchmark = vault_mgr.create_benchmark(
        CreateBenchmarkInput {
            guild_id: input.guild_id.clone(),
            user_id: input.user_id.clone(),
            canonical_username: input.canonical_username,
            server_nickname: input.server_nickname,
            community_role: input.community_role,
            avatar_url: input.avatar_url,
            tags,
            sensitivity_override: input.sensitivity_override,
        },
        input.avatar_perceptual_hash,
    )?;

    let mut incident_status_updated = false;
    let mut audit_log_id = None;

    // 2. If incident_id is provided, resolve incident and write audit log
    if let Some(ref inc_id) = input.incident_id {
        let conn = vault_mgr.storage().get_connection();
        let conn = conn.lock().unwrap();

        let now = chrono::Utc::now().timestamp();
        let notes = input
            .resolution_notes
            .unwrap_or_else(|| format!("Promoted to Canonical Benchmark ID: {}", benchmark.id));
        let operator = input.operator_id.as_deref().unwrap_or("CONSOLE_OPERATOR");

        // Update incident to 'whitelisted'
        let rows = conn.execute(
            "UPDATE incidents SET
                status = 'whitelisted',
                resolution_notes = ?1,
                operator_id = ?2,
                resolved_at = ?3
             WHERE id = ?4;",
            params![notes, operator, now, inc_id],
        )?;
        incident_status_updated = rows > 0;

        // Insert audit log
        let audit_id = format!("audit_{}_{}", now, benchmark.id);
        let meta = serde_json::json!({
            "action": "PROMOTED_TO_BENCHMARK",
            "benchmark_id": benchmark.id,
            "user_id": benchmark.user_id,
            "guild_id": benchmark.guild_id,
            "community_role": benchmark.community_role,
        })
        .to_string();

        conn.execute(
            "INSERT INTO audit_logs (
                id, timestamp, action, guild_id, operator_id, target_user_id,
                incident_id, reason, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
            params![
                audit_id,
                now,
                "PROMOTED_TO_BENCHMARK",
                benchmark.guild_id,
                operator,
                benchmark.user_id,
                inc_id,
                "Operator promoted incident suspect to protected canonical benchmark profile",
                meta,
            ],
        )?;

        audit_log_id = Some(audit_id);
    }

    Ok(PromotionResult {
        benchmark,
        incident_id: input.incident_id,
        incident_status_updated,
        audit_log_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_role_import_from_member_list() {
        let vault_mgr = VaultManager::in_memory().unwrap();
        let guild_id = "guild_role_import";
        let admin_role = "role_admin_123";

        let members = vec![
            DiscordGuildMember {
                user: DiscordMemberUser {
                    id: "10001".into(),
                    username: "alice_admin".into(),
                    discriminator: "0".into(),
                    global_name: Some("Alice Admin".into()),
                    avatar: Some("av_alice".into()),
                    bot: Some(false),
                },
                nick: Some("Alice [Lead]".into()),
                roles: vec![admin_role.into(), "role_general".into()],
                joined_at: None,
                avatar: None,
            },
            DiscordGuildMember {
                user: DiscordMemberUser {
                    id: "10002".into(),
                    username: "bob_member".into(),
                    discriminator: "0".into(),
                    global_name: Some("Bob Member".into()),
                    avatar: None,
                    bot: Some(false),
                },
                nick: None,
                roles: vec!["role_general".into()], // Does not have admin role
                joined_at: None,
                avatar: None,
            },
            DiscordGuildMember {
                user: DiscordMemberUser {
                    id: "10003".into(),
                    username: "charlie_admin".into(),
                    discriminator: "0".into(),
                    global_name: None,
                    avatar: Some("av_charlie".into()),
                    bot: Some(false),
                },
                nick: None,
                roles: vec![admin_role.into()],
                joined_at: None,
                avatar: None,
            },
        ];

        let options = RoleImportOptions {
            guild_id: guild_id.into(),
            role_id: admin_role.into(),
            role_name: Some("Server Administrator".into()),
            community_role: Some("Executive Leadership".into()),
            additional_tags: vec!["Core Staff".into()],
            compute_avatar_hashes: false,
            overwrite_existing: false,
        };

        let report = import_members_from_list(&vault_mgr, &members, options).unwrap();

        assert_eq!(report.total_members_scanned, 3);
        assert_eq!(report.matching_role_members, 2);
        assert_eq!(report.imported_count, 2);
        assert_eq!(report.skipped_count, 0);

        // Verify benchmarks exist in vault
        let benchmarks = vault_mgr.list_benchmarks(guild_id, true).unwrap();
        assert_eq!(benchmarks.len(), 2);

        let alice = benchmarks.iter().find(|b| b.user_id == "10001").unwrap();
        assert_eq!(alice.canonical_username, "Alice Admin");
        assert_eq!(alice.server_nickname.as_deref(), Some("Alice [Lead]"));
        assert_eq!(alice.community_role, "Executive Leadership");
        assert!(alice.tags.contains(&"Core Staff".to_string()));
        assert!(alice
            .tags
            .contains(&"Role: Server Administrator".to_string()));

        // Test second import with overwrite_existing = false (should skip)
        let report_dup = import_members_from_list(
            &vault_mgr,
            &members,
            RoleImportOptions {
                guild_id: guild_id.into(),
                role_id: admin_role.into(),
                role_name: Some("Server Administrator".into()),
                community_role: None,
                additional_tags: vec![],
                compute_avatar_hashes: false,
                overwrite_existing: false,
            },
        )
        .unwrap();

        assert_eq!(report_dup.imported_count, 0);
        assert_eq!(report_dup.skipped_count, 2);
    }

    #[test]
    fn test_one_click_promotion_from_triage_stream() {
        let vault_mgr = VaultManager::in_memory().unwrap();
        let guild_id = "guild_triage_test";
        let incident_id = "inc_test_999";
        let user_id = "555666777888";

        // Seed an active incident in the SQLite database
        {
            let conn = vault_mgr.storage().get_connection();
            let conn = conn.lock().unwrap();

            // First create a benchmark to satisfy foreign key constraint on incidents(matched_benchmark_id)
            conn.execute(
                "INSERT INTO benchmarks (
                    id, guild_id, user_id, canonical_username, community_role,
                    is_active, created_at, updated_at
                 ) VALUES ('bm_seed', 'guild_triage_test', '111', 'ElderJohn', 'Staff', 1, 1000, 1000);",
                [],
            )
            .unwrap();

            conn.execute(
                "INSERT INTO incidents (
                    id, guild_id, timestamp, suspect_user_id, suspect_username,
                    suspect_nickname, suspect_avatar_url, suspect_account_age_hours,
                    matched_benchmark_id, matched_benchmark_name, string_similarity_score,
                    homoglyph_detected, normalized_diff, risk_tier, status
                 ) VALUES (
                    ?1, ?2, 1000, ?3, 'PastorDanAlt', 'Dan [Alt]', 'https://av.png', 48,
                    'bm_seed', 'ElderJohn', 0.88, 0, 'diff', 'notable', 'pending'
                 );",
                params![incident_id, guild_id, user_id],
            )
            .unwrap();
        }

        let input = PromotionFromTriageInput {
            guild_id: guild_id.into(),
            user_id: user_id.into(),
            canonical_username: "Pastor Dan (Official Alt)".into(),
            server_nickname: Some("Dan | Alt Account".into()),
            avatar_url: Some("https://av.png".into()),
            avatar_perceptual_hash: Some("abcd1234efgh5678".into()),
            community_role: "Authorized Alt".into(),
            tags: vec!["Authorized Alt".into(), "Staff".into()],
            sensitivity_override: Some(0.80),
            incident_id: Some(incident_id.into()),
            operator_id: Some("op_moderator_1".into()),
            resolution_notes: Some("Verified in voice chat as genuine alt".into()),
        };

        let result = promote_from_triage(&vault_mgr, input).unwrap();

        assert_eq!(result.benchmark.user_id, user_id);
        assert_eq!(result.benchmark.community_role, "Authorized Alt");
        assert!(result
            .benchmark
            .tags
            .contains(&"Promoted from Triage".to_string()));
        assert!(result.incident_status_updated);
        assert!(result.audit_log_id.is_some());

        // Verify incident in SQLite was resolved to 'whitelisted'
        let reader = vault_mgr.storage().open_reader().unwrap();
        let (status, notes, operator): (String, Option<String>, Option<String>) = reader
            .query_row(
                "SELECT status, resolution_notes, operator_id FROM incidents WHERE id = ?1",
                [incident_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();

        assert_eq!(status, "whitelisted");
        assert_eq!(
            notes.as_deref(),
            Some("Verified in voice chat as genuine alt")
        );
        assert_eq!(operator.as_deref(), Some("op_moderator_1"));

        // Verify audit log row in SQLite
        let (action, audit_user): (String, String) = reader
            .query_row(
                "SELECT action, target_user_id FROM audit_logs WHERE incident_id = ?1",
                [incident_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();

        assert_eq!(action, "PROMOTED_TO_BENCHMARK");
        assert_eq!(audit_user, user_id);
    }

    #[tokio::test]
    async fn test_manual_snowflake_import_offline_mock() {
        let vault_mgr = VaultManager::in_memory().unwrap();
        let mock_body = r#"{
            "user": {
                "id": "999888777666",
                "username": "vip_leader",
                "discriminator": "0001",
                "global_name": "VIP Leader Global",
                "avatar": "avatar_hash_xyz",
                "bot": false
            },
            "nick": "Leader | VIP",
            "roles": ["role_vip"]
        }"#;

        // Spawn mock Discord REST server
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            tokio::select! {
                _ = &mut shutdown_rx => {}
                res = listener.accept() => {
                    if let Ok((mut socket, _)) = res {
                        use tokio::io::AsyncWriteExt;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            mock_body.len(),
                            mock_body
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                }
            }
        });

        let client = IngestionClient::with_base_url(format!("http://{}", addr));

        let input = ManualSnowflakeImportInput {
            guild_id: "guild_mock_snowflake".into(),
            user_id: "999888777666".into(),
            community_role: Some("Verified VIP".into()),
            tags: vec!["VIP".into()],
            sensitivity_override: Some(0.90),
            compute_avatar_hash: false,
        };

        let benchmark = import_benchmark_by_snowflake(&vault_mgr, &client, "Bot mock_token", input)
            .await
            .unwrap();

        let _ = shutdown_tx.send(());

        assert_eq!(benchmark.user_id, "999888777666");
        assert_eq!(benchmark.canonical_username, "VIP Leader Global");
        assert_eq!(benchmark.server_nickname.as_deref(), Some("Leader | VIP"));
        assert_eq!(benchmark.community_role, "Verified VIP");
        assert!(benchmark.avatar_url.unwrap().contains("avatar_hash_xyz"));
        assert_eq!(benchmark.sensitivity_override, Some(0.90));
    }
}
