//! Discord API Handshake & Permission Validator (Phase 10.3)
//!
//! Provides pre-flight token format validation, REST identity verification against
//! Discord v10 `GET /users/@me`, privileged Gateway intent checking, and guild
//! administrative permission evaluation (`KICK_MEMBERS`, `BAN_MEMBERS`).

use crate::credentials::CredentialError;
use base64::Engine;
use serde::{Deserialize, Serialize};

/// Privileged Gateway Intent: `GUILD_MEMBERS` (`1 << 1`)
/// Essential for real-time `GUILD_MEMBER_ADD` and `GUILD_MEMBER_UPDATE` stream ingestion.
pub const INTENT_GUILD_MEMBERS: u64 = 1 << 1;

/// Official Discord Administrative Permission Bits:
/// - `KICK_MEMBERS`: Bit 1 (`1 << 1`)
/// - `BAN_MEMBERS`: Bit 2 (`1 << 2`)
/// - `ADMINISTRATOR`: Bit 3 (`1 << 3`) - Implicitly bypasses all individual permission checks
/// - `VIEW_CHANNEL`: Bit 10 (`1 << 10`)
/// - `MODERATE_MEMBERS`: Bit 40 (`1 << 40`)
pub const PERM_KICK_MEMBERS: u64 = 1 << 1;
pub const PERM_BAN_MEMBERS: u64 = 1 << 2;
pub const PERM_ADMINISTRATOR: u64 = 1 << 3;
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_MODERATE_MEMBERS: u64 = 1 << 40;

/// Validated segments extracted from a Discord Bot Token
#[derive(Debug, Clone)]
pub struct ValidatedTokenParts {
    pub clean_token: String,
    pub bot_snowflake_id: u64,
    pub raw_segment1: String,
    pub raw_segment2: String,
    pub raw_segment3: String,
}

/// Official Discord Bot User payload returned by `GET /users/@me`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: Option<bool>,
    #[serde(default)]
    pub flags: Option<u64>,
}

/// Guild summary returned by `GET /users/@me/guilds`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordGuildSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub permissions: Option<String>,
}

/// Evaluation breakdown of guild moderation permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildPermissionEvaluation {
    pub is_administrator: bool,
    pub has_kick_members: bool,
    pub has_ban_members: bool,
    pub has_moderate_members: bool,
    pub has_view_channel: bool,
    pub is_fully_authorized: bool,
    pub missing_permissions: Vec<String>,
}

/// Safe pre-flight handshake result summary (zero-plaintext, safe for IPC / UI presentation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeSummary {
    pub bot_id: String,
    pub bot_username: String,
    pub bot_discriminator: String,
    pub bot_avatar: Option<String>,
    pub is_official_bot: bool,
    pub target_guild_id: Option<String>,
    pub target_guild_name: Option<String>,
    pub permissions: Option<GuildPermissionEvaluation>,
    pub verified_at: i64,
}

/// Helper to decode the base64 snowflake from segment 1 of a Discord bot token
pub fn decode_snowflake_segment(seg: &str) -> Result<u64, CredentialError> {
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(seg)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(seg))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(seg))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(seg))
        .map_err(|_| CredentialError::InvalidBotTokenFormat)?;

    let snowflake_str =
        std::str::from_utf8(&decoded_bytes).map_err(|_| CredentialError::InvalidBotTokenFormat)?;

    let snowflake: u64 = snowflake_str
        .parse()
        .map_err(|_| CredentialError::InvalidBotTokenFormat)?;

    // Discord snowflake IDs are 64-bit integers; valid IDs are strictly positive and greater than minimum epoch offset
    if snowflake < 10_000_000_000_000 {
        return Err(CredentialError::InvalidBotTokenFormat);
    }

    Ok(snowflake)
}

/// Pre-check bot token format strictly according to the official 3-part base64 snowflake format.
pub fn validate_token_format(token: &str) -> Result<ValidatedTokenParts, CredentialError> {
    let trimmed = token.trim();
    let clean_token = trimmed.strip_prefix("Bot ").unwrap_or(trimmed);
    let parts: Vec<&str> = clean_token.split('.').collect();

    if parts.len() != 3 {
        return Err(CredentialError::InvalidBotTokenFormat);
    }

    let seg1 = parts[0];
    let seg2 = parts[1];
    let seg3 = parts[2];

    if seg1.len() < 18 || seg2.is_empty() || seg3.len() < 25 {
        return Err(CredentialError::InvalidBotTokenFormat);
    }

    let bot_snowflake = decode_snowflake_segment(seg1)?;

    Ok(ValidatedTokenParts {
        clean_token: clean_token.to_string(),
        bot_snowflake_id: bot_snowflake,
        raw_segment1: seg1.to_string(),
        raw_segment2: seg2.to_string(),
        raw_segment3: seg3.to_string(),
    })
}

