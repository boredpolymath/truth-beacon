use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Dismiss,
    ExcludeUser,
    BanAndPurge,
    WhitelistAlternate,
    CreateBenchmark,
    UpdateBenchmark,
    DeleteBenchmark,
    CircuitBreakerTripped,
    CircuitBreakerReset,
    PromotedToBenchmark,
    AvatarHashSynchronized,
    #[serde(other)]
    Other,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dismiss => "dismiss",
            Self::ExcludeUser => "exclude_user",
            Self::BanAndPurge => "ban_and_purge",
            Self::WhitelistAlternate => "whitelist_alternate",
            Self::CreateBenchmark => "create_benchmark",
            Self::UpdateBenchmark => "update_benchmark",
            Self::DeleteBenchmark => "delete_benchmark",
            Self::CircuitBreakerTripped => "circuit_breaker_tripped",
            Self::CircuitBreakerReset => "circuit_breaker_reset",
            Self::PromotedToBenchmark => "promoted_to_benchmark",
            Self::AvatarHashSynchronized => "avatar_hash_synchronized",
            Self::Other => "other",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().replace('-', "_").as_str() {
            "dismiss" | "dismiss_coincidence" => Self::Dismiss,
            "exclude_user" | "quarantine_user" | "exclude" | "quarantine" => Self::ExcludeUser,
            "ban_and_purge" | "adjudicate_impersonation" | "ban" => Self::BanAndPurge,
            "whitelist_alternate" | "authorize_alternate" | "whitelist" => Self::WhitelistAlternate,
            "create_benchmark" => Self::CreateBenchmark,
            "update_benchmark" => Self::UpdateBenchmark,
            "delete_benchmark" => Self::DeleteBenchmark,
            "circuit_breaker_tripped" => Self::CircuitBreakerTripped,
            "circuit_breaker_reset" => Self::CircuitBreakerReset,
            "promoted_to_benchmark" => Self::PromotedToBenchmark,
            "avatar_hash_synchronized" => Self::AvatarHashSynchronized,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: i64,
    pub action: ActionType,
    pub guild_id: String,
    pub operator_id: String,
    pub target_user_id: Option<String>,
    pub incident_id: Option<String>,
    pub reason: String,
    pub metadata: Option<serde_json::Value>,
}

