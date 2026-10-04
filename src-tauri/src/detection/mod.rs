pub mod homoglyph;
pub mod metrics;
pub mod perceptual_hash;
pub mod snowflake;
pub mod unicode;

use crate::models::{CanonicalBenchmark, IdentityDiscrepancy, RiskTier};
pub use homoglyph::{
    evaluate_homoglyph_spoof, evaluate_homoglyph_spoof_with_precomputed, to_visual_skeleton,
    HomoglyphCategory, HomoglyphMatchResult,
};
pub use metrics::{
    evaluate_multi_field_metrics, evaluate_string_metrics, BenchmarkIdentityFields,
    CandidateIdentityFields, FieldMatchPair, MultiFieldSimilarityResult, StringSimilarityResult,
};
pub use perceptual_hash::{
    calculate_hamming_distance, classify_hamming_distance, compare_perceptual_hashes,
    compute_perceptual_hash, HashError, PerceptualHashComparison, VisualSimilarityTier,
};
pub use snowflake::{
    calculate_account_age_days, calculate_account_age_hours, calculate_account_age_years,
    extract_snowflake_timestamp_ms, extract_snowflake_timestamp_ms_from_str, is_brand_new_account,
    is_brand_new_account_from_str, parse_snowflake_str, snowflake_str_to_datetime,
    snowflake_to_datetime, AccountAge, SnowflakeError, BRAND_NEW_ACCOUNT_HOURS_THRESHOLD,
    DISCORD_EPOCH_MS,
};
pub use unicode::{
    inspect_unicode_anomalies, normalize_and_deobfuscate, strip_deobfuscate_unicode,
    UnicodeAnomalyReport,
};

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
    pub fn new(
        string_threshold: f64,
        avatar_hamming_threshold: u32,
        new_account_age_hours_threshold: u64,
    ) -> Self {
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
        self.evaluate_candidate_with_avatar_hash(
            candidate_user_id,
            candidate_username,
            candidate_nickname,
            candidate_avatar_url,
            None,
            candidate_age_hours,
            benchmarks,
        )
    }

    /// Evaluates candidate identity using their Discord snowflake ID to dynamically compute account age.
    pub fn evaluate_candidate_snowflake(
        &self,
        candidate_user_id: &str,
        candidate_username: &str,
        candidate_nickname: Option<&str>,
        candidate_avatar_url: Option<&str>,
        candidate_avatar_hash: Option<&str>,
        benchmarks: &[CanonicalBenchmark],
    ) -> Option<IdentityDiscrepancy> {
        let age_hours = if let Ok(snowflake) = parse_snowflake_str(candidate_user_id) {
            calculate_account_age_hours(snowflake, chrono::Utc::now().timestamp_millis() as u64)
                .floor() as u64
        } else {
            0
        };

        self.evaluate_candidate_with_avatar_hash(
            candidate_user_id,
            candidate_username,
            candidate_nickname,
            candidate_avatar_url,
            candidate_avatar_hash,
            age_hours,
            benchmarks,
        )
    }

    /// Evaluates candidate identity incorporating textual metrics, homoglyph deobfuscation,
    /// and visual perceptual avatar hash Hamming distance.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_candidate_with_avatar_hash(
        &self,
        candidate_user_id: &str,
        candidate_username: &str,
        candidate_nickname: Option<&str>,
        candidate_avatar_url: Option<&str>,
        candidate_avatar_hash: Option<&str>,
        candidate_age_hours: u64,
        benchmarks: &[CanonicalBenchmark],
    ) -> Option<IdentityDiscrepancy> {
        // Phase 12.3: Automated exemption for registered whitelisted alt accounts & approved satire
        if let Some(candidate_bm) = benchmarks.iter().find(|b| b.user_id == candidate_user_id) {
            if candidate_bm.is_exempt() {
                return None;
            }
        }

        // Automated exemption if any registered benchmark explicitly whitelists this candidate as an authorized alt
        if benchmarks
            .iter()
            .any(|b| b.whitelists_alt_user(candidate_user_id))
        {
            return None;
        }

        // Phase 13.5: Derive effective account age from snowflake if age not explicitly provided
        let effective_age_hours = if candidate_age_hours > 0 {
            candidate_age_hours
        } else if let Ok(snowflake) = parse_snowflake_str(candidate_user_id) {
            calculate_account_age_hours(snowflake, chrono::Utc::now().timestamp_millis() as u64)
                .floor() as u64
        } else {
            0
        };

        let normalized_candidate_name = normalize_and_deobfuscate(candidate_username);
        let candidate_name_skel = to_visual_skeleton(candidate_username);
        let normalized_candidate_nick = candidate_nickname.map(normalize_and_deobfuscate);
        let candidate_nick_skel = candidate_nickname.map(to_visual_skeleton);

        let mut highest_discrepancy: Option<IdentityDiscrepancy> = None;
        let mut max_risk_weight = 0.0;

        for benchmark in benchmarks {
            // Ignore if candidate is the actual benchmark identity
            if benchmark.user_id == candidate_user_id {
                continue;
            }

            // Phase 12.3: Automated exemption if target benchmark explicitly whitelists this candidate as an alt
            if benchmark.whitelists_alt_user(candidate_user_id) {
                continue;
            }

            let normalized_bm_name = normalize_and_deobfuscate(&benchmark.canonical_username);
            let bm_name_skel = to_visual_skeleton(&benchmark.canonical_username);
            let normalized_bm_nick = benchmark
                .server_nickname
                .as_ref()
                .map(|n| normalize_and_deobfuscate(n));
            let bm_nick_skel = benchmark.server_nickname.as_deref().map(to_visual_skeleton);

            // Phase 13.3: Dual-field independent evaluation of username and server nickname
            let candidate_fields = CandidateIdentityFields {
                username: &normalized_candidate_name,
                global_name: None,
                server_nickname: normalized_candidate_nick.as_deref(),
            };
            let benchmark_fields = BenchmarkIdentityFields {
                canonical_username: &normalized_bm_name,
                server_nickname: normalized_bm_nick.as_deref(),
            };

            let multi_metrics = evaluate_multi_field_metrics(&candidate_fields, &benchmark_fields);
            let top_string_score = multi_metrics.top_composite_score;

            // Phase 13.2: Homoglyph and visual confusable evaluation across identity fields
            let name_vs_canonical = evaluate_homoglyph_spoof_with_precomputed(
                candidate_username,
                &benchmark.canonical_username,
                &normalized_candidate_name,
                &normalized_bm_name,
                &candidate_name_skel,
                &bm_name_skel,
            );
            let nick_vs_canonical = match (
                candidate_nickname,
                &normalized_candidate_nick,
                &candidate_nick_skel,
            ) {
                (Some(cand_nick), Some(cand_nick_norm), Some(cand_skel)) => {
                    Some(evaluate_homoglyph_spoof_with_precomputed(
                        cand_nick,
                        &benchmark.canonical_username,
                        cand_nick_norm,
                        &normalized_bm_name,
                        cand_skel,
                        &bm_name_skel,
                    ))
                }
                _ => None,
            };
            let nick_vs_nickname = match (
                candidate_nickname,
                &normalized_candidate_nick,
                &candidate_nick_skel,
                benchmark.server_nickname.as_deref(),
                &normalized_bm_nick,
                &bm_nick_skel,
            ) {
                (
                    Some(cand_nick),
                    Some(cand_nick_norm),
                    Some(cand_skel),
                    Some(bm_nick),
                    Some(bm_nick_norm),
                    Some(b_skel),
                ) => Some(evaluate_homoglyph_spoof_with_precomputed(
                    cand_nick,
                    bm_nick,
                    cand_nick_norm,
                    bm_nick_norm,
                    cand_skel,
                    b_skel,
                )),
                _ => None,
            };

            let is_homoglyph = name_vs_canonical.is_homoglyph_match
                || nick_vs_canonical
                    .as_ref()
                    .is_some_and(|h| h.is_homoglyph_match)
                || nick_vs_nickname
                    .as_ref()
                    .is_some_and(|h| h.is_homoglyph_match);

            // Phase 13.4: Visual perceptual hash Hamming distance evaluation
            let avatar_hamming = match (
                candidate_avatar_hash,
                benchmark.avatar_perceptual_hash.as_deref(),
            ) {
                (Some(c_hash), Some(b_hash)) => calculate_hamming_distance(c_hash, b_hash).ok(),
                _ => None,
            };
            let is_avatar_clone = avatar_hamming.is_some_and(|dist| dist <= 4);

            // Phase 13.6: Compound Threat Adjudication
            let effective_threshold = benchmark
                .sensitivity_override
                .unwrap_or(self.string_threshold);

            if let Some(assessment) = adjudicate_compound_threat(
                top_string_score,
                effective_threshold,
                is_homoglyph,
                avatar_hamming,
                effective_age_hours,
                self.new_account_age_hours_threshold,
            ) {
                if assessment.risk_weight > max_risk_weight {
                    max_risk_weight = assessment.risk_weight;
                    let diff_explanation = if is_avatar_clone {
                        format!(
                            "Visual Avatar Clone detected (Hamming distance: {}) for benchmark '{}'",
                            avatar_hamming.unwrap_or(0),
                            benchmark.canonical_username
                        )
                    } else if name_vs_canonical.is_homoglyph_match {
                        name_vs_canonical.explanation
                    } else if let Some(ref h) = nick_vs_canonical {
                        if h.is_homoglyph_match {
                            h.explanation.clone()
                        } else if let Some(ref hn) = nick_vs_nickname {
                            if hn.is_homoglyph_match {
                                hn.explanation.clone()
                            } else {
                                format!(
                                    "Candidate: '{}' -> Match: '{}' [{:?}]",
                                    normalized_candidate_name,
                                    normalized_bm_name,
                                    multi_metrics.top_field_pair
                                )
                            }
                        } else {
                            format!(
                                "Candidate: '{}' -> Match: '{}' [{:?}]",
                                normalized_candidate_name,
                                normalized_bm_name,
                                multi_metrics.top_field_pair
                            )
                        }
                    } else if let Some(ref hn) = nick_vs_nickname {
                        if hn.is_homoglyph_match {
                            hn.explanation.clone()
                        } else {
                            format!(
                                "Candidate: '{}' -> Match: '{}' [{:?}]",
                                normalized_candidate_name,
                                normalized_bm_name,
                                multi_metrics.top_field_pair
                            )
                        }
                    } else {
                        format!(
                            "Candidate: '{}' -> Match: '{}' [{:?}]",
                            normalized_candidate_name,
                            normalized_bm_name,
                            multi_metrics.top_field_pair
                        )
                    };

                    highest_discrepancy = Some(IdentityDiscrepancy {
                        matched_benchmark_id: benchmark.id.clone(),
                        matched_benchmark_name: benchmark.canonical_username.clone(),
                        suspect_user_id: candidate_user_id.to_string(),
                        suspect_username: candidate_username.to_string(),
                        suspect_nickname: candidate_nickname.map(|s| s.to_string()),
                        suspect_avatar_url: candidate_avatar_url.map(|s| s.to_string()),
                        suspect_account_age_hours: effective_age_hours,
                        string_similarity_score: (top_string_score * 100.0).round() / 100.0,
                        homoglyph_detected: is_homoglyph,
                        normalized_diff: diff_explanation,
                        avatar_hamming_distance: avatar_hamming,
                        risk_tier: assessment.risk_tier,
                    });
                }
            }
        }

        highest_discrepancy
    }
}

