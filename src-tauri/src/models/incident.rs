use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    Standard,
    Notable,
    Elevated,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Pending,
    Dismissed,
    Excluded,
    Banned,
    Whitelisted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityDiscrepancy {
    pub matched_benchmark_id: String,
    pub matched_benchmark_name: String,
    pub suspect_user_id: String,
    pub suspect_username: String,
    pub suspect_nickname: Option<String>,
    pub suspect_avatar_url: Option<String>,
    pub suspect_account_age_hours: u64,
    pub string_similarity_score: f64,
    pub homoglyph_detected: bool,
    pub normalized_diff: String,
    pub avatar_hamming_distance: Option<u32>,
    pub risk_tier: RiskTier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageIncident {
    pub id: String,
    pub guild_id: String,
    pub timestamp: i64,
    pub discrepancy: IdentityDiscrepancy,
    pub status: IncidentStatus,
    pub resolution_notes: Option<String>,
    pub operator_id: Option<String>,
    pub resolved_at: Option<i64>,
}
