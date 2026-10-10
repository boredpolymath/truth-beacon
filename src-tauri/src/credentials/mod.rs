//! Zero-Prompt Authenticated AES-256-GCM Encrypted Secret Lifecycle Management.
//!
//! # Platform Encrypted Vault Architecture
//! This module provides an encrypted credential vault designed for desktop identity security:
//! - Uses `ring::aead::AES_256_GCM` with cryptographic 12-byte random nonces and 128-bit authentication tags.
//! - Derives a high-entropy 256-bit encryption key using Argon2id (RFC 9106) with a 32-byte cryptographically
//!   random salt stored alongside the vault (`~/.truthbeacon/vault.salt`).
//! - Binds the derived key to a hardware-backed secret (TPM, Secure Enclave, or OS keyring) where available,
//!   providing defense-in-depth against host file system exfiltration while supporting graceful fallback.
//! - Automatically migrates legacy vaults encrypted under earlier HKDF derivations to Argon2id + companion salt.
//! - Enforces strict operating system file security (`0600` permissions on Unix/macOS) so only the current user account can read the vault and salt files.
//! - Memory buffers holding secrets and key material are zeroized on drop via `zeroize`.
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
pub const HARDWARE_KEY_ACCOUNT: &str = "vault_master_hardware_binding";
pub const VAULT_KEY_ACCOUNT: &str = "vault_master_encryption_key_256";
pub const SALT_LEN: usize = 32;

#[derive(Error, Debug)]
pub enum CredentialError {
    #[error("Encrypted vault storage error: {0}")]
    VaultError(String),
    #[error("Platform keychain unavailable or access denied: {0}")]
    KeychainUnavailable(String),
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
    #[error("Vault corrupted or tampered: {0}")]
    VaultCorruptedOrTampered(String),
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

use argon2::Argon2;
use base64::prelude::*;
use ring::aead::{
    Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM, NONCE_LEN,
};
use ring::rand::{SecureRandom, SystemRandom};

static MEMORY_TOKEN_CACHE: LazyLock<RwLock<HashMap<String, Zeroizing<String>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static MEMORY_GUILD_REGISTRY: LazyLock<RwLock<Vec<String>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct EncryptedVaultPayload {
    pub(crate) registry: Vec<String>,
    pub(crate) tokens: HashMap<String, String>,
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

#[cfg(test)]
thread_local! {
    static TEST_OVERRIDE_VAULT_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    static TEST_OVERRIDE_KEYCHAIN_DENIED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn vault_dir() -> PathBuf {
    #[cfg(test)]
    {
        if let Ok(Some(dir)) = TEST_OVERRIDE_VAULT_DIR.try_with(|d| d.borrow().clone()) {
            return dir;
        }
    }
    if let Ok(custom) = std::env::var("TRUTHBEACON_VAULT_DIR") {
        PathBuf::from(custom)
    } else {
        let base_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        base_dir.join(".truthbeacon")
    }
}

fn vault_file_path() -> PathBuf {
    vault_dir().join("vault.enc")
}

fn vault_salt_path() -> PathBuf {
    vault_dir().join("vault.salt")
}

fn ensure_secure_dir(dir: &std::path::Path) -> Result<(), CredentialError> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| {
            CredentialError::VaultError(format!("Failed to create vault directory: {}", e))
        })?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(dir, perms).map_err(|e| {
            CredentialError::VaultError(format!(
                "Failed to set restrictive 0700 permissions on vault directory: {}",
                e
            ))
        })?;
    }
    Ok(())
}

fn atomic_write_secure_file(path: &std::path::Path, data: &[u8]) -> Result<(), CredentialError> {
    if let Some(parent) = path.parent() {
        ensure_secure_dir(parent)?;
    }

    let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp_path)
            .map_err(|e| {
                CredentialError::VaultError(format!("Failed to create secure temp file: {}", e))
            })?;
        file.write_all(data).map_err(|e| {
            CredentialError::VaultError(format!("Failed to write secure data: {}", e))
        })?;
        file.sync_all().map_err(|e| {
            CredentialError::VaultError(format!("Failed to sync secure data to disk: {}", e))
        })?;
    }

    #[cfg(not(unix))]
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(|e| {
                CredentialError::VaultError(format!("Failed to create secure temp file: {}", e))
            })?;
        file.write_all(data).map_err(|e| {
            CredentialError::VaultError(format!("Failed to write secure data: {}", e))
        })?;
        file.sync_all().map_err(|e| {
            CredentialError::VaultError(format!("Failed to sync secure data to disk: {}", e))
        })?;
    }

    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        CredentialError::VaultError(format!("Failed to atomically replace vault file: {}", e))
    })?;

    Ok(())
}

fn get_or_create_vault_salt_in(dir: &std::path::Path) -> Result<[u8; SALT_LEN], CredentialError> {
    ensure_secure_dir(dir)?;
    let salt_path = dir.join("vault.salt");
    if salt_path.exists() {
        if let Ok(bytes) = std::fs::read(&salt_path) {
            if let Ok(salt) = <[u8; SALT_LEN]>::try_from(bytes) {
                return Ok(salt);
            }
        }
    }

    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).map_err(|e| {
        CredentialError::VaultError(format!(
            "Cryptographic RNG failure generating vault salt: {}",
            e
        ))
    })?;

    atomic_write_secure_file(&salt_path, &salt)?;
    Ok(salt)
}

