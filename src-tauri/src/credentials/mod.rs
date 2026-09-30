use keyring::Entry;
use thiserror::Error;

const SERVICE_NAME: &str = "truth_beacon_secure_vault";

#[derive(Error, Debug)]
pub enum CredentialError {
    #[error("Keyring access failure: {0}")]
    KeyringError(#[from] keyring::Error),
    #[error("Missing token for guild {0}")]
    TokenNotFound(String),
    #[error("Invalid bot token format. Only official Discord Bot tokens are permitted; user tokens and self-bots are strictly prohibited.")]
    InvalidBotTokenFormat,
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

    /// Save platform bot token securely to the host operating system's native keychain.
    pub fn store_token(guild_id: &str, token: &str) -> Result<(), CredentialError> {
        let clean_token = Self::validate_bot_token_format(token)?;
        let entry = Entry::new(SERVICE_NAME, guild_id)?;
        entry.set_password(&clean_token)?;
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
        match entry.delete_credential() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(CredentialError::KeyringError(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_bot_token_format() {
        // Construct mock token dynamically to prevent secret scanning false-positives on git push
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
        // User tokens or arbitrary strings that do not follow 3-part Discord Bot format
        assert!(CredentialManager::validate_bot_token_format("invalid_token_string").is_err());
        assert!(CredentialManager::validate_bot_token_format("partone.parttwo").is_err());
        assert!(CredentialManager::validate_bot_token_format("").is_err());
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
}
