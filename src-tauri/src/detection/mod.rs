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
            let name_eval =
                evaluate_string_metrics(&normalized_candidate_name, &normalized_bm_name);

            let nick_eval = if let (Some(nick), Some(bm_nick)) = (
                &normalized_candidate_nick,
                benchmark
                    .server_nickname
                    .as_ref()
                    .map(|n| normalize_and_deobfuscate(n)),
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
                        normalized_diff: format!(
                            "Candidate: '{}' -> Match: '{}'",
                            normalized_candidate_name, normalized_bm_name
                        ),
                        avatar_hamming_distance: None,
                        risk_tier,
                    });
                }
            }
        }

        highest_discrepancy
    }
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

        let iterations = 100;
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

        // In-memory evaluation against 50 benchmarks must execute in < 5 milliseconds
        // (well within the end-to-end 50ms budget for gateway + detection + IPC + UI render)
        assert!(
            avg_duration_millis < 5.0,
            "Average evaluation latency {}ms exceeded 5.0ms target",
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
}