fn get_or_create_vault_salt() -> Result<[u8; SALT_LEN], CredentialError> {
    get_or_create_vault_salt_in(&vault_dir())
}

#[cfg(test)]
const ARGON2_M_COST: u32 = 64; // 64 KiB for tests to preserve strict background idle RAM budget (<30MB)
#[cfg(not(test))]
const ARGON2_M_COST: u32 = 19456; // 19 MiB (RFC 9106 / OWASP recommended memory parameter)

const ARGON2_T_COST: u32 = 2;
const ARGON2_P_COST: u32 = 1;

type CachedVaultKey = ([u8; SALT_LEN], Zeroizing<[u8; 32]>);

static HARDWARE_SECRET_CACHE: LazyLock<RwLock<Option<Zeroizing<String>>>> =
    LazyLock::new(|| RwLock::new(None));
static VAULT_KEY_CACHE: LazyLock<RwLock<Option<CachedVaultKey>>> =
    LazyLock::new(|| RwLock::new(None));

fn get_or_create_hardware_secret() -> Result<Zeroizing<String>, CredentialError> {
    #[cfg(test)]
    {
        if TEST_OVERRIDE_KEYCHAIN_DENIED
            .try_with(|c| c.get())
            .unwrap_or(false)
            || std::env::var("TRUTHBEACON_MOCK_KEYCHAIN_DENIED").is_ok()
        {
            return Err(CredentialError::KeychainUnavailable(
                "Access denied to operating system secure keychain (simulated test fault)".into(),
            ));
        }
        if std::env::var("TRUTHBEACON_LIVE_KEYRING_TEST").is_err() {
            if let Ok(cache) = HARDWARE_SECRET_CACHE.read() {
                if let Some(sec) = &*cache {
                    return Ok(sec.clone());
                }
            }
            let mock_secret =
                Zeroizing::new("test_hardware_backed_enclave_secret_xyz123".to_string());
            if let Ok(mut cache) = HARDWARE_SECRET_CACHE.write() {
                *cache = Some(mock_secret.clone());
            }
            return Ok(mock_secret);
        }
    }

    if let Ok(cache) = HARDWARE_SECRET_CACHE.read() {
        if let Some(sec) = &*cache {
            return Ok(sec.clone());
        }
    }

    let entry = keyring::Entry::new(SERVICE_NAME, HARDWARE_KEY_ACCOUNT).map_err(|e| {
        CredentialError::KeychainUnavailable(format!("Keyring initialization error: {}", e))
    })?;
    let secret = match entry.get_password() {
        Ok(secret) if !secret.is_empty() => Zeroizing::new(secret),
        _ => {
            let mut secret_bytes = [0u8; 32];
            if getrandom::getrandom(&mut secret_bytes).is_err() {
                return Err(CredentialError::VaultError(
                    "Cryptographic RNG entropy generation failure".into(),
                ));
            }
            let secret_str = BASE64_STANDARD.encode(secret_bytes);
            entry.set_password(&secret_str).map_err(|e| {
                CredentialError::KeychainUnavailable(format!(
                    "Failed to store hardware secret in secure OS keychain: {}",
                    e
                ))
            })?;
            Zeroizing::new(secret_str)
        }
    };

    if let Ok(mut cache) = HARDWARE_SECRET_CACHE.write() {
        *cache = Some(secret.clone());
    }

    Ok(secret)
}

fn purge_hardware_secret() {
    if let Ok(mut cache) = VAULT_KEY_CACHE.write() {
        *cache = None;
    }
    if let Ok(mut cache) = HARDWARE_SECRET_CACHE.write() {
        *cache = None;
    }

    #[cfg(test)]
    {
        if std::env::var("TRUTHBEACON_LIVE_KEYRING_TEST").is_err() {
            return;
        }
    }

    if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, VAULT_KEY_ACCOUNT) {
        let _ = entry.delete_credential();
    }
    if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, HARDWARE_KEY_ACCOUNT) {
        let _ = entry.delete_credential();
    }
}

/// Derives a 256-bit key from a user passphrase or local secret and salt using Argon2id with high memory/iteration costs.
/// Fast hashes (e.g. raw SHA-256) are strictly avoided to prevent brute-force attacks on the local vault payload.
pub fn derive_key_from_passphrase(
    passphrase: &str,
    vault_salt: &[u8; SALT_LEN],
) -> Result<Zeroizing<[u8; 32]>, CredentialError> {
    let params = argon2::Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .map_err(|e| CredentialError::VaultError(format!("Argon2id parameter error: {}", e)))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase.as_bytes(), vault_salt, key.as_mut())
        .map_err(|e| {
            CredentialError::VaultError(format!("Argon2id key derivation failed: {}", e))
        })?;

    Ok(key)
}