/// Verifies that requested Gateway Intents include the required privileged `GUILD_MEMBERS` intent (`1 << 1`).
pub fn verify_privileged_intents(intents: u64) -> Result<(), CredentialError> {
    if (intents & INTENT_GUILD_MEMBERS) == 0 {
        return Err(CredentialError::MissingPrivilegedIntent(
            "GUILD_MEMBERS (1 << 1) privileged intent is required for real-time join stream adjudication"
                .to_string(),
        ));
    }
    Ok(())
}

/// Evaluates a bitwise permissions integer against required administrative moderation capabilities.
pub fn evaluate_guild_permissions(permission_bits: u64) -> GuildPermissionEvaluation {
    let is_admin = (permission_bits & PERM_ADMINISTRATOR) != 0;
    let has_kick = is_admin || (permission_bits & PERM_KICK_MEMBERS) != 0;
    let has_ban = is_admin || (permission_bits & PERM_BAN_MEMBERS) != 0;
    let has_moderate = is_admin || (permission_bits & PERM_MODERATE_MEMBERS) != 0;
    let has_view_channel = is_admin || (permission_bits & PERM_VIEW_CHANNEL) != 0;

    let mut missing = Vec::new();
    if !has_kick {
        missing.push("KICK_MEMBERS (1 << 1)".to_string());
    }
    if !has_ban {
        missing.push("BAN_MEMBERS (1 << 2)".to_string());
    }

    let is_fully_authorized = has_kick && has_ban;

    GuildPermissionEvaluation {
        is_administrator: is_admin,
        has_kick_members: has_kick,
        has_ban_members: has_ban,
        has_moderate_members: has_moderate,
        has_view_channel,
        is_fully_authorized,
        missing_permissions: missing,
    }
}

/// Client for conducting official Discord API handshakes and permission verifications.
pub struct DiscordHandshakeValidator {
    client: reqwest::Client,
    api_base_url: String,
}

