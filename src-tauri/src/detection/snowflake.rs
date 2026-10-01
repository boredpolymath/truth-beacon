//! Discord Snowflake Timestamp & Account Age Calculator (Phase 13.5)
//!
//! Discord uses Twitter-style 64-bit Snowflake IDs for users, messages, guilds, and channels.
//! The first 42 bits encode the timestamp (milliseconds since Discord Epoch: January 1, 2015 00:00:00 UTC).
//!
//! Extraction formula:
//! `timestamp_ms = (snowflake >> 22) + 1420070400000`
//!
//! This module parses snowflakes, extracts creation timestamps, calculates account age in hours,
//! days, and years, and flags accounts less than 72 hours old as heightened risk factors.

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// Discord Epoch in milliseconds since Unix Epoch: January 1, 2015 00:00:00.000 UTC
pub const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

/// Default threshold in hours to flag an account as brand new (Heightened Risk Factor)
pub const BRAND_NEW_ACCOUNT_HOURS_THRESHOLD: u64 = 72;

pub const MS_PER_SECOND: f64 = 1_000.0;
pub const MS_PER_MINUTE: f64 = 60.0 * MS_PER_SECOND;
pub const MS_PER_HOUR: f64 = 60.0 * MS_PER_MINUTE; // 3,600,000.0
pub const MS_PER_DAY: f64 = 24.0 * MS_PER_HOUR; // 86,400,000.0
pub const DAYS_PER_YEAR: f64 = 365.2425; // Mean Gregorian calendar year

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum SnowflakeError {
    #[error("Snowflake input cannot be empty")]
    EmptyInput,

    #[error("Invalid snowflake format: {0}")]
    InvalidFormat(String),

    #[error("Invalid timestamp generated from snowflake: {0} ms")]
    InvalidTimestamp(i64),
}

/// Structured representation of a Discord account's age and creation metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountAge {
    /// The original 64-bit snowflake ID
    pub snowflake: u64,
    /// Unix timestamp in milliseconds when the account was created
    pub created_at_ms: u64,
    /// ISO-8601 UTC timestamp of account creation
    pub created_at_utc: DateTime<Utc>,
    /// Account age in hours
    pub age_hours: f64,
    /// Account age in days
    pub age_days: f64,
    /// Account age in years
    pub age_years: f64,
    /// Flag indicating whether the account is < 72 hours old (Heightened Risk)
    pub is_brand_new: bool,
}

impl AccountAge {
    /// Calculate account age from a 64-bit snowflake against a reference UTC time.
    pub fn from_snowflake(snowflake: u64, now: DateTime<Utc>) -> Result<Self, SnowflakeError> {
        let created_at_ms = extract_snowflake_timestamp_ms(snowflake);
        let created_at_utc = snowflake_to_datetime(snowflake)?;

        let now_ms = now.timestamp_millis();
        let created_ms_i64 = created_at_ms as i64;

        // Safely guard against clock drift or future snowflakes without underflow
        let age_ms = if now_ms > created_ms_i64 {
            (now_ms - created_ms_i64) as f64
        } else {
            0.0
        };

        let age_hours = age_ms / MS_PER_HOUR;
        let age_days = age_ms / MS_PER_DAY;
        let age_years = age_days / DAYS_PER_YEAR;
        let is_brand_new = age_hours < BRAND_NEW_ACCOUNT_HOURS_THRESHOLD as f64;

        Ok(Self {
            snowflake,
            created_at_ms,
            created_at_utc,
            age_hours,
            age_days,
            age_years,
            is_brand_new,
        })
    }

    /// Calculate account age from a string snowflake against a reference UTC time.
    pub fn from_snowflake_str(
        snowflake_str: &str,
        now: DateTime<Utc>,
    ) -> Result<Self, SnowflakeError> {
        let snowflake = parse_snowflake_str(snowflake_str)?;
        Self::from_snowflake(snowflake, now)
    }

    /// Calculate account age from a 64-bit snowflake evaluated at `Utc::now()`.
    pub fn from_snowflake_now(snowflake: u64) -> Result<Self, SnowflakeError> {
        Self::from_snowflake(snowflake, Utc::now())
    }

    /// Calculate account age from a string snowflake evaluated at `Utc::now()`.
    pub fn from_snowflake_str_now(snowflake_str: &str) -> Result<Self, SnowflakeError> {
        Self::from_snowflake_str(snowflake_str, Utc::now())
    }

    /// Rounded age in whole hours
    pub fn age_hours_rounded(&self) -> u64 {
        self.age_hours.floor() as u64
    }

    /// Rounded age in whole days
    pub fn age_days_rounded(&self) -> u64 {
        self.age_days.floor() as u64
    }
}

