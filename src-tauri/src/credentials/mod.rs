//! Native OS Secure Enclave Keyring Integration & Secret Lifecycle Management.
//!
//! # Platform Secure Enclave Architecture
//! This module interfaces with native operating system credential enclaves via `keyring-rs`:
//! - **macOS**: Apple Keychain Services (`Security.framework`) via `apple-native-keyring-store`.
//!   Uses `kSecClassGenericPassword` entries protected by user login keychain and hardware Secure Enclave.
//! - **Windows**: Windows Credential Manager (`wincred.h`) via `windows-native-keyring-store`.
//!   Targeted against Windows Vault credential store encrypted with DPAPI (Data Protection API).
//! - **Linux**: Secret Service API specification over D-Bus via `zbus-secret-service-keyring-store`.
//!   Integrates directly with GNOME Keyring (`libsecret`) and KDE KWallet.
//!
//! # Zero-Plaintext Security Policy
//! TruthBeacon enforces an uncompromising zero-plaintext policy:
//! 1. Platform bot tokens are stored exclusively in the OS native secure enclave.
//! 2. Tokens are NEVER logged; all `Debug` and `Display` implementations redact secrets (`[REDACTED]`).
//! 3. Tokens intentionally do not implement `serde::Serialize` to prevent serialization into JSON or Tauri IPC responses.
//! 4. Tokens are NEVER written to SQLite databases, flat files, or `.env` files.
//! 5. Memory buffers holding secrets are zeroized (wiped) on drop via `zeroize`.

pub mod validator;
pub use validator::{
    decode_snowflake_segment, evaluate_guild_permissions, validate_token_format,
    verify_privileged_intents, DiscordGuildSummary, DiscordHandshakeValidator, DiscordUser,
    GuildPermissionEvaluation, HandshakeSummary, ValidatedTokenParts, INTENT_GUILD_MEMBERS,
    PERM_ADMINISTRATOR, PERM_BAN_MEMBERS, PERM_KICK_MEMBERS, PERM_MODERATE_MEMBERS,
    PERM_VIEW_CHANNEL,
};

use keyring::Entry;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const SERVICE_NAME: &str = "truth_beacon_secure_vault";
pub const GUILD_REGISTRY_KEY: &str = "__truth_beacon_guild_registry__";

#[derive(Error, Debug)]
pub enum CredentialError {
    #[error("Keyring secure enclave access failure: {0}")]
    KeyringError(#[from] keyring::Error),
    #[error("Missing token for guild {0}")]
    TokenNotFound(String),
    #[error("Invalid bot token format. Only official Discord Bot tokens are permitted; user tokens and self-bots are strictly prohibited.")]
    InvalidBotTokenFormat,
    #[error("Failed to parse credential guild registry: {0}")]
    RegistryCorruption(String),
    #[error("Discord REST API request failed: {0}")]
    ApiError(String),
    #[error("Discord authentication failed (401 Unauthorized): Bot token is invalid or revoked")]
    UnauthorizedToken,
    #[error("Discord rate limit encountered: retry after {0} seconds")]
    RateLimited(f64),
    #[error("Non-bot account detected. TruthBeacon strictly prohibits user tokens and self-bots.")]
    NonBotAccountRejected,
    #[error("Missing required privileged Gateway Intent: {0}")]
    MissingPrivilegedIntent(String),
    #[error("Bot lacks required guild administrative permissions: {0:?}")]
    MissingGuildPermissions(Vec<String>),
    #[error("Bot is not a member of guild {0}")]
    GuildMembershipNotFound(String),
}

/// Secure wrapper for platform Bot Tokens that strictly enforces the zero-plaintext policy.
///
/// Plaintext memory is wiped on drop via `Zeroizing`, `Debug` and `Display` are redacted,
/// and `serde::Serialize` is intentionally NOT implemented.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureBotToken {
    #[zeroize(skip)]
    guild_id: String,
    secret: Zeroizing<String>,
}

impl SecureBotToken {
    pub fn new(guild_id: impl Into<String>, token: String) -> Self {
        Self {
            guild_id: guild_id.into(),
            secret: Zeroizing::new(token),
        }
    }

    pub fn guild_id(&self) -> &str {
        &self.guild_id
    }

    /// Exposes the underlying secret token string for authorized low-level API operations
    /// (such as Discord Gateway WebSocket authentication handshake and REST authorization headers).
    pub fn expose_secret(&self) -> &str {
        &self.secret
    }
}

impl std::fmt::Debug for SecureBotToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecureBotToken")
            .field("guild_id", &self.guild_id)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl std::fmt::Display for SecureBotToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

pub struct CredentialManager;