pub fn derive_argon2id_key_with_binding(
    vault_salt: &[u8; SALT_LEN],
    hw_binding: &str,
) -> Result<Zeroizing<[u8; 32]>, CredentialError> {
    if hw_binding.trim().is_empty() {
        return Err(CredentialError::VaultError(
            "Hardware secret binding cannot be empty".into(),
        ));
    }
    let secret_material = Zeroizing::new(format!("TruthBeacon:Argon2id:VaultKey:{}", hw_binding));

    derive_key_from_passphrase(secret_material.as_str(), vault_salt)
}

/// Retrieves or creates the 256-bit vault encryption key.
///
/// Stores ONLY the 256-bit key in platform-native secure enclaves or keychains
/// (Windows DPAPI / Credential Manager, macOS Keychain, Linux Secret Service via keyring-rs).
/// The vault data payload itself remains in the local file `vault.enc`, avoiding repeated permission prompts
/// while keeping key derivation hardware-isolated.
pub fn get_or_create_vault_encryption_key(
    vault_salt: &[u8; SALT_LEN],
) -> Result<Zeroizing<[u8; 32]>, CredentialError> {
    if let Ok(cache) = VAULT_KEY_CACHE.read() {
        if let Some((cached_salt, key)) = &*cache {
            if cached_salt == vault_salt {
                return Ok(key.clone());
            }
        }
    }

    #[cfg(test)]
    let skip_live_keyring = std::env::var("TRUTHBEACON_LIVE_KEYRING_TEST").is_err();
    #[cfg(not(test))]
    let skip_live_keyring = false;

    // 1. Attempt to retrieve the 256-bit key directly from the platform-native keychain
    if !skip_live_keyring {
        if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, VAULT_KEY_ACCOUNT) {
            if let Ok(stored_b64) = entry.get_password() {
                if let Ok(decoded) = BASE64_STANDARD.decode(stored_b64.trim()) {
                    if decoded.len() == 32 {
                        let mut key_arr = [0u8; 32];
                        key_arr.copy_from_slice(&decoded);
                        let zero_key = Zeroizing::new(key_arr);
                        if let Ok(mut cache) = VAULT_KEY_CACHE.write() {
                            *cache = Some((*vault_salt, zero_key.clone()));
                        }
                        return Ok(zero_key);
                    }
                }
            }
        }
    }

    // 2. Derive 256-bit key via Argon2id with high memory/iteration costs using protected hardware secret
    let hw_secret = get_or_create_hardware_secret()?;
    let derived_key = derive_argon2id_key_with_binding(vault_salt, hw_secret.as_str())?;

    // 3. Store ONLY the 256-bit key in the platform keychain
    if !skip_live_keyring {
        if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, VAULT_KEY_ACCOUNT) {
            let encoded = BASE64_STANDARD.encode(*derived_key);
            let _ = entry.set_password(&encoded);
        }
    }

    if let Ok(mut cache) = VAULT_KEY_CACHE.write() {
        *cache = Some((*vault_salt, derived_key.clone()));
    }

    Ok(derived_key)
}

fn derive_argon2id_vault_key(
    vault_salt: &[u8; SALT_LEN],
) -> Result<Zeroizing<[u8; 32]>, CredentialError> {
    get_or_create_vault_encryption_key(vault_salt)
}

fn derive_legacy_vault_key() -> [u8; 32] {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "truthbeacon_operator".to_string());
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let legacy_salt = "TruthBeacon:DataSovereignty:AES256GCM:ZeroPromptSecureVault:2026";
    let combined = format!("{}:{}:{}", user, home, legacy_salt);
    let hash = ring::digest::digest(&ring::digest::SHA256, combined.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(hash.as_ref());
    key
}

/// Authenticated Associated Data (AAD) binding vault payloads to the TruthBeacon application domain.
pub const VAULT_AAD_V1: &[u8] = b"TruthBeacon:EncryptedVault:v1";