impl Default for DiscordHandshakeValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscordHandshakeValidator {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_base_url: "https://discord.com/api/v10".to_string(),
        }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_base_url: base_url.into(),
        }
    }

    /// Pre-flight REST verification against `GET /users/@me`.
    /// Confirms bot identity and verifies that the account is an official Discord Bot (not a self-bot).
    pub async fn verify_bot_identity(&self, token: &str) -> Result<DiscordUser, CredentialError> {
        let parts = validate_token_format(token)?;
        let url = format!("{}/users/@me", self.api_base_url);

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bot {}", parts.clean_token))
            .header(
                "User-Agent",
                "DiscordBot (https://orangeheart.io/truthbeacon, 0.1.0)",
            )
            .send()
            .await
            .map_err(|e| CredentialError::ApiError(e.to_string()))?;

        let status = res.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CredentialError::UnauthorizedToken);
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = res
                .headers()
                .get("Retry-After")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(5.0);
            return Err(CredentialError::RateLimited(retry_after));
        } else if !status.is_success() {
            return Err(CredentialError::ApiError(format!(
                "Discord API returned HTTP status {}",
                status
            )));
        }

        let user: DiscordUser = res
            .json()
            .await
            .map_err(|e| CredentialError::ApiError(e.to_string()))?;

        // Enforce strict Discord Developer Policy: user tokens and self-bots are strictly prohibited.
        if user.bot != Some(true) {
            return Err(CredentialError::NonBotAccountRejected);
        }

        Ok(user)
    }

    /// Fetches all guilds configured for the bot and verifies administrative permissions for a target guild.
    pub async fn verify_guild_permissions(
        &self,
        token: &str,
        guild_id: &str,
    ) -> Result<(DiscordGuildSummary, GuildPermissionEvaluation), CredentialError> {
        let parts = validate_token_format(token)?;
        let url = format!("{}/users/@me/guilds", self.api_base_url);

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bot {}", parts.clean_token))
            .header(
                "User-Agent",
                "DiscordBot (https://orangeheart.io/truthbeacon, 0.1.0)",
            )
            .send()
            .await
            .map_err(|e| CredentialError::ApiError(e.to_string()))?;

        if !res.status().is_success() {
            return Err(CredentialError::ApiError(format!(
                "Failed to fetch bot guilds: HTTP {}",
                res.status()
            )));
        }

        let guilds: Vec<DiscordGuildSummary> = res
            .json()
            .await
            .map_err(|e| CredentialError::ApiError(e.to_string()))?;

        let target_guild = guilds
            .into_iter()
            .find(|g| g.id == guild_id)
            .ok_or_else(|| CredentialError::GuildMembershipNotFound(guild_id.to_string()))?;

        let perm_bits: u64 = target_guild
            .permissions
            .as_deref()
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);

        let evaluation = evaluate_guild_permissions(perm_bits);
        if !evaluation.is_fully_authorized {
            return Err(CredentialError::MissingGuildPermissions(
                evaluation.missing_permissions.clone(),
            ));
        }

        Ok((target_guild, evaluation))
    }

    /// Full end-to-end preflight handshake: validates token format, verifies `GET /users/@me`,
    /// and optionally verifies target guild membership and permissions.
    pub async fn execute_preflight_handshake(
        &self,
        token: &str,
        guild_id: Option<&str>,
    ) -> Result<HandshakeSummary, CredentialError> {
        let user = self.verify_bot_identity(token).await?;

        let (target_guild_id, target_guild_name, permissions) = match guild_id {
            Some(gid) if !gid.trim().is_empty() => {
                let (guild, eval) = self.verify_guild_permissions(token, gid).await?;
                (Some(guild.id), Some(guild.name), Some(eval))
            }
            _ => (None, None, None),
        };

        Ok(HandshakeSummary {
            bot_id: user.id,
            bot_username: user.username,
            bot_discriminator: user.discriminator,
            bot_avatar: user.avatar,
            is_official_bot: true,
            target_guild_id,
            target_guild_name,
            permissions,
            verified_at: chrono::Utc::now().timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_mock_bot_token(snowflake: u64) -> String {
        let snowflake_str = snowflake.to_string();
        let seg1 = base64::engine::general_purpose::STANDARD.encode(snowflake_str);
        let seg2 = "SIG_COMPONENT_TOKEN";
        let seg3 = "HMAC_SECRET_HASH_SIGNATURE_25_CHARS_LONG";
        format!("{}.{}.{}", seg1, seg2, seg3)
    }

    #[test]
    fn test_token_format_precheck_valid_snowflake() {
        let snowflake = 112233445566778899u64;
        let token = generate_mock_bot_token(snowflake);

        let parts = validate_token_format(&token).unwrap();
        assert_eq!(parts.bot_snowflake_id, snowflake);
        assert_eq!(parts.clean_token, token);

        let with_bot_prefix = format!("Bot {}", token);
        let parts_prefix = validate_token_format(&with_bot_prefix).unwrap();
        assert_eq!(parts_prefix.bot_snowflake_id, snowflake);
        assert_eq!(parts_prefix.clean_token, token);
    }

    #[test]
    fn test_token_format_precheck_rejects_malformed_and_user_tokens() {
        // Plain string without dots
        assert!(validate_token_format("unformatted_token_string").is_err());
        // Only 2 segments
        assert!(validate_token_format("segone.segtwo").is_err());
        // Empty
        assert!(validate_token_format("").is_err());
        // Invalid base64 in segment 1
        assert!(
            validate_token_format("!!!not_base64!!!.part2.part3_signature_is_long_enough").is_err()
        );
        // Non-numeric base64 decoded string in segment 1
        let non_num_b64 = base64::engine::general_purpose::STANDARD.encode("non_numeric_user_id");
        assert!(validate_token_format(&format!(
            "{}.part2.part3_signature_long_enough",
            non_num_b64
        ))
        .is_err());
        // Snowflake under minimum epoch
        let tiny_snowflake = base64::engine::general_purpose::STANDARD.encode("123");
        assert!(validate_token_format(&format!(
            "{}.part2.part3_signature_long_enough",
            tiny_snowflake
        ))
        .is_err());
    }

    #[test]
    fn test_privileged_intents_verification() {
        // GUILD_MEMBERS (1 << 1) included
        assert!(verify_privileged_intents(INTENT_GUILD_MEMBERS).is_ok());
        assert!(verify_privileged_intents(INTENT_GUILD_MEMBERS | (1 << 0) | (1 << 9)).is_ok());

        // GUILD_MEMBERS missing
        let err = verify_privileged_intents(1 << 0 | 1 << 9);
        assert!(err.is_err());
        match err.err().unwrap() {
            CredentialError::MissingPrivilegedIntent(msg) => {
                assert!(msg.contains("GUILD_MEMBERS"));
            }
            _ => panic!("Expected MissingPrivilegedIntent"),
        }
    }

    #[test]
    fn test_guild_permissions_evaluation_admin() {
        // Administrator bit grants everything implicitly
        let eval = evaluate_guild_permissions(PERM_ADMINISTRATOR);
        assert!(eval.is_administrator);
        assert!(eval.has_kick_members);
        assert!(eval.has_ban_members);
        assert!(eval.is_fully_authorized);
        assert!(eval.missing_permissions.is_empty());
    }

    #[test]
    fn test_guild_permissions_evaluation_explicit_kick_and_ban() {
        let perms = PERM_KICK_MEMBERS | PERM_BAN_MEMBERS | PERM_VIEW_CHANNEL;
        let eval = evaluate_guild_permissions(perms);
        assert!(!eval.is_administrator);
        assert!(eval.has_kick_members);
        assert!(eval.has_ban_members);
        assert!(eval.has_view_channel);
        assert!(eval.is_fully_authorized);
        assert!(eval.missing_permissions.is_empty());
    }

    #[test]
    fn test_guild_permissions_evaluation_missing_kick_or_ban() {
        // Only kick, missing ban
        let eval_kick_only = evaluate_guild_permissions(PERM_KICK_MEMBERS);
        assert!(!eval_kick_only.is_fully_authorized);
        assert!(eval_kick_only
            .missing_permissions
            .iter()
            .any(|p| p.contains("BAN_MEMBERS")));

        // Only ban, missing kick
        let eval_ban_only = evaluate_guild_permissions(PERM_BAN_MEMBERS);
        assert!(!eval_ban_only.is_fully_authorized);
        assert!(eval_ban_only
            .missing_permissions
            .iter()
            .any(|p| p.contains("KICK_MEMBERS")));

        // No permissions
        let eval_none = evaluate_guild_permissions(0);
        assert!(!eval_none.is_fully_authorized);
        assert_eq!(eval_none.missing_permissions.len(), 2);
    }

    // Offline mock HTTP server helper for testing Discord REST handshake
    async fn spawn_mock_discord_server(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, tokio::sync::oneshot::Sender<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            tokio::select! {
                _ = &mut shutdown_rx => {}
                res = listener.accept() => {
                    if let Ok((mut socket, _)) = res {
                        use tokio::io::{AsyncReadExt, AsyncWriteExt};
                        let mut buf = [0u8; 2048];
                        let _ = socket.read(&mut buf).await;
                        let response = format!(
                            "{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            status_line,
                            body.len(),
                            body
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.shutdown().await;
                    }
                }
            }
        });

        (format!("http://{}", addr), shutdown_tx)
    }

    #[tokio::test]
    async fn test_offline_mock_preflight_handshake_success() {
        let mock_body = r#"{"id":"112233445566778899","username":"TruthBeaconBot","discriminator":"0001","bot":true}"#;
        let (base_url, _tx) = spawn_mock_discord_server("HTTP/1.1 200 OK", mock_body).await;

        let validator = DiscordHandshakeValidator::with_base_url(base_url);
        let token = generate_mock_bot_token(112233445566778899);

        let user = validator.verify_bot_identity(&token).await.unwrap();
        assert_eq!(user.id, "112233445566778899");
        assert_eq!(user.username, "TruthBeaconBot");
        assert_eq!(user.bot, Some(true));
    }

    #[tokio::test]
    async fn test_offline_mock_preflight_handshake_rejects_user_token() {
        // Simulates a user token or self-bot where bot == false
        let mock_body = r#"{"id":"112233445566778899","username":"RegularUserAccount","discriminator":"1234","bot":false}"#;
        let (base_url, _tx) = spawn_mock_discord_server("HTTP/1.1 200 OK", mock_body).await;

        let validator = DiscordHandshakeValidator::with_base_url(base_url);
        let token = generate_mock_bot_token(112233445566778899);

        let res = validator.verify_bot_identity(&token).await;
        assert!(res.is_err());
        match res.err().unwrap() {
            CredentialError::NonBotAccountRejected => {}
            other => panic!("Expected NonBotAccountRejected, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_offline_mock_preflight_handshake_unauthorized() {
        let mock_body = r#"{"message":"401: Unauthorized","code":0}"#;
        let (base_url, _tx) =
            spawn_mock_discord_server("HTTP/1.1 401 Unauthorized", mock_body).await;

        let validator = DiscordHandshakeValidator::with_base_url(base_url);
        let token = generate_mock_bot_token(112233445566778899);

        let res = validator.verify_bot_identity(&token).await;
        assert!(res.is_err());
        match res.err().unwrap() {
            CredentialError::UnauthorizedToken => {}
            other => panic!("Expected UnauthorizedToken, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_offline_mock_preflight_handshake_rate_limited() {
        let mock_body =
            r#"{"message":"You are being rate limited.","retry_after":3.5,"global":false}"#;
        let (base_url, _tx) =
            spawn_mock_discord_server("HTTP/1.1 429 Too Many Requests", mock_body).await;

        let validator = DiscordHandshakeValidator::with_base_url(base_url);
        let token = generate_mock_bot_token(112233445566778899);

        let res = validator.verify_bot_identity(&token).await;
        assert!(res.is_err());
        match res.err().unwrap() {
            CredentialError::RateLimited(secs) => {
                assert!(secs > 0.0);
            }
            other => panic!("Expected RateLimited, got {:?}", other),
        }
    }
}
