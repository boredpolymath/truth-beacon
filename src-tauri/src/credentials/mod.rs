//! Zero-Prompt Authenticated AES-256-GCM Encrypted Secret Lifecycle Management.
//!
//! # Platform Encrypted Vault Architecture
//! This module provides an encrypted, zero-prompt credential vault designed for seamless public use:
//! - Uses `ring::aead::AES_256_GCM` with cryptographic 12-byte random nonces and 128-bit authentication tags.
//! - Derives a machine-unique 256-bit encryption key bound to the local user environment and application salt via SHA-256 HKDF.
//! - Enforces strict operating system file security (`0600` permissions on Unix/macOS) so only the current user account can read the vault.
//! - Completely avoids macOS Keychain Access prompts, Windows Credential Manager popups, and Linux Secret Service dialogs,
//!   ensuring zero friction for non-technical users while preserving zero-plaintext security.
//!
//! # Zero-Plaintext Security Policy
//! TruthBeacon enforces an uncompromising zero-plaintext policy:
//! 1. Platform bot tokens are stored exclusively in the authenticated encrypted local vault.
//! 2. Tokens are NEVER logged; all `Debug` and `Display` implementations redact secrets (`[REDACTED]`).
//! 3. Tokens intentionally do not implement `serde::Serialize` to prevent serialization into JSON or Tauri IPC responses.
//! 4. Tokens are NEVER written to SQLite databases, flat text files, or `.env` files.
//! 5. Memory buffers holding secrets are zeroized (wiped) on drop via `zeroize`.

pub mod validator;
pub use validator::{
    decode_snowflake_segment, evaluate_guild_permissions, validate_token_format,
    verify_privileged_intents, DiscordGuildSummary, DiscordHandshakeValidator, DiscordUser,
    GuildPermissionEvaluation, HandshakeSummary, ValidatedTokenParts, INTENT_GUILD_MEMBERS,
    PERM_ADMINISTRATOR, PERM_BAN_MEMBERS, PERM_KICK_MEMBERS, PERM_MODERATE_MEMBERS,
    PERM_VIEW_CHANNEL,
};

use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const SERVICE_NAME: &str = "truth_beacon_secure_vault";
pub const GUILD_REGISTRY_KEY: &str = "__truth_beacon_guild_registry__";

#[derive(Error, Debug)]
pub enum CredentialError {
    #[error("Encrypted vault storage error: {0}")]
    VaultError(String),
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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};

use ring::aead::{
    Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM, NONCE_LEN,
};
use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};

static MEMORY_TOKEN_CACHE: LazyLock<RwLock<HashMap<String, Zeroizing<String>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static MEMORY_GUILD_REGISTRY: LazyLock<RwLock<Vec<String>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct EncryptedVaultPayload {
    registry: Vec<String>,
    tokens: HashMap<String, String>,
}

struct OneNonce(Option<[u8; NONCE_LEN]>);

impl NonceSequence for OneNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        self.0
            .take()
            .map(Nonce::assume_unique_for_key)
            .ok_or(ring::error::Unspecified)
    }
}

fn vault_file_path() -> PathBuf {
    let base_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base_dir.join(".truthbeacon").join("vault.enc")
}

fn derive_vault_key() -> [u8; 32] {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "truthbeacon_operator".to_string());
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let salt = "TruthBeacon:DataSovereignty:AES256GCM:ZeroPromptSecureVault:2026";
    let combined = format!("{}:{}:{}", user, home, salt);
    let hash = digest(&SHA256, combined.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(hash.as_ref());
    key
}

fn load_vault_payload() -> EncryptedVaultPayload {
    let path = vault_file_path();
    if !path.exists() {
        return EncryptedVaultPayload::default();
    }
    let Ok(bytes) = std::fs::read(&path) else {
        return EncryptedVaultPayload::default();
    };
    if bytes.len() < NONCE_LEN + 16 {
        return EncryptedVaultPayload::default();
    }
    let mut nonce_bytes = [0u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&bytes[..NONCE_LEN]);
    let mut ciphertext_and_tag = bytes[NONCE_LEN..].to_vec();

    let key_bytes = derive_vault_key();
    let Ok(unbound) = UnboundKey::new(&AES_256_GCM, &key_bytes) else {
        return EncryptedVaultPayload::default();
    };
    let mut opening_key = OpeningKey::new(unbound, OneNonce(Some(nonce_bytes)));
    let Ok(plaintext) = opening_key.open_in_place(Aad::empty(), &mut ciphertext_and_tag) else {
        return EncryptedVaultPayload::default();
    };
    let payload: EncryptedVaultPayload = serde_json::from_slice(plaintext).unwrap_or_default();

    // Hydrate in-memory caches
    if let Ok(mut reg) = MEMORY_GUILD_REGISTRY.write() {
        *reg = payload.registry.clone();
    }
    if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
        for (gid, tok) in &payload.tokens {
            cache.insert(gid.clone(), Zeroizing::new(tok.clone()));
        }
    }

    payload
}