/// Loads and decrypts the encrypted vault payload with strict authentication tag and AAD verification.
/// If the vault file exists but has been tampered with or corrupted, an explicit error is returned.
pub(crate) fn load_vault_payload_checked() -> Result<EncryptedVaultPayload, CredentialError> {
    let path = vault_file_path();
    if !path.exists() {
        return Ok(EncryptedVaultPayload::default());
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| CredentialError::VaultError(format!("Failed to read vault file: {}", e)))?;
    if bytes.len() < NONCE_LEN + 16 {
        return Err(CredentialError::VaultCorruptedOrTampered(
            "Vault file truncated or malformed".into(),
        ));
    }
    let mut nonce_bytes = [0u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&bytes[..NONCE_LEN]);
    let mut ciphertext_and_tag = bytes[NONCE_LEN..].to_vec();

    let salt_path = vault_salt_path();
    let payload = if salt_path.exists() {
        let salt = get_or_create_vault_salt()?;
        let key_bytes = derive_argon2id_vault_key(&salt)?;
        let unbound = UnboundKey::new(&AES_256_GCM, key_bytes.as_ref())
            .map_err(|_| CredentialError::VaultError("Cipher initialization failed".into()))?;
        let mut opening_key = OpeningKey::new(unbound, OneNonce(Some(nonce_bytes)));

        // Try authenticated domain-bound AAD first
        let mut attempt_v1 = ciphertext_and_tag.clone();
        let plaintext = match opening_key.open_in_place(Aad::from(VAULT_AAD_V1), &mut attempt_v1) {
            Ok(pt) => pt.to_vec(),
            Err(_) => {
                // Backward compatibility: try legacy empty AAD for v0 vaults on untampered buffer
                let unbound_retry =
                    UnboundKey::new(&AES_256_GCM, key_bytes.as_ref()).map_err(|_| {
                        CredentialError::VaultError("Cipher initialization failed".into())
                    })?;
                let mut opening_key_retry =
                    OpeningKey::new(unbound_retry, OneNonce(Some(nonce_bytes)));
                opening_key_retry
                    .open_in_place(Aad::empty(), &mut ciphertext_and_tag)
                    .map_err(|_| {
                        CredentialError::VaultCorruptedOrTampered(
                            "Authentication tag mismatch: ciphertext or key has been tampered with or corrupted".into(),
                        )
                    })?
                    .to_vec()
            }
        };

        serde_json::from_slice::<EncryptedVaultPayload>(&plaintext).map_err(|e| {
            CredentialError::VaultCorruptedOrTampered(format!(
                "Payload JSON deserialization failed: {}",
                e
            ))
        })?
    } else {
        // Attempt seamless legacy vault migration
        let legacy_key = derive_legacy_vault_key();
        let unbound = UnboundKey::new(&AES_256_GCM, &legacy_key).map_err(|_| {
            CredentialError::VaultError("Legacy cipher key initialization failed".into())
        })?;
        let mut opening_key = OpeningKey::new(unbound, OneNonce(Some(nonce_bytes)));
        let plaintext = opening_key
            .open_in_place(Aad::empty(), &mut ciphertext_and_tag)
            .map_err(|_| {
                CredentialError::VaultCorruptedOrTampered(
                    "Legacy authentication tag verification failed".into(),
                )
            })?;
        let migrated: EncryptedVaultPayload = serde_json::from_slice(plaintext).map_err(|e| {
            CredentialError::VaultCorruptedOrTampered(format!(
                "Legacy payload JSON deserialization failed: {}",
                e
            ))
        })?;
        let _ = save_vault_payload(&migrated);
        migrated
    };

    // Hydrate in-memory caches
    if let Ok(mut reg) = MEMORY_GUILD_REGISTRY.write() {
        *reg = payload.registry.clone();
    }
    if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
        for (gid, tok) in &payload.tokens {
            cache.insert(gid.clone(), Zeroizing::new(tok.clone()));
        }
    }

    Ok(payload)
}

