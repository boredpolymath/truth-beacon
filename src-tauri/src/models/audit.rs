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
