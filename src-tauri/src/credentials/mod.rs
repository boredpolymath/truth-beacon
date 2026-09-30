use keyring::Entry;
use thiserror::Error;

const SERVICE_NAME: &str = "truth_beacon_secure_vault";

#[derive(Error, Debug)]
pub enum CredentialError {
    #[error("Keyring access failure: {0}")]
    KeyringError(#[from] keyring::Error),
    #[error("Missing token for guild {0}")]
    TokenNotFound(String),
}

pub struct CredentialManager;

impl CredentialManager {
    /// Save platform bot token securely to the host operating system's native keychain.
    pub fn store_token(guild_id: &str, token: &str) -> Result<(), CredentialError> {
        let entry = Entry::new(SERVICE_NAME, guild_id)?;
        entry.set_password(token)?;
        Ok(())
    }

    /// Retrieve platform bot token from the OS keychain. Never logged or stored in SQLite.
    pub fn get_token(guild_id: &str) -> Result<String, CredentialError> {
        let entry = Entry::new(SERVICE_NAME, guild_id)?;
        match entry.get_password() {
            Ok(token) => Ok(token),
            Err(keyring::Error::NoEntry) => Err(CredentialError::TokenNotFound(guild_id.to_string())),
            Err(e) => Err(CredentialError::KeyringError(e)),
        }
    }

    /// Purge credentials for a guild from the OS keychain upon user request or data eradication.
    pub fn delete_token(guild_id: &str) -> Result<(), CredentialError> {
        let entry = Entry::new(SERVICE_NAME, guild_id)?;
        match entry.delete_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(CredentialError::KeyringError(e)),
        }
    }
}
