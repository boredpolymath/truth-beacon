use serde::{Deserialize, Serialize};

/// Canonical tagging taxonomy for benchmarks (Phase 12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaxonomyTag {
    #[serde(rename = "Core Staff")]
    CoreStaff,
    #[serde(rename = "Public Creator")]
    PublicCreator,
    #[serde(rename = "Verified VIP")]
    VerifiedVip,
    #[serde(rename = "Authorized Alt")]
    AuthorizedAlt,
    #[serde(rename = "Approved Satire")]
    ApprovedSatire,
}

impl TaxonomyTag {
    pub const ALL: [TaxonomyTag; 5] = [
        TaxonomyTag::CoreStaff,
        TaxonomyTag::PublicCreator,
        TaxonomyTag::VerifiedVip,
        TaxonomyTag::AuthorizedAlt,
        TaxonomyTag::ApprovedSatire,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CoreStaff => "Core Staff",
            Self::PublicCreator => "Public Creator",
            Self::VerifiedVip => "Verified VIP",
            Self::AuthorizedAlt => "Authorized Alt",
            Self::ApprovedSatire => "Approved Satire",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        let lower = trimmed.to_lowercase().replace(['_', '-'], " ");
        match lower.as_str() {
            "core staff" | "staff" | "admin" => Some(Self::CoreStaff),
            "public creator" | "creator" | "streamer" => Some(Self::PublicCreator),
            "verified vip" | "vip" | "verified" => Some(Self::VerifiedVip),
            "authorized alt" | "alt" | "authorized" => Some(Self::AuthorizedAlt),
            "approved satire" | "satire" | "parody" => Some(Self::ApprovedSatire),
            _ => None,
        }
    }

    /// Returns true if this taxonomy tag automatically exempts the account from lookalike discrepancy alerts.
    pub fn is_exempt(&self) -> bool {
        matches!(self, Self::AuthorizedAlt | Self::ApprovedSatire)
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::CoreStaff => {
                "Server administrators, moderators, and executive leadership members with privileged roles."
            }
            Self::PublicCreator => {
                "High-profile creators, community figures, or streamers targeted by copycats."
            }
            Self::VerifiedVip => {
                "Distinguished guests, partner representatives, or VIPs requiring identity protection."
            }
            Self::AuthorizedAlt => {
                "Legitimate secondary account belonging to staff or VIPs; automatically exempted from impersonation alerts."
            }
            Self::ApprovedSatire => {
                "Officially recognized parody or comedy account; automatically exempted from lookalike discrepancy alerts."
            }
        }
    }
}

impl std::fmt::Display for TaxonomyTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyTagInfo {
    pub tag: String,
    pub display_name: String,
    pub is_exempt: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    #[serde(default)]
    pub sensitivity_override: Option<f64>,
}

impl CanonicalBenchmark {
    /// Returns true if this benchmark contains the specified taxonomy tag.
    pub fn has_taxonomy_tag(&self, tag: TaxonomyTag) -> bool {
        self.tags
            .iter()
            .any(|t| t == tag.as_str() || TaxonomyTag::from_str_loose(t) == Some(tag))
    }

    /// Returns true if this benchmark profile is marked as exempt from lookalike discrepancy alerts
    /// (e.g., tagged with `Authorized Alt` or `Approved Satire`).
    pub fn is_exempt(&self) -> bool {
        self.tags.iter().any(|t| {
            if let Some(tag) = TaxonomyTag::from_str_loose(t) {
                tag.is_exempt()
            } else {
                let lower = t.to_lowercase();
                lower == "authorized alt"
                    || lower == "approved satire"
                    || lower == "whitelisted alt"
                    || lower == "whitelisted"
            }
        })
    }

    /// Checks if this benchmark explicitly whitelists a candidate user ID as an authorized alt.
    /// Recognizes tags like "Alt: 123456789" or "Whitelisted Alt: 123456789".
    pub fn whitelists_alt_user(&self, candidate_user_id: &str) -> bool {
        self.tags.iter().any(|t| {
            let lower = t.to_lowercase();
            if let Some(rest) = lower.strip_prefix("alt:") {
                rest.trim() == candidate_user_id
            } else if let Some(rest) = lower.strip_prefix("whitelisted alt:") {
                rest.trim() == candidate_user_id
            } else {
                false
            }
        })
    }
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
    #[serde(default)]
    pub sensitivity_override: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateBenchmarkInput {
    #[serde(default)]
    pub server_nickname: Option<String>,
    #[serde(default)]
    pub community_role: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub avatar_perceptual_hash: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub sensitivity_override: Option<f64>,
}