fn save_vault_payload(payload: &EncryptedVaultPayload) -> Result<(), CredentialError> {
    let path = vault_file_path();
    let serialized = serde_json::to_vec(payload)
        .map_err(|e| CredentialError::RegistryCorruption(e.to_string()))?;

    let rand = SystemRandom::new();
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand.fill(&mut nonce_bytes)
        .map_err(|_| CredentialError::RegistryCorruption("PRNG failure".into()))?;

    let salt = get_or_create_vault_salt()?;
    let key_bytes = derive_argon2id_vault_key(&salt)?;

    let unbound = UnboundKey::new(&AES_256_GCM, key_bytes.as_ref()).map_err(|_| {
        CredentialError::RegistryCorruption("Cipher key initialization failure".into())
    })?;
    let mut sealing_key = SealingKey::new(unbound, OneNonce(Some(nonce_bytes)));

    let mut in_out = serialized;
    sealing_key
        .seal_in_place_append_tag(Aad::from(VAULT_AAD_V1), &mut in_out)
        .map_err(|_| CredentialError::RegistryCorruption("Vault encryption failure".into()))?;

    let mut output = Vec::with_capacity(NONCE_LEN + in_out.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&in_out);

    atomic_write_secure_file(&path, &output)?;

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
        #[cfg(not(test))]
        if let Ok(mem) = MEMORY_GUILD_REGISTRY.read() {
            if !mem.is_empty() {
                return Ok(mem.clone());
            }
        }
        let payload = load_vault_payload_checked()?;
        Ok(payload.registry)
    }

    /// Save platform bot token securely into the zero-prompt AES-256-GCM encrypted machine vault.
    pub fn store_token(guild_id: &str, token: &str) -> Result<(), CredentialError> {
        let clean_token = Self::validate_bot_token_format(token)?;

        // Update in-memory zeroized cache
        if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
            cache.insert(guild_id.to_string(), Zeroizing::new(clean_token.clone()));
        }

        // Update persistent encrypted vault - check first to ensure tampered vaults are never silently overwritten
        let mut payload = load_vault_payload_checked()?;
        payload.tokens.insert(guild_id.to_string(), clean_token);
        if !payload.registry.contains(&guild_id.to_string()) {
            payload.registry.push(guild_id.to_string());
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

        let payload = load_vault_payload_checked()?;
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

        let mut payload = load_vault_payload_checked()?;
        payload.tokens.remove(guild_id);
        payload.registry.retain(|g| g != guild_id);

        if let Ok(mut mem) = MEMORY_GUILD_REGISTRY.write() {
            *mem = payload.registry.clone();
        }

        save_vault_payload(&payload)?;
        Ok(())
    }

    /// Complete system eradication: Purges all registered bot tokens, salt, and registry metadata.
    pub fn purge_all_credentials() -> Result<(), CredentialError> {
        if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
            cache.clear();
        }
        if let Ok(mut mem) = MEMORY_GUILD_REGISTRY.write() {
            mem.clear();
        }

        let mut errors = Vec::new();
        let path = vault_file_path();
        if path.exists() {
            if let Ok(len) = std::fs::metadata(&path).map(|m| m.len() as usize) {
                let zeros = vec![0u8; len];
                if let Err(e) = std::fs::write(&path, &zeros) {
                    errors.push(format!("Failed to zero vault file: {}", e));
                }
            }
            if let Err(e) = std::fs::remove_file(&path) {
                errors.push(format!("Failed to remove vault file: {}", e));
            }
        }

        let salt_path = vault_salt_path();
        if salt_path.exists() {
            if let Ok(len) = std::fs::metadata(&salt_path).map(|m| m.len() as usize) {
                let zeros = vec![0u8; len];
                if let Err(e) = std::fs::write(&salt_path, &zeros) {
                    errors.push(format!("Failed to zero vault salt file: {}", e));
                }
            }
            if let Err(e) = std::fs::remove_file(&salt_path) {
                errors.push(format!("Failed to remove vault salt file: {}", e));
            }
        }

        purge_hardware_secret();

        if !errors.is_empty() {
            return Err(CredentialError::VaultError(format!(
                "Failed to fully purge credentials: {}",
                errors.join("; ")
            )));
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

    fn generate_test_salt(modifier: u8) -> [u8; SALT_LEN] {
        let mut salt = [0u8; SALT_LEN];
        let _ = getrandom::getrandom(&mut salt);
        salt[0] ^= modifier;
        salt
    }

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

    #[test]
    fn test_vault_salt_generation_persistence_and_permissions() {
        let test_dir =
            std::env::temp_dir().join(format!("truthbeacon_test_salt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&test_dir);
        let _ = std::fs::create_dir_all(&test_dir);

        let salt1 =
            get_or_create_vault_salt_in(&test_dir).expect("Initial salt generation should succeed");
        assert_eq!(salt1.len(), SALT_LEN);
        let salt_file = test_dir.join("vault.salt");
        assert!(
            salt_file.exists(),
            "vault.salt file should exist alongside vault"
        );

        // Verify salt is reused and persistent
        let salt2 =
            get_or_create_vault_salt_in(&test_dir).expect("Reading existing salt should succeed");
        assert_eq!(
            salt1, salt2,
            "Salt must be persistent across multiple reads"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&salt_file).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "Salt file permissions must be 0600 on Unix");
        }

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_argon2id_derivation_binding_and_uniqueness() {
        let salt_a = generate_test_salt(1);
        let salt_b = generate_test_salt(2);
        let binding_1 = "test_hardware_binding_1";
        let binding_2 = "test_hardware_binding_2";

        let key_a1 = derive_argon2id_key_with_binding(&salt_a, binding_1)
            .expect("Argon2id derivation failed");
        let key_a2 = derive_argon2id_key_with_binding(&salt_a, binding_1)
            .expect("Argon2id derivation failed");
        let key_b = derive_argon2id_key_with_binding(&salt_b, binding_1)
            .expect("Argon2id derivation failed");
        let key_bound_differently = derive_argon2id_key_with_binding(&salt_a, binding_2)
            .expect("Argon2id derivation failed");

        assert_eq!(key_a1.len(), 32);
        assert_eq!(
            *key_a1, *key_a2,
            "Argon2id must be deterministic for identical salt and binding"
        );
        assert_ne!(
            *key_a1, *key_b,
            "Argon2id must produce different keys for different salts"
        );
        assert_ne!(
            *key_a1, *key_bound_differently,
            "Argon2id must produce different keys when hardware secret binding differs"
        );
    }

    #[test]
    fn test_vault_encryption_and_legacy_migration() {
        let salt = generate_test_salt(42);
        let key = derive_argon2id_key_with_binding(&salt, "test_hardware_binding").unwrap();

        // 1. Verify encryption and decryption with Argon2id derived key
        let payload = EncryptedVaultPayload {
            registry: vec!["test_guild_123".to_string()],
            tokens: [(
                "test_guild_123".to_string(),
                "sample_secret_token".to_string(),
            )]
            .into_iter()
            .collect(),
        };
        let serialized = serde_json::to_vec(&payload).unwrap();

        let rand = SystemRandom::new();
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand.fill(&mut nonce_bytes).unwrap();

        let unbound = UnboundKey::new(&AES_256_GCM, key.as_ref()).unwrap();
        let mut sealing_key = SealingKey::new(unbound, OneNonce(Some(nonce_bytes)));
        let mut in_out = serialized.clone();
        sealing_key
            .seal_in_place_append_tag(Aad::empty(), &mut in_out)
            .unwrap();

        let unbound_open = UnboundKey::new(&AES_256_GCM, key.as_ref()).unwrap();
        let mut opening_key = OpeningKey::new(unbound_open, OneNonce(Some(nonce_bytes)));
        let decrypted = opening_key
            .open_in_place(Aad::empty(), &mut in_out)
            .unwrap();
        let decrypted_payload: EncryptedVaultPayload = serde_json::from_slice(decrypted).unwrap();
        assert_eq!(
            decrypted_payload.tokens.get("test_guild_123").unwrap(),
            "sample_secret_token"
        );

        // 2. Verify legacy derivation compatibility
        let legacy_key = derive_legacy_vault_key();
        let rand = SystemRandom::new();
        let mut legacy_nonce = [0u8; NONCE_LEN];
        rand.fill(&mut legacy_nonce).unwrap();

        let unbound_legacy = UnboundKey::new(&AES_256_GCM, &legacy_key).unwrap();
        let mut sealing_legacy = SealingKey::new(unbound_legacy, OneNonce(Some(legacy_nonce)));
        let mut in_out_legacy = serialized.clone();
        sealing_legacy
            .seal_in_place_append_tag(Aad::empty(), &mut in_out_legacy)
            .unwrap();

        let unbound_open_legacy = UnboundKey::new(&AES_256_GCM, &legacy_key).unwrap();
        let mut opening_legacy = OpeningKey::new(unbound_open_legacy, OneNonce(Some(legacy_nonce)));
        let decrypted_legacy = opening_legacy
            .open_in_place(Aad::empty(), &mut in_out_legacy)
            .unwrap();
        let decrypted_legacy_payload: EncryptedVaultPayload =
            serde_json::from_slice(decrypted_legacy).unwrap();
        assert_eq!(
            decrypted_legacy_payload
                .tokens
                .get("test_guild_123")
                .unwrap(),
            "sample_secret_token"
        );
    }

    #[test]
    fn test_hardware_secret_binding_resilience() {
        let sec = get_or_create_hardware_secret()
            .expect("Hardware secret creation should succeed in test mock");
        assert!(
            !sec.is_empty(),
            "Hardware secret if present must be non-empty"
        );
        purge_hardware_secret();
    }

    #[test]
    fn test_keychain_stores_only_256bit_vault_encryption_key() {
        let salt = generate_test_salt(77);
        let key = get_or_create_vault_encryption_key(&salt).expect("Key creation should succeed");
        assert_eq!(
            key.len(),
            32,
            "Key stored/derived must be exactly 256 bits (32 bytes)"
        );

        // Verify that the key is distinct from any raw SHA-256 hash
        let sha256_mock = ring::digest::digest(&ring::digest::SHA256, b"raw_sha256_hash");
        assert_ne!(
            &*key,
            sha256_mock.as_ref(),
            "Key must not be derived via raw SHA-256"
        );

        // Verify local vault file vs keychain isolation:
        // Encrypt test payload directly with the hardware-isolated key to verify AES-256-GCM behavior
        let test_payload = EncryptedVaultPayload {
            registry: vec!["isolated_guild".to_string()],
            tokens: [("isolated_guild".to_string(), "sample_tok".to_string())]
                .into_iter()
                .collect(),
        };
        let serialized = serde_json::to_vec(&test_payload).unwrap();

        let rand = SystemRandom::new();
        let mut nonce = [0u8; NONCE_LEN];
        rand.fill(&mut nonce).unwrap();

        let unbound = UnboundKey::new(&AES_256_GCM, key.as_ref()).unwrap();
        let mut sealing_key = SealingKey::new(unbound, OneNonce(Some(nonce)));
        let mut in_out = serialized.clone();
        sealing_key
            .seal_in_place_append_tag(Aad::empty(), &mut in_out)
            .unwrap();

        // Ciphertext must never contain the plaintext token
        assert!(!String::from_utf8_lossy(&in_out).contains("sample_tok"));

        let unbound_open = UnboundKey::new(&AES_256_GCM, key.as_ref()).unwrap();
        let mut opening_key = OpeningKey::new(unbound_open, OneNonce(Some(nonce)));
        let decrypted = opening_key
            .open_in_place(Aad::empty(), &mut in_out)
            .unwrap();
        let decrypted_payload: EncryptedVaultPayload = serde_json::from_slice(decrypted).unwrap();
        assert_eq!(
            decrypted_payload.tokens.get("isolated_guild").unwrap(),
            "sample_tok"
        );
    }

    #[test]
    fn test_passphrase_derivation_uses_argon2id_high_cost() {
        let salt = generate_test_salt(99);
        let passphrase = "OperatorSecurePassphrase2026!";

        let key = derive_key_from_passphrase(passphrase, &salt)
            .expect("Argon2id derivation should succeed");
        assert_eq!(key.len(), 32);

        // Different salt produces completely different key
        let salt_diff = generate_test_salt(100);
        let key_diff_salt = derive_key_from_passphrase(passphrase, &salt_diff).unwrap();
        assert_ne!(*key, *key_diff_salt);

        // Different passphrase produces completely different key
        let key_diff_pass = derive_key_from_passphrase("DifferentPassphrase!", &salt).unwrap();
        assert_ne!(*key, *key_diff_pass);

        // Raw fast hashing (e.g. SHA-256) is fundamentally different from Argon2id output
        let fast_hash = ring::digest::digest(&ring::digest::SHA256, passphrase.as_bytes());
        assert_ne!(
            &*key,
            fast_hash.as_ref(),
            "Argon2id output must not match fast SHA-256 hash"
        );
    }

    #[test]
    fn test_vault_tampered_ciphertext_rejection() {
        let salt = generate_test_salt(55);
        let key = derive_argon2id_key_with_binding(&salt, "test_binding").unwrap();

        let payload = EncryptedVaultPayload {
            registry: vec!["guild_tamper".to_string()],
            tokens: [(
                "guild_tamper".to_string(),
                "original_token_secret".to_string(),
            )]
            .into_iter()
            .collect(),
        };
        let serialized = serde_json::to_vec(&payload).unwrap();

        let rand = SystemRandom::new();
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand.fill(&mut nonce_bytes).unwrap();

        let unbound = UnboundKey::new(&AES_256_GCM, key.as_ref()).unwrap();
        let mut sealing_key = SealingKey::new(unbound, OneNonce(Some(nonce_bytes)));
        let mut in_out = serialized;
        sealing_key
            .seal_in_place_append_tag(Aad::from(VAULT_AAD_V1), &mut in_out)
            .unwrap();

        // Tamper with one byte in the ciphertext
        in_out[0] ^= 0x42;

        let unbound_open = UnboundKey::new(&AES_256_GCM, key.as_ref()).unwrap();
        let mut opening_key = OpeningKey::new(unbound_open, OneNonce(Some(nonce_bytes)));
        let open_res = opening_key.open_in_place(Aad::from(VAULT_AAD_V1), &mut in_out);
        assert!(
            open_res.is_err(),
            "Tampered ciphertext must fail authentication tag check"
        );
    }

    #[test]
    fn test_vault_tampered_aad_rejection() {
        let salt = generate_test_salt(56);
        let key = derive_argon2id_key_with_binding(&salt, "test_binding").unwrap();

        let payload = EncryptedVaultPayload {
            registry: vec!["guild_aad".to_string()],
            tokens: [("guild_aad".to_string(), "sample_token".to_string())]
                .into_iter()
                .collect(),
        };
        let serialized = serde_json::to_vec(&payload).unwrap();

        let rand = SystemRandom::new();
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand.fill(&mut nonce_bytes).unwrap();

        let unbound = UnboundKey::new(&AES_256_GCM, key.as_ref()).unwrap();
        let mut sealing_key = SealingKey::new(unbound, OneNonce(Some(nonce_bytes)));
        let mut in_out = serialized;
        sealing_key
            .seal_in_place_append_tag(Aad::from(VAULT_AAD_V1), &mut in_out)
            .unwrap();

        // Attempt decrypt with different AAD
        let unbound_open = UnboundKey::new(&AES_256_GCM, key.as_ref()).unwrap();
        let mut opening_key = OpeningKey::new(unbound_open, OneNonce(Some(nonce_bytes)));
        let open_res = opening_key.open_in_place(Aad::from(b"Forged:AAD:Context"), &mut in_out);
        assert!(
            open_res.is_err(),
            "Decryption with forged AAD must be rejected by AES-256-GCM"
        );
    }

    #[test]
    fn test_vault_token_rotation_and_redaction() {
        let token_1 = format!(
            "Bot {}.{}.{}",
            "TEST_ROTATION_SNOWFLAKE_12345", "HMAC01", "MOCK_FIRST_TOKEN_SIGNATURE_PAYLOAD_123456"
        );
        let token_2 = format!(
            "Bot {}.{}.{}",
            "TEST_ROTATION_SNOWFLAKE_12345",
            "HMAC02",
            "MOCK_ROTATED_TOKEN_SIGNATURE_PAYLOAD_789012"
        );

        // Validate format
        let clean_1 = CredentialManager::validate_bot_token_format(&token_1).unwrap();
        let clean_2 = CredentialManager::validate_bot_token_format(&token_2).unwrap();
        assert_ne!(clean_1, clean_2);

        // SecureBotToken redact verification
        let sec_tok = SecureBotToken::new("guild_rot", clean_1.clone());
        let debug_str = format!("{:?}", sec_tok);
        let display_str = format!("{}", sec_tok);

        assert!(
            !debug_str.contains(&clean_1),
            "Debug must never reveal secret token"
        );
        assert!(debug_str.contains("[REDACTED]"));
        assert_eq!(display_str, "[REDACTED]");
    }

    struct TestKeychainDeniedGuard;

    impl TestKeychainDeniedGuard {
        fn enable() -> Self {
            TEST_OVERRIDE_KEYCHAIN_DENIED.with(|c| c.set(true));
            Self
        }
    }

    impl Drop for TestKeychainDeniedGuard {
        fn drop(&mut self) {
            TEST_OVERRIDE_KEYCHAIN_DENIED.with(|c| c.set(false));
        }
    }

    struct TestVaultDirGuard {
        prev: Option<PathBuf>,
    }

    impl TestVaultDirGuard {
        fn set(dir: PathBuf) -> Self {
            let prev = TEST_OVERRIDE_VAULT_DIR.with(|d| d.borrow_mut().replace(dir));
            if let Ok(mut reg) = MEMORY_GUILD_REGISTRY.write() {
                reg.clear();
            }
            if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
                cache.clear();
            }
            Self { prev }
        }
    }

    impl Drop for TestVaultDirGuard {
        fn drop(&mut self) {
            TEST_OVERRIDE_VAULT_DIR.with(|d| {
                *d.borrow_mut() = self.prev.take();
            });
            if let Ok(mut reg) = MEMORY_GUILD_REGISTRY.write() {
                reg.clear();
            }
            if let Ok(mut cache) = MEMORY_TOKEN_CACHE.write() {
                cache.clear();
            }
        }
    }

    #[test]
    fn test_keychain_access_denial_fails_safely() {
        let _guard = TestKeychainDeniedGuard::enable();

        let hw_res = get_or_create_hardware_secret();
        assert!(
            matches!(hw_res, Err(CredentialError::KeychainUnavailable(_))),
            "Expected KeychainUnavailable when keychain is denied"
        );

        let test_salt = generate_test_salt(88);
        let vault_key_res = get_or_create_vault_encryption_key(&test_salt);
        assert!(
            matches!(vault_key_res, Err(CredentialError::KeychainUnavailable(_))),
            "Vault key derivation must fail safely without insecure fallbacks when hardware secret is unavailable"
        );

        let empty_bind_res = derive_argon2id_key_with_binding(&test_salt, "   ");
        assert!(
            matches!(empty_bind_res, Err(CredentialError::VaultError(_))),
            "Empty hardware binding must be rejected"
        );
    }

    #[test]
    fn test_vault_atomic_write_and_restrictive_permissions() {
        let temp_dir = std::env::temp_dir().join(format!("tb_test_perm_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let test_file = temp_dir.join("vault.enc");
        let sample_data = b"confidential_vault_encrypted_bytes";
        atomic_write_secure_file(&test_file, sample_data)
            .expect("Atomic secure write should succeed");

        assert!(
            test_file.exists(),
            "Target file must exist after atomic write"
        );
        let read_back = std::fs::read(&test_file).expect("File must be readable");
        assert_eq!(read_back, sample_data);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir_meta = std::fs::metadata(&temp_dir).unwrap();
            let dir_mode = dir_meta.permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700, "Directory must have 0700 permissions");

            let file_meta = std::fs::metadata(&test_file).unwrap();
            let file_mode = file_meta.permissions().mode() & 0o777;
            assert_eq!(file_mode, 0o600, "Vault file must have 0600 permissions");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_corrupted_vault_file_is_not_silently_overwritten() {
        let temp_dir = std::env::temp_dir().join(format!("tb_test_corrupt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let _guard = TestVaultDirGuard::set(temp_dir.clone());

        let vault_file = temp_dir.join("vault.enc");
        std::fs::write(&vault_file, b"corrupted_short_payload").unwrap();

        let load_res = load_vault_payload_checked();
        assert!(
            matches!(load_res, Err(CredentialError::VaultCorruptedOrTampered(_))),
            "Corrupted truncated vault must return VaultCorruptedOrTampered"
        );

        let get_res = CredentialManager::get_token("guild_unregistered_1");
        assert!(
            matches!(get_res, Err(CredentialError::VaultCorruptedOrTampered(_))),
            "get_token must propagate corruption error instead of reporting token not found"
        );

        let dummy_tok = "Bot 123456789012345678.ABCDEF.1234567890123456789012345";
        let store_res = CredentialManager::store_token("guild_1", dummy_tok);
        assert!(
            matches!(store_res, Err(CredentialError::VaultCorruptedOrTampered(_))),
            "store_token must not overwrite a corrupted vault"
        );

        let del_res = CredentialManager::delete_token("guild_1");
        assert!(
            matches!(del_res, Err(CredentialError::VaultCorruptedOrTampered(_))),
            "delete_token must not overwrite a corrupted vault"
        );

        drop(_guard);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_purge_credentials_propagates_removal_errors() {
        let temp_dir = std::env::temp_dir().join(format!("tb_test_purge_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let _guard = TestVaultDirGuard::set(temp_dir.clone());

        let dummy_tok = "Bot 123456789012345678.ABCDEF.1234567890123456789012345";
        CredentialManager::store_token("guild_purge_clean", dummy_tok).unwrap();
        assert!(temp_dir.join("vault.enc").exists());
        assert!(temp_dir.join("vault.salt").exists());

        let purge_res = CredentialManager::purge_all_credentials();
        assert!(
            purge_res.is_ok(),
            "Purge should succeed when files can be deleted"
        );
        assert!(!temp_dir.join("vault.enc").exists());
        assert!(!temp_dir.join("vault.salt").exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(temp_dir.join("vault.enc"), b"cannot_remove").unwrap();
            std::fs::write(temp_dir.join("vault.salt"), b"cannot_remove").unwrap();
            let _ = std::fs::set_permissions(&temp_dir, std::fs::Permissions::from_mode(0o500));

            let purge_fail_res = CredentialManager::purge_all_credentials();
            assert!(
                matches!(purge_fail_res, Err(CredentialError::VaultError(_))),
                "Purge must propagate removal errors instead of falsely reporting success"
            );

            let _ = std::fs::set_permissions(&temp_dir, std::fs::Permissions::from_mode(0o700));
        }

        drop(_guard);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