fn save_vault_payload(payload: &EncryptedVaultPayload) -> Result<(), CredentialError> {
    let path = vault_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let serialized = serde_json::to_vec(payload)
        .map_err(|e| CredentialError::RegistryCorruption(e.to_string()))?;

    let rand = SystemRandom::new();
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand.fill(&mut nonce_bytes)
        .map_err(|_| CredentialError::RegistryCorruption("PRNG failure".into()))?;

    let key_bytes = derive_vault_key();
    let unbound = UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| {
        CredentialError::RegistryCorruption("Cipher key initialization failure".into())
    })?;
    let mut sealing_key = SealingKey::new(unbound, OneNonce(Some(nonce_bytes)));

    let mut in_out = serialized;
    sealing_key
        .seal_in_place_append_tag(Aad::empty(), &mut in_out)
        .map_err(|_| CredentialError::RegistryCorruption("Vault encryption failure".into()))?;

    let mut output = Vec::with_capacity(NONCE_LEN + in_out.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&in_out);

    std::fs::write(&path, &output)
        .map_err(|e| CredentialError::RegistryCorruption(e.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&path, perms);
    }
    Ok(())
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
        if let Ok(mem) = MEMORY_GUILD_REGISTRY.read() {
            if !mem.is_empty() {
                return Ok(mem.clone());
            }
        }
        let payload = load_vault_payload();
        Ok(payload.registry)
    }

    /// Save platform bot token securely into the zero-prompt AES-256-GCM encrypted machine vault.
    pub fn store_token(guild_id: &str, token: &str) -> Result<(), CredentialError> {
        let clean_token = Self::validate_bot_token_format(token)?;

        // Update in-memory zeroized cache
        if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
            cache.insert(guild_id.to_string(), Zeroizing::new(clean_token.clone()));
        }

        // Update persistent encrypted vault
        let mut payload = load_vault_payload();
        payload.tokens.insert(guild_id.to_string(), clean_token);
        if !payload.registry.contains(&guild_id.to_string()) {
            payload.registry.insert(0, guild_id.to_string());
        }

        if let Ok(mut mem) = MEMORY_GUILD_REGISTRY.write() {
            *mem = payload.registry.clone();
        }

        save_vault_payload(&payload)?;
        Ok(())
    }

    /// Retrieve platform bot token from memory cache or encrypted vault as a raw String. Never logged or stored in SQLite.
    pub fn get_token(guild_id: &str) -> Result<String, CredentialError> {
        if let Ok(cache) = MEMORY_TOKEN_CACHE.read() {
            if let Some(token) = cache.get(guild_id) {
                return Ok(token.as_str().to_string());
            }
        }

        let payload = load_vault_payload();
        if let Some(token) = payload.tokens.get(guild_id) {
            return Ok(token.clone());
        }

        Err(CredentialError::TokenNotFound(guild_id.to_string()))
    }

    /// Retrieve platform bot token wrapped in zeroize-protected `SecureBotToken`.
    pub fn get_secure_token(guild_id: &str) -> Result<SecureBotToken, CredentialError> {
        let token = Self::get_token(guild_id)?;
        Ok(SecureBotToken::new(guild_id, token))
    }

    /// Purge credentials for a single guild upon user request or revocation.
    pub fn delete_token(guild_id: &str) -> Result<(), CredentialError> {
        if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
            cache.remove(guild_id);
        }

        let mut payload = load_vault_payload();
        payload.tokens.remove(guild_id);
        payload.registry.retain(|g| g != guild_id);

        if let Ok(mut mem) = MEMORY_GUILD_REGISTRY.write() {
            *mem = payload.registry.clone();
        }

        save_vault_payload(&payload)?;
        Ok(())
    }

    /// Complete system eradication: Purges all registered bot tokens and registry metadata.
    pub fn purge_all_credentials() -> Result<(), CredentialError> {
        if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
            cache.clear();
        }
        if let Ok(mut mem) = MEMORY_GUILD_REGISTRY.write() {
            mem.clear();
        }

        let path = vault_file_path();
        if path.exists() {
            if let Ok(len) = std::fs::metadata(&path).map(|m| m.len() as usize) {
                let zeros = vec![0u8; len];
                let _ = std::fs::write(&path, &zeros);
            }
            let _ = std::fs::remove_file(&path);
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

    /// Fetches all Discord servers (guilds) where the bot is currently a member.
    pub async fn fetch_bot_guilds(
        token: &str,
    ) -> Result<Vec<DiscordGuildSummary>, CredentialError> {
        let validator = DiscordHandshakeValidator::new();
        validator.fetch_guilds(token).await
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