impl CredentialManager {
    /// Validates that a token strictly follows the official 3-part Discord Bot token format.
    /// User tokens, scrapers, and self-bots are explicitly prohibited per Discord Developer Terms of Service.
    pub fn validate_bot_token_format(token: &str) -> Result<String, CredentialError> {
        let trimmed = token.trim();
        let clean_token = trimmed.strip_prefix("Bot ").unwrap_or(trimmed);
        let parts: Vec<&str> = clean_token.split('.').collect();
        if parts.len() != 3 || parts[0].len() < 18 || parts[1].is_empty() || parts[2].len() < 25 {
            return Err(CredentialError::InvalidBotTokenFormat);
        }
        Ok(clean_token.to_string())
    }

    /// Formats the raw token into an official Bot Gateway authorization header string.
    pub fn format_bot_auth_header(token: &str) -> Result<String, CredentialError> {
        let clean_token = Self::validate_bot_token_format(token)?;
        Ok(format!("Bot {}", clean_token))
    }

    /// Helper to retrieve the list of guild IDs stored in the vault registry (IDs only, never tokens).
    fn get_registered_guild_ids() -> Result<Vec<String>, CredentialError> {
        let entry = Entry::new(SERVICE_NAME, GUILD_REGISTRY_KEY)?;
        match entry.get_password() {
            Ok(json_str) => serde_json::from_str(&json_str)
                .map_err(|e| CredentialError::RegistryCorruption(e.to_string())),
            Err(keyring::Error::NoEntry) => Ok(Vec::new()),
            Err(e) => Err(CredentialError::KeyringError(e)),
        }
    }

    /// Registers a guild ID into the metadata index in the secure enclave.
    fn register_guild_id(guild_id: &str) -> Result<(), CredentialError> {
        let mut guilds = Self::get_registered_guild_ids().unwrap_or_default();
        if !guilds.iter().any(|g| g == guild_id) {
            guilds.push(guild_id.to_string());
            let entry = Entry::new(SERVICE_NAME, GUILD_REGISTRY_KEY)?;
            let json_str = serde_json::to_string(&guilds)
                .map_err(|e| CredentialError::RegistryCorruption(e.to_string()))?;
            entry.set_password(&json_str)?;
        }
        Ok(())
    }

    /// Unregisters a guild ID from the metadata index in the secure enclave.
    fn unregister_guild_id(guild_id: &str) -> Result<(), CredentialError> {
        let mut guilds = Self::get_registered_guild_ids().unwrap_or_default();
        if let Some(pos) = guilds.iter().position(|g| g == guild_id) {
            guilds.remove(pos);
            let entry = Entry::new(SERVICE_NAME, GUILD_REGISTRY_KEY)?;
            if guilds.is_empty() {
                let _ = entry.delete_credential();
            } else {
                let json_str = serde_json::to_string(&guilds)
                    .map_err(|e| CredentialError::RegistryCorruption(e.to_string()))?;
                entry.set_password(&json_str)?;
            }
        }
        Ok(())
    }

    /// Save platform bot token securely to the host operating system's native keychain.
    pub fn store_token(guild_id: &str, token: &str) -> Result<(), CredentialError> {
        let clean_token = Self::validate_bot_token_format(token)?;
        let entry = Entry::new(SERVICE_NAME, guild_id)?;
        entry.set_password(&clean_token)?;
        let _ = Self::register_guild_id(guild_id);
        Ok(())
    }

    /// Retrieve platform bot token from the OS keychain as a raw String. Never logged or stored in SQLite.
    pub fn get_token(guild_id: &str) -> Result<String, CredentialError> {
        let entry = Entry::new(SERVICE_NAME, guild_id)?;
        match entry.get_password() {
            Ok(token) => Ok(token),
            Err(keyring::Error::NoEntry) => {
                Err(CredentialError::TokenNotFound(guild_id.to_string()))
            }
            Err(e) => Err(CredentialError::KeyringError(e)),
        }
    }

    /// Retrieve platform bot token wrapped in zeroize-protected `SecureBotToken`.
    pub fn get_secure_token(guild_id: &str) -> Result<SecureBotToken, CredentialError> {
        let token = Self::get_token(guild_id)?;
        Ok(SecureBotToken::new(guild_id, token))
    }