/// Parses a string slice into a 64-bit unsigned snowflake ID.
pub fn parse_snowflake_str(snowflake_str: &str) -> Result<u64, SnowflakeError> {
    let trimmed = snowflake_str.trim();
    if trimmed.is_empty() {
        return Err(SnowflakeError::EmptyInput);
    }
    trimmed.parse::<u64>().map_err(|e| {
        SnowflakeError::InvalidFormat(format!("Failed to parse snowflake '{}': {}", trimmed, e))
    })
}

/// Extracts the Discord snowflake creation timestamp in milliseconds since Unix Epoch.
///
/// Formula: `(snowflake >> 22) + 1420070400000`
#[inline]
pub fn extract_snowflake_timestamp_ms(snowflake: u64) -> u64 {
    (snowflake >> 22) + DISCORD_EPOCH_MS
}

/// Extracts the Discord snowflake creation timestamp in milliseconds from a snowflake string.
pub fn extract_snowflake_timestamp_ms_from_str(snowflake_str: &str) -> Result<u64, SnowflakeError> {
    let snowflake = parse_snowflake_str(snowflake_str)?;
    Ok(extract_snowflake_timestamp_ms(snowflake))
}

/// Converts a 64-bit Discord snowflake into a UTC `DateTime`.
pub fn snowflake_to_datetime(snowflake: u64) -> Result<DateTime<Utc>, SnowflakeError> {
    let ts_ms = extract_snowflake_timestamp_ms(snowflake);
    Utc.timestamp_millis_opt(ts_ms as i64)
        .single()
        .ok_or(SnowflakeError::InvalidTimestamp(ts_ms as i64))
}

/// Converts a string Discord snowflake into a UTC `DateTime`.
pub fn snowflake_str_to_datetime(snowflake_str: &str) -> Result<DateTime<Utc>, SnowflakeError> {
    let snowflake = parse_snowflake_str(snowflake_str)?;
    snowflake_to_datetime(snowflake)
}

/// Calculates account age in hours from a snowflake and reference Unix timestamp in milliseconds.
pub fn calculate_account_age_hours(snowflake: u64, now_ms: u64) -> f64 {
    let created_ms = extract_snowflake_timestamp_ms(snowflake);
    if now_ms > created_ms {
        (now_ms - created_ms) as f64 / MS_PER_HOUR
    } else {
        0.0
    }
}

/// Calculates account age in days from a snowflake and reference Unix timestamp in milliseconds.
pub fn calculate_account_age_days(snowflake: u64, now_ms: u64) -> f64 {
    let created_ms = extract_snowflake_timestamp_ms(snowflake);
    if now_ms > created_ms {
        (now_ms - created_ms) as f64 / MS_PER_DAY
    } else {
        0.0
    }
}

/// Calculates account age in years from a snowflake and reference Unix timestamp in milliseconds.
pub fn calculate_account_age_years(snowflake: u64, now_ms: u64) -> f64 {
    calculate_account_age_days(snowflake, now_ms) / DAYS_PER_YEAR
}

/// Flags whether an account is brand new based on snowflake and current Unix time in milliseconds.
/// If `threshold_hours` is `None`, defaults to 72 hours.
pub fn is_brand_new_account(snowflake: u64, now_ms: u64, threshold_hours: Option<u64>) -> bool {
    let threshold = threshold_hours.unwrap_or(BRAND_NEW_ACCOUNT_HOURS_THRESHOLD) as f64;
    calculate_account_age_hours(snowflake, now_ms) < threshold
}

/// Flags whether an account is brand new from a snowflake string.
pub fn is_brand_new_account_from_str(
    snowflake_str: &str,
    now_ms: u64,
    threshold_hours: Option<u64>,
) -> Result<bool, SnowflakeError> {
    let snowflake = parse_snowflake_str(snowflake_str)?;
    Ok(is_brand_new_account(snowflake, now_ms, threshold_hours))
}

