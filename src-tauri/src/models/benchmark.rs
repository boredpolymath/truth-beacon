use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalBenchmark {
    pub id: String,
    pub guild_id: String,
    pub user_id: String,
    pub canonical_username: String,
    pub server_nickname: Option<String>,
    pub community_role: String,
    pub avatar_url: Option<String>,
    pub avatar_perceptual_hash: Option<String>,
    pub is_active: bool,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBenchmarkInput {
    pub guild_id: String,
    pub user_id: String,
    pub canonical_username: String,
    pub server_nickname: Option<String>,
    pub community_role: String,
    pub avatar_url: Option<String>,
    pub tags: Vec<String>,
}