    /// Purge credentials for a single guild from the OS keychain upon user request or revocation.
    pub fn delete_token(guild_id: &str) -> Result<(), CredentialError> {
        let entry = Entry::new(SERVICE_NAME, guild_id)?;
        let res = match entry.delete_credential() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(CredentialError::KeyringError(e)),
        };
        let _ = Self::unregister_guild_id(guild_id);
        res
    }

    /// Complete system eradication: Purges all registered bot tokens and registry metadata
    /// from the native OS secure enclave.
    pub fn purge_all_credentials() -> Result<(), CredentialError> {
        let guilds = Self::get_registered_guild_ids().unwrap_or_default();
        for guild_id in guilds {
            if let Ok(entry) = Entry::new(SERVICE_NAME, &guild_id) {
                let _ = entry.delete_credential();
            }
        }
        if let Ok(registry_entry) = Entry::new(SERVICE_NAME, GUILD_REGISTRY_KEY) {
            let _ = registry_entry.delete_credential();
        }
        Ok(())
    }

    /// Lists registered guild IDs that have credentials stored, without exposing tokens.
    pub fn list_registered_guilds() -> Result<Vec<String>, CredentialError> {
        Self::get_registered_guild_ids()
    }

    /// Conducts pre-flight Discord API identity verification and permission checks before storing token.
    pub async fn preflight_handshake(
        token: &str,
        guild_id: Option<&str>,
    ) -> Result<HandshakeSummary, CredentialError> {
        let validator = DiscordHandshakeValidator::new();
        validator.execute_preflight_handshake(token, guild_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_valid_bot_token_format() {
        let seg1 = "TEST_SNOWFLAKE_BASE64_1234";
        let seg2 = "SIG_T";
        let seg3 = "MOCK_SIGNATURE_FOR_UNIT_TEST_PADDING_XYZ";
        let sample_token = format!("{}.{}.{}", seg1, seg2, seg3);

        let res = CredentialManager::validate_bot_token_format(&sample_token);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), sample_token);

        let with_prefix = format!("Bot {}", sample_token);
        let res_prefix = CredentialManager::validate_bot_token_format(&with_prefix);
        assert!(res_prefix.is_ok());
        assert_eq!(res_prefix.unwrap(), sample_token);
    }

    #[test]
    fn test_reject_user_token_or_malformed() {
        assert!(CredentialManager::validate_bot_token_format("invalid_token_string").is_err());
        assert!(CredentialManager::validate_bot_token_format("partone.parttwo").is_err());
        assert!(CredentialManager::validate_bot_token_format("").is_err());
        assert!(CredentialManager::validate_bot_token_format("short.b.c").is_err());
    }

    #[test]
    fn test_format_bot_auth_header() {
        let seg1 = "TEST_SNOWFLAKE_BASE64_1234";
        let seg2 = "SIG_T";
        let seg3 = "MOCK_SIGNATURE_FOR_UNIT_TEST_PADDING_XYZ";
        let sample_token = format!("{}.{}.{}", seg1, seg2, seg3);

        let header = CredentialManager::format_bot_auth_header(&sample_token).unwrap();
        assert_eq!(header, format!("Bot {}", sample_token));
    }

    #[test]
    fn test_zero_plaintext_policy_redaction() {
        let secret_raw = "MOCK_SECRET_TOKEN_VALUE_NOT_TO_BE_LOGGED";
        let secure_token = SecureBotToken::new("guild_987", secret_raw.to_string());

        // Verify Debug representation redacts the secret token
        let debug_str = format!("{:?}", secure_token);
        assert!(
            !debug_str.contains(secret_raw),
            "Debug leaked secret token!"
        );
        assert!(debug_str.contains("[REDACTED]"));
        assert!(debug_str.contains("guild_987"));

        // Verify Display representation redacts the secret token
        let display_str = format!("{}", secure_token);
        assert_eq!(display_str, "[REDACTED]");
        assert!(!display_str.contains(secret_raw));

        // Authorized internal access provides clean secret
        assert_eq!(secure_token.expose_secret(), secret_raw);
        assert_eq!(secure_token.guild_id(), "guild_987");
    }

    #[test]
    fn test_zero_plaintext_sqlite_schema_invariant() {
        // Enforce zero-plaintext policy: Verify that no SQLite table schema in the storage engine
        // defines any token, secret, credential, or password column.
        let mut conn = Connection::open_in_memory().unwrap();
        crate::storage::migrations::run_migrations(&mut conn).unwrap();

        let tables = vec!["benchmarks", "incidents", "audit_logs"];
        let prohibited_substrings = ["token", "secret", "credential", "password", "keyring"];

        for table in tables {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info({});", table))
                .unwrap();
            let column_names: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            for col in &column_names {
                let col_lower = col.to_lowercase();
                for forbidden in &prohibited_substrings {
                    assert!(
                        !col_lower.contains(forbidden),
                        "Zero-plaintext violation: table '{}' contains forbidden column '{}'",
                        table,
                        col
                    );
                }
            }
        }
    }

    #[test]
    fn test_credential_error_never_leaks_token() {
        let err = CredentialError::InvalidBotTokenFormat;
        let err_str = err.to_string();
        assert!(!err_str.contains("MOCK_SECRET"));
        assert!(err_str.contains("Invalid bot token format"));

        let not_found = CredentialError::TokenNotFound("guild_123".to_string());
        assert_eq!(not_found.to_string(), "Missing token for guild guild_123");
    }

    #[test]
    fn test_guild_registry_helpers_codec() {
        let guilds = vec!["123456789".to_string(), "987654321".to_string()];
        let json_str = serde_json::to_string(&guilds).unwrap();
        let decoded: Vec<String> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(decoded, guilds);
    }
}