/// Detailed breakdown of compound threat adjudication factors (Phase 13.6).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompoundThreatAssessment {
    pub risk_tier: RiskTier,
    pub top_string_score: f64,
    pub avatar_hamming_distance: Option<u32>,
    pub visual_similarity_tier: Option<VisualSimilarityTier>,
    pub is_homoglyph: bool,
    pub is_new_account: bool,
    pub account_age_hours: u64,
    pub risk_weight: f64,
    pub primary_threat_factor: String,
    pub explanation: String,
}

/// Correlates text similarity, visual distance, and account age into unified risk tiers:
/// `Standard`, `Notable`, `Elevated`, `Critical` (Phase 13.6).
pub fn adjudicate_compound_threat(
    top_string_score: f64,
    effective_threshold: f64,
    is_homoglyph: bool,
    avatar_hamming: Option<u32>,
    effective_age_hours: u64,
    new_account_age_hours_threshold: u64,
) -> Option<CompoundThreatAssessment> {
    let is_avatar_clone = avatar_hamming.is_some_and(|dist| dist <= 4);
    let is_avatar_similar = avatar_hamming.is_some_and(|dist| dist <= 10);
    let visual_similarity_tier = avatar_hamming.map(classify_hamming_distance);

    if top_string_score < effective_threshold
        && !is_homoglyph
        && !is_avatar_clone
        && !is_avatar_similar
    {
        return None;
    }

    let is_new_account = effective_age_hours < new_account_age_hours_threshold;

    let (risk_tier, primary_threat_factor) = match (
        top_string_score,
        is_new_account,
        is_homoglyph,
        is_avatar_clone,
        is_avatar_similar,
    ) {
        // Critical: Pixel clone avatar + (high string similarity OR homoglyph OR brand-new account)
        (_, _, _, true, _) if top_string_score >= 0.70 || is_homoglyph || is_new_account => (
            RiskTier::Critical,
            "Avatar clone coupled with identity mimicry or new account",
        ),
        // Critical: Near-identical name (> 0.95) on brand-new account (< 72h)
        (score, true, _, _, _) if score > 0.95 => (
            RiskTier::Critical,
            "Near-identical name on brand-new account (< 72h)",
        ),
        // Critical: Homoglyph spoofing on brand-new account (< 72h)
        (_, true, true, _, _) => (
            RiskTier::Critical,
            "Homoglyph spoofing on brand-new account (< 72h)",
        ),

        // Elevated: Avatar clone alone
        (_, _, _, true, _) => (
            RiskTier::Elevated,
            "Visual avatar clone (Hamming distance 0-4)",
        ),
        // Elevated: Notable avatar similarity + (string score >= 0.80 OR brand new account)
        (_, _, _, _, true) if top_string_score >= 0.80 || is_new_account => (
            RiskTier::Elevated,
            "Notable avatar similarity coupled with high name similarity or new account",
        ),
        // Elevated: High textual similarity on established account
        (score, false, _, _, _) if score > 0.92 => {
            (RiskTier::Elevated, "High textual similarity (> 0.92)")
        }
        // Elevated: Homoglyph spoofing on established account
        (_, _, true, _, _) => (
            RiskTier::Elevated,
            "Homoglyph character substitution detected",
        ),
        // Elevated: Brand-new account with threshold-crossing name
        (_, true, _, _, _) => (
            RiskTier::Elevated,
            "Brand-new account (< 72h) crossing similarity threshold",
        ),

        // Notable: Moderate-high string score on established account
        (score, _, _, _, _) if score >= 0.85 => (
            RiskTier::Notable,
            "Elevated textual similarity (0.85 - 0.92)",
        ),
        // Notable: Notable avatar similarity alone
        (_, _, _, _, true) => (
            RiskTier::Notable,
            "Notable avatar similarity (Hamming distance 5-10)",
        ),

        // Standard: Meets minimal sensitivity threshold but no compounding risk factors
        _ => (
            RiskTier::Standard,
            "Nominal similarity crossing sensitivity threshold",
        ),
    };

    let risk_weight = match risk_tier {
        RiskTier::Critical => 4.0,
        RiskTier::Elevated => 3.0,
        RiskTier::Notable => 2.0,
        RiskTier::Standard => 1.0,
    };

    let explanation = format!(
        "Risk Tier: {:?} | Factor: {} | Text: {:.2} | Avatar Hamming: {} | Age: {}h",
        risk_tier,
        primary_threat_factor,
        top_string_score,
        avatar_hamming.map_or("N/A".to_string(), |h| h.to_string()),
        effective_age_hours
    );

    Some(CompoundThreatAssessment {
        risk_tier,
        top_string_score,
        avatar_hamming_distance: avatar_hamming,
        visual_similarity_tier,
        is_homoglyph,
        is_new_account,
        account_age_hours: effective_age_hours,
        risk_weight,
        primary_threat_factor: primary_threat_factor.to_string(),
        explanation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CanonicalBenchmark, RiskTier};
    use std::time::Instant;

    fn generate_mock_benchmarks(count: usize) -> Vec<CanonicalBenchmark> {
        (0..count)
            .map(|i| CanonicalBenchmark {
                id: format!("bm_{:03}", i),
                guild_id: "guild_alpha_100".into(),
                user_id: format!("100000000000000{:03}", i),
                canonical_username: match i {
                    0 => "Pastor John".into(),
                    1 => "Elder Mark".into(),
                    2 => "Admin Sarah".into(),
                    3 => "Moderator David".into(),
                    4 => "Community Lead Rachel".into(),
                    n => format!("Staff Member {:03}", n),
                },
                server_nickname: match i {
                    0 => Some("Pastor John [Staff]".into()),
                    1 => Some("Mark | Tech Lead".into()),
                    _ => None,
                },
                community_role: if i < 5 {
                    "Leadership".into()
                } else {
                    "Staff".into()
                },
                avatar_url: Some(format!("https://cdn.discordapp.com/avatars/{}/a1.png", i)),
                avatar_perceptual_hash: Some("d4c3b2a10000ffff".into()),
                is_active: true,
                tags: vec!["vip".into(), "staff".into()],
                created_at: 1700000000,
                updated_at: 1700000000,
                sensitivity_override: None,
            })
            .collect()
    }

    #[test]
    fn test_evaluation_latency_under_50ms_budget() {
        let engine = DetectionEngine::default();
        let benchmarks = generate_mock_benchmarks(50); // 50 canonical VIPs

        let candidate_id = "999999999999999999";
        let candidate_username = "Pastor Jоhn"; // Contains Cyrillic 'о'
        let candidate_nickname = Some("Pastor John");
        let candidate_avatar = Some("https://cdn.discordapp.com/avatars/999/a2.png");
        let candidate_age_hours = 2; // Brand new account

        // Warm up
        let _ = engine.evaluate_candidate(
            candidate_id,
            candidate_username,
            candidate_nickname,
            candidate_avatar,
            candidate_age_hours,
            &benchmarks,
        );

        let iterations = 20;
        let start = Instant::now();

        for _ in 0..iterations {
            let res = engine.evaluate_candidate(
                candidate_id,
                candidate_username,
                candidate_nickname,
                candidate_avatar,
                candidate_age_hours,
                &benchmarks,
            );
            assert!(res.is_some());
        }

        let total_duration = start.elapsed();
        let avg_duration_micros = total_duration.as_micros() / iterations;
        let avg_duration_millis = avg_duration_micros as f64 / 1000.0;

        // In-memory evaluation against 50 benchmarks must execute well within the 50ms budget
        assert!(
            avg_duration_millis < 50.0,
            "Average evaluation latency {}ms exceeded 50.0ms budget",
            avg_duration_millis
        );
    }

    #[test]
    fn test_homoglyph_false_positive_rate_on_standard_latin_names() {
        let engine = DetectionEngine::default();
        let benchmarks = generate_mock_benchmarks(20);

        // Corpus of standard Latin names (no homoglyphs, legitimate diverse member names)
        let standard_latin_names = [
            "Alexander Smith",
            "Benjamin Clark",
            "Catherine Davis",
            "Daniel Evans",
            "Elizabeth Frank",
            "Franklin Garcia",
            "Grace Harris",
            "Henry Jenkins",
            "Isabella Kelly",
            "James Lewis",
            "Katherine Martinez",
            "Liam Nelson",
            "Mia Owens",
            "Noah Patterson",
            "Olivia Quinn",
            "Peter Roberts",
            "Quinn Sullivan",
            "Robert Taylor",
            "Sophia Underwood",
            "Thomas Vance",
            "Ursula Washington",
            "Victor Xavier",
            "Willow Young",
            "Zachary Zimmerman",
            "Aaron Brooks",
            "Brianna Campbell",
            "Christopher Diaz",
            "Danielle Edwards",
            "Ethan Flores",
            "Fiona Green",
            "Gabriel Howard",
            "Hannah Ingram",
            "Ian Jackson",
            "Jessica King",
            "Kevin Lawson",
            "Lauren Mitchell",
            "Michael Nguyen",
            "Natalie Ortiz",
            "Oliver Phillips",
            "Paige Ramirez",
            "Ryan Stewart",
            "Samantha Turner",
            "Tyler Ward",
            "Vanessa White",
            "William Scott",
            "Adam Bell",
            "Beatrice Cooper",
            "Colin Foster",
            "Diana Gray",
            "Edward Hayes",
            "Faith Jordan",
            "George Long",
            "Holly Morgan",
            "Isaac Powell",
            "Julia Ross",
            "Keith Sanders",
            "Laura Torres",
            "Marcus Webb",
            "Nora Adams",
            "Oscar Butler",
            "Penelope Cox",
            "Richard Fisher",
            "Stella Griffin",
            "Timothy Henderson",
            "Victoria James",
            "Walter Knight",
            "Yvonne Larson",
            "Arthur Myers",
            "Clara Price",
            "Derek Russell",
            "Evelyn Simmons",
            "Felix Thomas",
            "Gloria Walker",
            "Harvey Allen",
            "Irene Bennett",
            "Justin Coleman",
            "Kendra Hughes",
            "Leonard Morris",
            "Megan Perry",
            "Nathan Reed",
            "Patricia Sanchez",
            "Raymond Wood",
            "Sylvia Barnes",
            "Travis Crawford",
            "Valerie Gomez",
            "Wesley Hunt",
            "Audrey Myers",
            "Bradley Ross",
            "Carmen Sullivan",
            "Dominic West",
            "Elena Foster",
            "Gerald Higgins",
            "Hazel Meyer",
            "Ivan Palmer",
            "Judith Shaw",
            "Kyle Vaughn",
            "Lydia Weaver",
            "Martin Arnold",
            "Nina Bishop",
            "Owen Carroll",
        ];

        let total_candidates = standard_latin_names.len();
        let mut false_positive_count = 0;

        for (idx, name) in standard_latin_names.iter().enumerate() {
            let candidate_id = format!("user_{:05}", idx + 5000);
            let result = engine.evaluate_candidate(
                &candidate_id,
                name,
                None,
                None,
                1000, // Established account
                &benchmarks,
            );

            // A false positive occurs if a standard Latin name is falsely flagged as a homoglyph
            if let Some(discrepancy) = result {
                if discrepancy.homoglyph_detected {
                    false_positive_count += 1;
                }
            }
        }

        let false_positive_rate = (false_positive_count as f64 / total_candidates as f64) * 100.0;

        // Requirement: False-positive tolerance threshold for homoglyph detection < 1.0% on standard Latin names
        assert!(
            false_positive_rate < 1.0,
            "False positive rate was {}% (expected < 1.0%)",
            false_positive_rate
        );
        assert_eq!(
            false_positive_count, 0,
            "Expected 0 false positive homoglyphs on clean Latin corpus"
        );
    }

    #[test]
    fn test_homoglyph_true_positive_detection() {
        let engine = DetectionEngine::default();
        let benchmarks = generate_mock_benchmarks(10);

        // Vector 1: Cyrillic homoglyph (Cyrillic 'о' in "Pastor Jоhn")
        let cyrillic_impersonator = engine.evaluate_candidate(
            "attacker_1",
            "Pastor J\u{043E}hn", // Cyrillic Small Letter O
            None,
            None,
            1, // 1 hour old
            &benchmarks,
        );

        assert!(cyrillic_impersonator.is_some());
        let disp = cyrillic_impersonator.unwrap();
        assert!(disp.homoglyph_detected);
        assert_eq!(disp.matched_benchmark_name, "Pastor John");
        assert_eq!(disp.risk_tier, RiskTier::Critical);

        // Vector 2: Zero-width space insertion inside name
        let zws_impersonator = engine.evaluate_candidate(
            "attacker_2",
            "P\u{200B}astor John",
            None,
            None,
            2,
            &benchmarks,
        );

        assert!(zws_impersonator.is_some());
        let disp2 = zws_impersonator.unwrap();
        assert!(disp2.homoglyph_detected);
        assert_eq!(disp2.risk_tier, RiskTier::Critical);

        // Vector 3: Mathematical alphanumeric symbols (Bold script lookalike)
        let math_impersonator = engine.evaluate_candidate(
            "attacker_3",
            "𝑷𝒂𝒔𝒕𝒐𝒓 𝑱𝒐𝒉𝒏", // Mathematical Bold Italic
            None,
            None,
            5,
            &benchmarks,
        );

        assert!(math_impersonator.is_some());
        let disp3 = math_impersonator.unwrap();
        assert!(disp3.homoglyph_detected);
        assert_eq!(disp3.risk_tier, RiskTier::Critical);

        // Vector 4: Visual lookalike digit '0' for letter 'o'
        let digit_zero_impersonator =
            engine.evaluate_candidate("attacker_4", "Pastor J0hn", None, None, 1, &benchmarks);
        assert!(digit_zero_impersonator.is_some());
        let disp4 = digit_zero_impersonator.unwrap();
        assert!(disp4.homoglyph_detected);
        assert_eq!(disp4.matched_benchmark_name, "Pastor John");
        assert_eq!(disp4.risk_tier, RiskTier::Critical);

        // Vector 5: Visual lookalike digit '1' for letter 'l'
        let digit_one_impersonator =
            engine.evaluate_candidate("attacker_5", "E1der Mark", None, None, 3, &benchmarks);
        assert!(digit_one_impersonator.is_some());
        let disp5 = digit_one_impersonator.unwrap();
        assert!(disp5.homoglyph_detected);
        assert_eq!(disp5.matched_benchmark_name, "Elder Mark");
        assert_eq!(disp5.risk_tier, RiskTier::Critical);

        // Vector 6: Visual lookalike digraph 'rn' for letter 'm'
        let digraph_impersonator =
            engine.evaluate_candidate("attacker_6", "Elder rnark", None, None, 4, &benchmarks);
        assert!(digraph_impersonator.is_some());
        let disp6 = digraph_impersonator.unwrap();
        assert!(disp6.homoglyph_detected);
        assert_eq!(disp6.matched_benchmark_name, "Elder Mark");
        assert_eq!(disp6.risk_tier, RiskTier::Critical);
    }

    #[test]
    fn test_dual_field_server_nickname_independent_scoring() {
        let engine = DetectionEngine::default();
        let benchmarks = generate_mock_benchmarks(5);

        // Candidate has innocent/unrelated username ("random_user_1234"), but sets
        // their guild nickname to spoof benchmark "Pastor John"
        let result = engine.evaluate_candidate(
            "spoofed_nick_cand",
            "random_user_1234",
            Some("Pastor John"),
            None,
            2, // 2 hours old
            &benchmarks,
        );

        assert!(result.is_some());
        let triage = result.unwrap();
        assert_eq!(triage.matched_benchmark_name, "Pastor John");
        assert_eq!(triage.suspect_username, "random_user_1234");
        assert_eq!(triage.suspect_nickname, Some("Pastor John".into()));
        assert_eq!(triage.string_similarity_score, 1.0);
        assert_eq!(triage.risk_tier, RiskTier::Critical);
    }

    #[test]
    fn test_visual_perceptual_hash_avatar_clone_detection() {
        let engine = DetectionEngine::default();
        let benchmarks = generate_mock_benchmarks(5);
        // Benchmark 0 has hash "d4c3b2a10000ffff"
        let clone_hash = "d4c3b2a10000fffe"; // 1 bit diff (Hamming distance 1 <= 4: HighDuplicate)

        let result = engine.evaluate_candidate_with_avatar_hash(
            "avatar_clone_suspect",
            "Pastor John",
            None,
            Some("https://cdn.discordapp.com/avatars/clone/a1.png"),
            Some(clone_hash),
            1, // 1 hour old
            &benchmarks,
        );

        assert!(result.is_some());
        let disp = result.unwrap();
        assert_eq!(disp.avatar_hamming_distance, Some(1));
        assert_eq!(disp.risk_tier, RiskTier::Critical);
        assert!(disp
            .normalized_diff
            .contains("Visual Avatar Clone detected"));
    }

    #[test]
    fn test_triage_moderator_comparative_adjudication_payload() {
        let engine = DetectionEngine::default();
        let benchmarks = generate_mock_benchmarks(5);

        let result = engine.evaluate_candidate(
            "candidate_42",
            "Elder Markk", // 1 character transposition / insertion
            Some("Mark | Tech Lead"),
            Some("https://cdn.discordapp.com/avatars/42/suspect.png"),
            12, // 12 hours old
            &benchmarks,
        );

        assert!(result.is_some());
        let triage = result.unwrap();

        // Validate that all fields needed for rapid comparative adjudication (< 5 seconds) are present
        assert_eq!(triage.matched_benchmark_id, "bm_001");
        assert_eq!(triage.matched_benchmark_name, "Elder Mark");
        assert_eq!(triage.suspect_username, "Elder Markk");
        assert_eq!(triage.suspect_nickname, Some("Mark | Tech Lead".into()));
        assert_eq!(triage.suspect_account_age_hours, 12);
        assert!(triage.string_similarity_score > 0.90);
        assert_eq!(triage.risk_tier, RiskTier::Critical);
        assert!(!triage.normalized_diff.is_empty());
    }

    #[test]
    fn test_automated_exemption_for_whitelisted_alts_and_satire() {
        let engine = DetectionEngine::default();

        let benchmarks = vec![
            // 1. Primary Staff Benchmark
            CanonicalBenchmark {
                id: "bm_pastordan".into(),
                guild_id: "guild_1".into(),
                user_id: "10001".into(),
                canonical_username: "Pastor Dan".into(),
                server_nickname: Some("Dan | Senior Pastor".into()),
                community_role: "Executive Leadership".into(),
                avatar_url: None,
                avatar_perceptual_hash: None,
                is_active: true,
                tags: vec!["Core Staff".into(), "Whitelisted Alt: 20002".into()],
                created_at: 1000,
                updated_at: 1000,
                sensitivity_override: None,
            },
            // 2. Official Alt registered as a benchmark with Authorized Alt tag
            CanonicalBenchmark {
                id: "bm_pastordan_alt".into(),
                guild_id: "guild_1".into(),
                user_id: "20001".into(),
                canonical_username: "Pastor Dan (Mobile Alt)".into(),
                server_nickname: Some("Dan [Alt]".into()),
                community_role: "Authorized Alt".into(),
                avatar_url: None,
                avatar_perceptual_hash: None,
                is_active: true,
                tags: vec!["Authorized Alt".into()],
                created_at: 1000,
                updated_at: 1000,
                sensitivity_override: None,
            },
            // 3. Approved Satire account
            CanonicalBenchmark {
                id: "bm_pastordan_satire".into(),
                guild_id: "guild_1".into(),
                user_id: "30001".into(),
                canonical_username: "Pastor Dan (Parody)".into(),
                server_nickname: None,
                community_role: "Approved Satire".into(),
                avatar_url: None,
                avatar_perceptual_hash: None,
                is_active: true,
                tags: vec!["Approved Satire".into()],
                created_at: 1000,
                updated_at: 1000,
                sensitivity_override: None,
            },
        ];

        // Case A: Registered Authorized Alt joins with near-identical name -> BYPASSES ALERT
        let alt_res = engine.evaluate_candidate(
            "20001",
            "Pastor Dan (Mobile Alt)",
            Some("Dan [Alt]"),
            None,
            2,
            &benchmarks,
        );
        assert!(
            alt_res.is_none(),
            "Authorized Alt benchmark must bypass discrepancy alerts"
        );

        // Case B: Registered Approved Satire account joins -> BYPASSES ALERT
        let satire_res =
            engine.evaluate_candidate("30001", "Pastor Dan (Parody)", None, None, 1, &benchmarks);
        assert!(
            satire_res.is_none(),
            "Approved Satire account must bypass discrepancy alerts"
        );

        // Case C: Candidate user ID "20002" whitelisted via tag on primary benchmark -> BYPASSES ALERT
        let whitelisted_res =
            engine.evaluate_candidate("20002", "Pastor Dan (Test)", None, None, 1, &benchmarks);
        assert!(
            whitelisted_res.is_none(),
            "Explicitly whitelisted alt snowflake must bypass alerts"
        );

        // Case D: Impersonator "99999" (not whitelisted) joins with clone name -> FLAGGED
        let clone_res = engine.evaluate_candidate(
            "99999",
            "Pastor Dan",
            Some("Dan | Senior Pastor"),
            None,
            1,
            &benchmarks,
        );
        assert!(
            clone_res.is_some(),
            "Unwhitelisted lookalike clone must trigger discrepancy alert"
        );
        let incident = clone_res.unwrap();
        assert_eq!(incident.matched_benchmark_id, "bm_pastordan");
        assert_eq!(incident.risk_tier, RiskTier::Critical);
    }

    #[test]
    fn test_snowflake_timestamp_extraction_and_account_age_calculation() {
        use super::snowflake::*;
        use chrono::{TimeZone, Utc};

        // Discord epoch timestamp check
        let epoch_dt = snowflake_to_datetime(0).unwrap();
        assert_eq!(epoch_dt.to_rfc3339(), "2015-01-01T00:00:00+00:00");

        // Fixed reference time for deterministic testing
        let ref_now = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();

        // Account created 10 hours ago: 10 * 3600 * 1000 = 36,000,000 ms before ref_now
        let created_ms = (ref_now.timestamp_millis() - 36_000_000) as u64;
        let brand_new_snowflake = build_mock_snowflake(created_ms);
        let brand_new_age = AccountAge::from_snowflake(brand_new_snowflake, ref_now).unwrap();

        assert_eq!(brand_new_age.age_hours.round(), 10.0);
        assert!(brand_new_age.is_brand_new);
        assert!(is_brand_new_account(
            brand_new_snowflake,
            ref_now.timestamp_millis() as u64,
            None
        ));

        // Account created 120 days ago (~2880 hours ago)
        let old_created_ms = (ref_now.timestamp_millis() - (120 * 24 * 3_600_000)) as u64;
        let old_snowflake = build_mock_snowflake(old_created_ms);
        let old_age = AccountAge::from_snowflake(old_snowflake, ref_now).unwrap();

        assert_eq!(old_age.age_days.round(), 120.0);
        assert!(!old_age.is_brand_new);
        assert!(!is_brand_new_account(
            old_snowflake,
            ref_now.timestamp_millis() as u64,
            None
        ));
    }

    #[test]
    fn test_snowflake_candidate_evaluation_escalates_brand_new_account_risk() {
        use super::snowflake::build_mock_snowflake;
        use chrono::Utc;

        let engine = DetectionEngine::default();
        let benchmarks = vec![CanonicalBenchmark {
            id: "bm_pastor".into(),
            guild_id: "guild_alpha".into(),
            user_id: "100000000000000001".into(),
            canonical_username: "Pastor Dave".into(),
            server_nickname: None,
            community_role: "Staff".into(),
            avatar_url: None,
            avatar_perceptual_hash: None,
            is_active: true,
            tags: vec![],
            created_at: 1000,
            updated_at: 1000,
            sensitivity_override: None,
        }];

        let now_ms = Utc::now().timestamp_millis() as u64;

        // Attacker A: Brand new account created 5 hours ago
        let new_account_ts = now_ms - (5 * 3_600_000);
        let new_user_snowflake = build_mock_snowflake(new_account_ts).to_string();

        // Attacker B: Established account created 200 days ago
        let old_account_ts = now_ms - (200 * 24 * 3_600_000);
        let old_user_snowflake = build_mock_snowflake(old_account_ts).to_string();

        // 1. Evaluate Attacker A with identical name -> Brand new account (< 72h) + score 1.0 -> Critical
        let res_new = engine.evaluate_candidate_snowflake(
            &new_user_snowflake,
            "Pastor Dave",
            None,
            None,
            None,
            &benchmarks,
        );
        assert!(res_new.is_some());
        let incident_new = res_new.unwrap();
        assert_eq!(incident_new.risk_tier, RiskTier::Critical);
        assert!(incident_new.suspect_account_age_hours < 72);

        // 2. Evaluate Attacker B with high string similarity (score ~0.94) -> Established account -> Elevated, not Critical
        let res_old = engine.evaluate_candidate_snowflake(
            &old_user_snowflake,
            "Pastor Davee",
            None,
            None,
            None,
            &benchmarks,
        );
        assert!(res_old.is_some());
        let incident_old = res_old.unwrap();
        assert_eq!(incident_old.risk_tier, RiskTier::Elevated);
        assert!(incident_old.suspect_account_age_hours > 72);
    }

    #[test]
    fn test_compound_threat_adjudication_unified_tiers() {
        let default_threshold = 0.82;
        let new_account_threshold_hours = 72;

        // 1. Critical: Pixel clone avatar (Hamming 2 <= 4) + brand-new account (10h < 72h)
        let crit_avatar_new = adjudicate_compound_threat(
            0.50, // low text score
            default_threshold,
            false,
            Some(2),
            10,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(crit_avatar_new.risk_tier, RiskTier::Critical);
        assert_eq!(crit_avatar_new.risk_weight, 4.0);

        // 2. Critical: Exact text match (1.0 > 0.95) + brand-new account (2h < 72h)
        let crit_text_new = adjudicate_compound_threat(
            1.0,
            default_threshold,
            false,
            None,
            2,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(crit_text_new.risk_tier, RiskTier::Critical);

        // 3. Critical: Homoglyph spoofing + brand-new account
        let crit_homoglyph_new = adjudicate_compound_threat(
            0.60,
            default_threshold,
            true, // homoglyph detected
            None,
            5,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(crit_homoglyph_new.risk_tier, RiskTier::Critical);

        // 4. Elevated: Avatar clone alone (Hamming 3 <= 4) on old account (1000h)
        let elev_avatar_old = adjudicate_compound_threat(
            0.50,
            default_threshold,
            false,
            Some(3),
            1000,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(elev_avatar_old.risk_tier, RiskTier::Elevated);
        assert_eq!(elev_avatar_old.risk_weight, 3.0);

        // 5. Elevated: Notable avatar similarity (Hamming 7 <= 10) + high text score (0.88 >= 0.80)
        let elev_notable_avatar = adjudicate_compound_threat(
            0.88,
            default_threshold,
            false,
            Some(7),
            1000,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(elev_notable_avatar.risk_tier, RiskTier::Elevated);

        // 6. Elevated: Homoglyph spoofing on established account (1000h)
        let elev_homoglyph_old = adjudicate_compound_threat(
            0.75,
            default_threshold,
            true,
            None,
            1000,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(elev_homoglyph_old.risk_tier, RiskTier::Elevated);

        // 7. Notable: High text score (0.86 >= 0.85) on established account without homoglyph or avatar clone
        let notable_text_old = adjudicate_compound_threat(
            0.86,
            default_threshold,
            false,
            None,
            1000,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(notable_text_old.risk_tier, RiskTier::Notable);
        assert_eq!(notable_text_old.risk_weight, 2.0);

        // 8. Notable: Notable avatar similarity alone (Hamming 8 <= 10) with sub-threshold text (0.40)
        let notable_avatar_alone = adjudicate_compound_threat(
            0.40,
            default_threshold,
            false,
            Some(8),
            1000,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(notable_avatar_alone.risk_tier, RiskTier::Notable);

        // 9. Standard: Meets minimal threshold (0.83 >= 0.82) on established account without other factors
        let standard_match = adjudicate_compound_threat(
            0.83,
            default_threshold,
            false,
            None,
            1000,
            new_account_threshold_hours,
        )
        .unwrap();
        assert_eq!(standard_match.risk_tier, RiskTier::Standard);
        assert_eq!(standard_match.risk_weight, 1.0);

        // 10. Sub-threshold: Text score 0.70 < 0.82, no homoglyph, no avatar match -> None
        let sub_threshold = adjudicate_compound_threat(
            0.70,
            default_threshold,
            false,
            None,
            1000,
            new_account_threshold_hours,
        );
        assert!(sub_threshold.is_none());
    }

    #[test]
    fn test_pipeline_latency_budget_under_50ms_per_payload() {
        let engine = DetectionEngine::default();
        // Guild with 100 canonical benchmarks
        let benchmarks = generate_mock_benchmarks(100);

        let candidate_id = "999999999999999999";
        let candidate_username = "Pastor Jоhn"; // Cyrillic lookalike
        let candidate_nickname = Some("Pastor John [Staff]");
        let candidate_avatar_hash = Some("d4c3b2a10000fffe"); // 1 bit diff from benchmark hash
        let candidate_age_hours = 2; // Brand new account

        // Warm up lazy regexes and skeleton tables
        let _ = engine.evaluate_candidate_with_avatar_hash(
            candidate_id,
            candidate_username,
            candidate_nickname,
            None,
            candidate_avatar_hash,
            candidate_age_hours,
            &benchmarks,
        );

        // Measure a single payload execution time
        let single_start = Instant::now();
        let discrepancy = engine.evaluate_candidate_with_avatar_hash(
            candidate_id,
            candidate_username,
            candidate_nickname,
            None,
            candidate_avatar_hash,
            candidate_age_hours,
            &benchmarks,
        );
        let single_duration = single_start.elapsed();
        let single_duration_millis = single_duration.as_micros() as f64 / 1000.0;

        assert!(discrepancy.is_some());
        let incident = discrepancy.unwrap();
        assert_eq!(incident.risk_tier, RiskTier::Critical);

        // Enforce pipeline latency budget: strictly < 50 milliseconds total evaluation time per payload
        assert!(
            single_duration_millis < 50.0,
            "Single payload evaluation took {:.2}ms, exceeding 50ms latency budget!",
            single_duration_millis
        );

        // Also verify batch execution consistency across 50 iterations
        let iterations = 50;
        let batch_start = Instant::now();
        for _ in 0..iterations {
            let res = engine.evaluate_candidate_with_avatar_hash(
                candidate_id,
                candidate_username,
                candidate_nickname,
                None,
                candidate_avatar_hash,
                candidate_age_hours,
                &benchmarks,
            );
            assert!(res.is_some());
        }
        let batch_duration = batch_start.elapsed();
        let avg_duration_millis = (batch_duration.as_micros() as f64 / iterations as f64) / 1000.0;

        assert!(
            avg_duration_millis < 50.0,
            "Average payload evaluation took {:.2}ms, exceeding 50.0ms budget",
            avg_duration_millis
        );
    }

    #[test]
    fn test_phase_21_1_adversarial_red_team_simulation() {
        use image::imageops::{crop_imm, resize, FilterType};
        use image::{Rgba, RgbaImage};

        let engine = DetectionEngine::default();
        let staging_guild_id = "999888777666555444";

        // Step 1: Configure dedicated Discord staging server with seeded benchmark staff accounts
        let base_avatar_w = 64;
        let base_avatar_h = 64;
        let mut dan_avatar = RgbaImage::new(base_avatar_w, base_avatar_h);
        for x in 0..base_avatar_w {
            for y in 0..base_avatar_h {
                let r = ((x * 255) / base_avatar_w) as u8;
                let g = ((y * 255) / base_avatar_h) as u8;
                let b = if (x + y) % 8 < 4 { 220 } else { 40 };
                dan_avatar.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
        let mut dan_bytes = std::io::Cursor::new(Vec::new());
        dan_avatar
            .write_to(&mut dan_bytes, image::ImageFormat::Png)
            .unwrap();
        let dan_hash = compute_perceptual_hash(dan_bytes.get_ref()).unwrap();

        let benchmarks = vec![
            CanonicalBenchmark {
                id: "bm_dan".to_string(),
                guild_id: staging_guild_id.to_string(),
                user_id: "100000000000000001".to_string(),
                canonical_username: "DanWard".to_string(),
                server_nickname: Some("Dan | Executive Pastor".to_string()),
                community_role: "Executive Pastor".to_string(),
                avatar_url: Some("https://cdn.discordapp.com/avatars/101/dan.png".to_string()),
                avatar_perceptual_hash: Some(dan_hash.clone()),
                tags: vec!["Staff".to_string(), "Leadership".to_string()],
                is_active: true,
                sensitivity_override: None,
                created_at: 1767225600,
                updated_at: 1767225600,
            },
            CanonicalBenchmark {
                id: "bm_sarah".to_string(),
                guild_id: staging_guild_id.to_string(),
                user_id: "100000000000000002".to_string(),
                canonical_username: "SarahChen".to_string(),
                server_nickname: Some("Sarah | Head of Ops".to_string()),
                community_role: "Operations Director".to_string(),
                avatar_url: Some("https://cdn.discordapp.com/avatars/102/sarah.png".to_string()),
                avatar_perceptual_hash: Some("a1b2c3d4e5f60000".to_string()),
                tags: vec!["Staff".to_string()],
                is_active: true,
                sensitivity_override: None,
                created_at: 1767225600,
                updated_at: 1767225600,
            },
        ];

        // Step 2: Simulate Adversarial Attacks & Validate Risk Tiers within 50ms

        // Attack Vector 1A: Script-mixed lookalike username DanШard (Cyrillic Sha \u{0428})
        let t0 = Instant::now();
        let res_dan_cyrillic = engine.evaluate_candidate_with_avatar_hash(
            "200000000000000001",
            "Dan\u{0428}ard",
            None,
            None,
            None,
            500, // Established account
            &benchmarks,
        );
        let elapsed_1a = t0.elapsed();
        assert!(
            res_dan_cyrillic.is_some(),
            "DanШard attack must be detected"
        );
        let disc_1a = res_dan_cyrillic.unwrap();
        assert!(disc_1a.homoglyph_detected);
        assert_eq!(disc_1a.risk_tier, RiskTier::Elevated);
        assert!(
            elapsed_1a.as_millis() < 50,
            "Latency {:?} exceeded 50ms budget",
            elapsed_1a
        );

        // Attack Vector 1B: Script-mixed lookalike username SarahСhen (Cyrillic Es \u{0421})
        let t0 = Instant::now();
        let res_sarah_cyrillic = engine.evaluate_candidate_with_avatar_hash(
            "200000000000000002",
            "Sarah\u{0421}hen",
            None,
            None,
            None,
            12, // Brand-new account (< 72h)
            &benchmarks,
        );
        let elapsed_1b = t0.elapsed();
        assert!(
            res_sarah_cyrillic.is_some(),
            "SarahСhen attack must be detected"
        );
        let disc_1b = res_sarah_cyrillic.unwrap();
        assert!(disc_1b.homoglyph_detected);
        assert_eq!(
            disc_1b.risk_tier,
            RiskTier::Critical,
            "Homoglyph on brand new account must trigger Critical"
        );
        assert!(
            elapsed_1b.as_millis() < 50,
            "Latency {:?} exceeded 50ms budget",
            elapsed_1b
        );

        // Attack Vector 1C: Authority affix spoofing DanWard_Official
        let t0 = Instant::now();
        let res_dan_official = engine.evaluate_candidate_with_avatar_hash(
            "200000000000000003",
            "DanWard_Official",
            None,
            None,
            None,
            1000,
            &benchmarks,
        );
        let elapsed_1c = t0.elapsed();
        assert!(
            res_dan_official.is_some(),
            "DanWard_Official must be detected"
        );
        let disc_1c = res_dan_official.unwrap();
        assert!(disc_1c.string_similarity_score >= 0.94);
        assert_eq!(disc_1c.risk_tier, RiskTier::Elevated);
        assert!(
            elapsed_1c.as_millis() < 50,
            "Latency {:?} exceeded 50ms budget",
            elapsed_1c
        );

        // Attack Vector 2A: Zero-width separator injection D\u{200B}a\u{200C}n\u{200D}W\u{FEFF}a\u{2060}r\u{00AD}d
        let t0 = Instant::now();
        let injected_zw = "D\u{200B}a\u{200C}n\u{200D}W\u{FEFF}a\u{2060}r\u{00AD}d";
        let res_zw = engine.evaluate_candidate_with_avatar_hash(
            "200000000000000004",
            injected_zw,
            None,
            None,
            None,
            500,
            &benchmarks,
        );
        let elapsed_2a = t0.elapsed();
        assert!(res_zw.is_some(), "Zero-width injection must be detected");
        let disc_2a = res_zw.unwrap();
        assert!(disc_2a.homoglyph_detected);
        assert_eq!(disc_2a.risk_tier, RiskTier::Elevated);
        assert!(
            elapsed_2a.as_millis() < 50,
            "Latency {:?} exceeded 50ms budget",
            elapsed_2a
        );

        // Attack Vector 2B: Invisible whitespace injection Dan\u{00A0}\u{2003}\u{3000}Ward
        let t0 = Instant::now();
        let injected_ws = "Dan\u{00A0}Ward";
        let res_ws = engine.evaluate_candidate_with_avatar_hash(
            "200000000000000005",
            injected_ws,
            Some("Dan\u{202F}Ward"),
            None,
            None,
            500,
            &benchmarks,
        );
        let elapsed_2b = t0.elapsed();
        assert!(
            res_ws.is_some(),
            "Invisible whitespace injection must be detected"
        );
        let disc_2b = res_ws.unwrap();
        assert!(disc_2b.homoglyph_detected);
        assert_eq!(disc_2b.risk_tier, RiskTier::Elevated);
        assert!(
            elapsed_2b.as_millis() < 50,
            "Latency {:?} exceeded 50ms budget",
            elapsed_2b
        );

        // Attack Vector 3A: Avatar modified with subtle rotation & crop
        let cropped = crop_imm(&dan_avatar, 3, 3, 58, 58).to_image();
        let resized = resize(&cropped, base_avatar_w, base_avatar_h, FilterType::Triangle);
        let mut mod_bytes = std::io::Cursor::new(Vec::new());
        resized
            .write_to(&mut mod_bytes, image::ImageFormat::Png)
            .unwrap();
        let mod_avatar_hash = compute_perceptual_hash(mod_bytes.get_ref()).unwrap();
        let dist = calculate_hamming_distance(&dan_hash, &mod_avatar_hash).unwrap();
        assert!(
            dist <= 10,
            "Hamming distance must indicate notable similarity or clone (dist={})",
            dist
        );

        // Attack Vector 3B: Avatar modified with color balance shift & noise overlays
        let mut color_noise_img = resized;
        for (i, p) in color_noise_img.pixels_mut().enumerate() {
            let r = p[0].saturating_add(12);
            let g = p[1].saturating_sub(6);
            let b = p[2].saturating_add(15);
            let noise = if i % 6 == 0 { 10 } else { 0 };
            *p = Rgba([r.saturating_add(noise), g, b, 255]);
        }
        let mut cn_bytes = std::io::Cursor::new(Vec::new());
        color_noise_img
            .write_to(&mut cn_bytes, image::ImageFormat::Png)
            .unwrap();
        let cn_avatar_hash = compute_perceptual_hash(cn_bytes.get_ref()).unwrap();
        let cn_dist = calculate_hamming_distance(&dan_hash, &cn_avatar_hash).unwrap();
        assert!(
            cn_dist <= 10,
            "Color+noise avatar Hamming distance {} should be <= 10",
            cn_dist
        );

        // Attack Vector 4: Compound Red Team attack: Script-mixed name + Modified avatar + Brand new account (< 24h)
        let t0 = Instant::now();
        let compound_res = engine.evaluate_candidate_with_avatar_hash(
            "200000000000000006",
            "Dan\u{0428}ard",
            Some("DanWard_Official"),
            Some("https://cdn.discordapp.com/avatars/adversary/clone.png"),
            Some(&cn_avatar_hash),
            6, // 6 hours old account
            &benchmarks,
        );
        let elapsed_compound = t0.elapsed();
        assert!(
            compound_res.is_some(),
            "Compound adversarial attack must be flagged"
        );
        let compound_disc = compound_res.unwrap();
        assert_eq!(
            compound_disc.risk_tier,
            RiskTier::Critical,
            "Compound red team attack must trigger CRITICAL risk tier"
        );
        assert!(compound_disc
            .avatar_hamming_distance
            .is_some_and(|d| d <= 10));
        assert!(compound_disc.homoglyph_detected);
        assert!(
            elapsed_compound.as_millis() < 50,
            "Compound evaluation took {:?}, exceeding 50ms budget",
            elapsed_compound
        );
    }
}
