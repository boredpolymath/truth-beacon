pub mod metrics;
pub mod perceptual_hash;
pub mod unicode;

use crate::models::{CanonicalBenchmark, IdentityDiscrepancy, RiskTier};
use metrics::evaluate_string_metrics;
use unicode::normalize_and_deobfuscate;

pub struct DetectionEngine {
    pub string_threshold: f64,
    pub avatar_hamming_threshold: u32,
    pub new_account_age_hours_threshold: u64,
}

impl Default for DetectionEngine {
    fn default() -> Self {
        Self {
            string_threshold: 0.82,
            avatar_hamming_threshold: 12,
            new_account_age_hours_threshold: 72,
        }
    }
}

impl DetectionEngine {
    pub fn new(string_threshold: f64, avatar_hamming_threshold: u32, new_account_age_hours_threshold: u64) -> Self {
        Self {
            string_threshold,
            avatar_hamming_threshold,
            new_account_age_hours_threshold,
        }
    }

    /// Evaluates an incoming member against canonical benchmarks to detect lookalikes and clones.
    pub fn evaluate_candidate(
        &self,
        candidate_user_id: &str,
        candidate_username: &str,
        candidate_nickname: Option<&str>,
        candidate_avatar_url: Option<&str>,
        candidate_age_hours: u64,
        benchmarks: &[CanonicalBenchmark],
    ) -> Option<IdentityDiscrepancy> {
        let normalized_candidate_name = normalize_and_deobfuscate(candidate_username);
        let normalized_candidate_nick = candidate_nickname.map(normalize_and_deobfuscate);

        let mut highest_discrepancy: Option<IdentityDiscrepancy> = None;
        let mut max_risk_weight = 0.0;

        for benchmark in benchmarks {
            // Ignore if candidate is the actual benchmark identity
            if benchmark.user_id == candidate_user_id {
                continue;
            }

            let normalized_bm_name = normalize_and_deobfuscate(&benchmark.canonical_username);
            let name_eval = evaluate_string_metrics(&normalized_candidate_name, &normalized_bm_name);

            let nick_eval = if let (Some(nick), Some(bm_nick)) = (
                &normalized_candidate_nick,
                benchmark.server_nickname.as_ref().map(|n| normalize_and_deobfuscate(n)),
            ) {
                Some(evaluate_string_metrics(nick, &bm_nick))
            } else {
                None
            };

            let top_string_score = if let Some(ne) = nick_eval {
                name_eval.composite_score.max(ne.composite_score)
            } else {
                name_eval.composite_score
            };

            let is_homoglyph = candidate_username != normalized_candidate_name
                && normalized_candidate_name == normalized_bm_name;

            // Determine if candidate crosses risk thresholds
            if top_string_score >= self.string_threshold || is_homoglyph {
                let is_new_account = candidate_age_hours < self.new_account_age_hours_threshold;

                let risk_tier = match (top_string_score, is_new_account, is_homoglyph) {
                    (score, true, _) if score > 0.95 => RiskTier::Critical,
                    (_, true, true) => RiskTier::Critical,
                    (score, false, _) if score > 0.92 => RiskTier::Elevated,
                    (_, true, _) => RiskTier::Elevated,
                    (score, _, _) if score >= 0.85 => RiskTier::Notable,
                    _ => RiskTier::Standard,
                };

                let risk_weight = match risk_tier {
                    RiskTier::Critical => 4.0,
                    RiskTier::Elevated => 3.0,
                    RiskTier::Notable => 2.0,
                    RiskTier::Standard => 1.0,
                };

                if risk_weight > max_risk_weight {
                    max_risk_weight = risk_weight;
                    highest_discrepancy = Some(IdentityDiscrepancy {
                        matched_benchmark_id: benchmark.id.clone(),
                        matched_benchmark_name: benchmark.canonical_username.clone(),
                        suspect_user_id: candidate_user_id.to_string(),
                        suspect_username: candidate_username.to_string(),
                        suspect_nickname: candidate_nickname.map(|s| s.to_string()),
                        suspect_avatar_url: candidate_avatar_url.map(|s| s.to_string()),
                        suspect_account_age_hours: candidate_age_hours,
                        string_similarity_score: (top_string_score * 100.0).round() / 100.0,
                        homoglyph_detected: is_homoglyph,
                        normalized_diff: format!("Candidate: '{}' -> Match: '{}'", normalized_candidate_name, normalized_bm_name),
                        avatar_hamming_distance: None,
                        risk_tier,
                    });
                }
            }
        }

        highest_discrepancy
    }
}