/// Constructs a Discord snowflake for testing purposes given a creation timestamp in milliseconds.
#[cfg(test)]
pub fn build_mock_snowflake(timestamp_ms: u64) -> u64 {
    if timestamp_ms >= DISCORD_EPOCH_MS {
        let offset = timestamp_ms - DISCORD_EPOCH_MS;
        (offset << 22) | 0x003F_FFFF
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_snowflake_epoch_zero() {
        // Snowflake 0 corresponds exactly to Discord Epoch (2015-01-01 00:00:00 UTC)
        let ts_ms = extract_snowflake_timestamp_ms(0);
        assert_eq!(ts_ms, DISCORD_EPOCH_MS);
        assert_eq!(ts_ms, 1_420_070_400_000);

        let dt = snowflake_to_datetime(0).unwrap();
        assert_eq!(dt.to_rfc3339(), "2015-01-01T00:00:00+00:00");
    }

    #[test]
    fn test_extract_known_real_world_discord_snowflake() {
        // Known Discord Snowflake: 175928847299117063
        // 175928847299117063 >> 22 = 41944705796
        // 41944705796 + 1420070400000 = 1462015105796 ms
        let snowflake = 175928847299117063u64;
        let ts_ms = extract_snowflake_timestamp_ms(snowflake);
        assert_eq!(ts_ms, 1_462_015_105_796);

        let dt = snowflake_to_datetime(snowflake).unwrap();
        // 1462015105796 ms = 2016-04-30 11:18:25.796 UTC
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2016-04-30 11:18:25"
        );
    }

    #[test]
    fn test_parse_snowflake_str_valid_and_invalid() {
        assert_eq!(
            parse_snowflake_str("175928847299117063").unwrap(),
            175928847299117063u64
        );
        assert_eq!(
            parse_snowflake_str("  175928847299117063 \n").unwrap(),
            175928847299117063u64
        );

        assert!(matches!(
            parse_snowflake_str(""),
            Err(SnowflakeError::EmptyInput)
        ));
        assert!(matches!(
            parse_snowflake_str("   "),
            Err(SnowflakeError::EmptyInput)
        ));
        assert!(matches!(
            parse_snowflake_str("not_a_number"),
            Err(SnowflakeError::InvalidFormat(_))
        ));
        assert!(matches!(
            parse_snowflake_str("-100"),
            Err(SnowflakeError::InvalidFormat(_))
        ));
    }

    #[test]
    fn test_calculate_account_age_hours_days_years() {
        let reference_now_ms = 1_700_000_000_000u64; // arbitrary reference time
        let reference_now = Utc.timestamp_millis_opt(reference_now_ms as i64).unwrap();

        // 100 days old account: 100 * 24 * 3600 * 1000 = 8,640,000,000 ms
        let created_ms = reference_now_ms - 8_640_000_000;
        let snowflake = build_mock_snowflake(created_ms);

        let age = AccountAge::from_snowflake(snowflake, reference_now).unwrap();
        assert_eq!(age.age_days.round(), 100.0);
        assert_eq!(age.age_hours.round(), 2400.0);
        assert_eq!(age.age_days_rounded(), 100);
        assert_eq!(age.age_hours_rounded(), 2400);

        // 100 days / 365.2425 ≈ 0.27379 years
        let diff_years = (age.age_years - (100.0 / DAYS_PER_YEAR)).abs();
        assert!(diff_years < 1e-4);

        // 100 days > 72 hours -> not brand new
        assert!(!age.is_brand_new);
    }

    #[test]
    fn test_flag_brand_new_account_under_72_hours() {
        let reference_now_ms = 1_700_000_000_000u64;
        let reference_now = Utc.timestamp_millis_opt(reference_now_ms as i64).unwrap();

        // Account created 2 hours ago: 2 * 3600 * 1000 = 7,200,000 ms
        let created_2h = reference_now_ms - 7_200_000;
        let snowflake_2h = build_mock_snowflake(created_2h);

        let age_2h = AccountAge::from_snowflake(snowflake_2h, reference_now).unwrap();
        assert_eq!(age_2h.age_hours.round(), 2.0);
        assert!(age_2h.is_brand_new);
        assert!(is_brand_new_account(snowflake_2h, reference_now_ms, None));

        // Account created 71.9 hours ago (< 72 hours threshold)
        let created_71h = reference_now_ms - (71 * 3_600_000 + 54 * 60_000);
        let snowflake_71h = build_mock_snowflake(created_71h);
        let age_71h = AccountAge::from_snowflake(snowflake_71h, reference_now).unwrap();
        assert!(age_71h.is_brand_new);

        // Account created 73 hours ago (> 72 hours threshold)
        let created_73h = reference_now_ms - (73 * 3_600_000);
        let snowflake_73h = build_mock_snowflake(created_73h);
        let age_73h = AccountAge::from_snowflake(snowflake_73h, reference_now).unwrap();
        assert!(!age_73h.is_brand_new);
        assert!(!is_brand_new_account(snowflake_73h, reference_now_ms, None));
    }

    #[test]
    fn test_clock_skew_future_snowflake_safe_handling() {
        let reference_now_ms = 1_700_000_000_000u64;
        let reference_now = Utc.timestamp_millis_opt(reference_now_ms as i64).unwrap();

        // Snowflake created 10 seconds into the future due to clock drift
        let future_created = reference_now_ms + 10_000;
        let future_snowflake = build_mock_snowflake(future_created);

        let age = AccountAge::from_snowflake(future_snowflake, reference_now).unwrap();
        assert_eq!(age.age_hours, 0.0);
        assert_eq!(age.age_days, 0.0);
        assert_eq!(age.age_years, 0.0);
        assert!(age.is_brand_new); // 0 hours < 72 hours
    }
}
