//! String Metric Evaluator Engine (Phase 13.3).
//!
//! Provides multi-vector phonetic and edit-distance string evaluation:
//! - **Jaro-Winkler Similarity**: Measures character matching with prefix weighting bonus.
//! - **Damerau-Levenshtein Edit Distance**: Measures minimum single-character operations
//!   (insertions, deletions, substitutions, and adjacent character transpositions).
//! - **Composite Weighted Metric**: Computes `(JaroWinkler * 0.70) + (NormalizedDamerau * 0.30)`.
//! - **Dual-Field Identity Evaluation**: Scores username, global display name, and server nickname
//!   independently across canonical benchmarks to detect cross-field impersonation.

use serde::{Deserialize, Serialize};
use strsim::{damerau_levenshtein, jaro_winkler};

/// Metric scores calculated between two individual string fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StringSimilarityResult {
    /// Jaro-Winkler similarity score in [0.0, 1.0] emphasizing common prefixes.
    pub jaro_winkler_score: f64,
    /// Damerau-Levenshtein distance (insertions, deletions, substitutions, transpositions).
    pub damerau_distance: usize,
    /// Normalized Damerau similarity score in [0.0, 1.0].
    pub normalized_damerau_score: f64,
    /// Composite weighted similarity: `(JaroWinkler * 0.70) + (NormalizedDamerau * 0.30)`.
    pub composite_score: f64,
}

/// Identifies which pair of identity fields was evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldMatchPair {
    UsernameVsCanonical,
    UsernameVsNickname,
    GlobalNameVsCanonical,
    GlobalNameVsNickname,
    NicknameVsCanonical,
    NicknameVsNickname,
}

/// Candidate identity fields provided for multi-field / dual-field evaluation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CandidateIdentityFields<'a> {
    /// Unique candidate username / handle.
    pub username: &'a str,
    /// Global display name (if present).
    pub global_name: Option<&'a str>,
    /// Guild-specific server nickname (if set).
    pub server_nickname: Option<&'a str>,
}

/// Benchmark identity fields tested against candidate fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BenchmarkIdentityFields<'a> {
    /// Canonical username or primary identity.
    pub canonical_username: &'a str,
    /// Guild-specific server nickname (if assigned).
    pub server_nickname: Option<&'a str>,
}

/// Comprehensive multi-field evaluation scoring each field pair independently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiFieldSimilarityResult {
    /// Candidate username vs benchmark canonical name.
    pub username_vs_canonical: StringSimilarityResult,
    /// Candidate username vs benchmark nickname (if benchmark nickname exists).
    pub username_vs_nickname: Option<StringSimilarityResult>,
    /// Candidate global name vs benchmark canonical name (if global name exists).
    pub global_vs_canonical: Option<StringSimilarityResult>,
    /// Candidate global name vs benchmark nickname (if both exist).
    pub global_vs_nickname: Option<StringSimilarityResult>,
    /// Candidate server nickname vs benchmark canonical name (if candidate nickname exists).
    pub nickname_vs_canonical: Option<StringSimilarityResult>,
    /// Candidate server nickname vs benchmark nickname (if both exist).
    pub nickname_vs_nickname: Option<StringSimilarityResult>,
    /// Highest composite score across all evaluated field pairs.
    pub top_composite_score: f64,
    /// The specific field pairing that yielded the highest composite score.
    pub top_field_pair: FieldMatchPair,
    /// The detailed string metrics corresponding to top_field_pair.
    pub top_metrics: StringSimilarityResult,
}

/// Strips common authority/impersonator spoofing affixes (e.g. `_official`, `_staff`, `_admin`, `official_`, `real_`, etc.)
pub fn strip_authority_affixes(input: &str) -> Option<&str> {
    let lower = input.trim();
    const SUFFIXES: &[&str] = &[
        "_official",
        "-official",
        ".official",
        " official",
        "official",
        "_staff",
        "-staff",
        ".staff",
        " staff",
        "staff",
        "_admin",
        "-admin",
        ".admin",
        " admin",
        "admin",
        "_mod",
        "-mod",
        ".mod",
        " mod",
        "_real",
        "-real",
        ".real",
        " real",
        "_support",
        "-support",
        ".support",
        " support",
        "_verified",
        "-verified",
        ".verified",
        " verified",
        "_bot",
        "-bot",
        ".bot",
        " bot",
    ];
    const PREFIXES: &[&str] = &[
        "official_",
        "official-",
        "official.",
        "official ",
        "real_",
        "real-",
        "real.",
        "real ",
        "staff_",
        "staff-",
        "staff.",
        "staff ",
        "admin_",
        "admin-",
        "admin.",
        "admin ",
    ];

    for &sfx in SUFFIXES {
        if let Some(stripped) = lower.strip_suffix(sfx) {
            let s = stripped.trim_end_matches(['_', '-', '.', ' ']);
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    for &pfx in PREFIXES {
        if let Some(stripped) = lower.strip_prefix(pfx) {
            let s = stripped.trim_start_matches(['_', '-', '.', ' ']);
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

/// Evaluates string distance and similarity metrics between two string fields,
/// generating canonical skeletons using standard Unicode Technical Report #39 (TR39)
/// confusable data tables before fuzzy-string comparisons, and incorporating
/// authority affix spoofing detection.
pub fn evaluate_string_metrics(candidate: &str, benchmark: &str) -> StringSimilarityResult {
    let base_res = evaluate_string_metrics_raw(candidate, benchmark);

    // Generate canonical TR39 skeletons before fuzzy-string comparisons to detect
    // cross-script homoglyphs and confusable substitutions under edit distance / similarity metrics
    let cand_skel = crate::detection::unicode::tr39_canonical_skeleton(candidate);
    let bm_skel = crate::detection::unicode::tr39_canonical_skeleton(benchmark);
    let skel_res = if cand_skel != candidate || bm_skel != benchmark {
        Some(evaluate_string_metrics_raw(&cand_skel, &bm_skel))
    } else {
        None
    };

    let best_res = if let Some(sr) = skel_res {
        if sr.composite_score > base_res.composite_score {
            sr
        } else {
            base_res
        }
    } else {
        base_res
    };

    // Check authority/impersonation affix spoofing (e.g. `DanWard_Official` vs `DanWard`)
    if let Some(stripped_cand) =
        strip_authority_affixes(candidate).or_else(|| strip_authority_affixes(&cand_skel))
    {
        let stripped_res = evaluate_string_metrics_raw(stripped_cand, benchmark);
        let stripped_skel_res = evaluate_string_metrics_raw(
            &crate::detection::unicode::tr39_canonical_skeleton(stripped_cand),
            &bm_skel,
        );
        let max_stripped_score = stripped_res
            .composite_score
            .max(stripped_skel_res.composite_score);
        if max_stripped_score >= 0.88 {
            let elevated = max_stripped_score.max(0.94);
            return StringSimilarityResult {
                jaro_winkler_score: best_res
                    .jaro_winkler_score
                    .max(stripped_res.jaro_winkler_score)
                    .max(stripped_skel_res.jaro_winkler_score),
                damerau_distance: best_res
                    .damerau_distance
                    .min(stripped_res.damerau_distance)
                    .min(stripped_skel_res.damerau_distance),
                normalized_damerau_score: best_res
                    .normalized_damerau_score
                    .max(stripped_res.normalized_damerau_score)
                    .max(stripped_skel_res.normalized_damerau_score),
                composite_score: (elevated * 10000.0).round() / 10000.0,
            };
        }
    }

    best_res
}

/// Raw string distance and similarity metric evaluator:
/// 1. Jaro-Winkler similarity: `jw` in `[0.0, 1.0]`.
/// 2. Damerau-Levenshtein edit distance: `damerau` (counting transpositions as 1 edit).
/// 3. Normalized Damerau score: `1.0 - (damerau / max_len)`.
/// 4. Composite weighted score: `(jw * 0.70) + (damerau_normalized * 0.30)`.
pub fn evaluate_string_metrics_raw(candidate: &str, benchmark: &str) -> StringSimilarityResult {
    if candidate.is_empty() && benchmark.is_empty() {
        return StringSimilarityResult {
            jaro_winkler_score: 1.0,
            damerau_distance: 0,
            normalized_damerau_score: 1.0,
            composite_score: 1.0,
        };
    }

    let jw = jaro_winkler(candidate, benchmark);
    let damerau = damerau_levenshtein(candidate, benchmark);

    let max_len = candidate.chars().count().max(benchmark.chars().count());
    let damerau_normalized = if max_len == 0 {
        1.0
    } else {
        (1.0 - (damerau as f64 / max_len as f64)).clamp(0.0, 1.0)
    };

    // Composite weighted score: 70% Jaro-Winkler, 30% normalized Damerau
    let composite = ((jw * 0.70) + (damerau_normalized * 0.30)).clamp(0.0, 1.0);

    StringSimilarityResult {
        jaro_winkler_score: (jw * 10000.0).round() / 10000.0,
        damerau_distance: damerau,
        normalized_damerau_score: (damerau_normalized * 10000.0).round() / 10000.0,
        composite_score: (composite * 10000.0).round() / 10000.0,
    }
}

/// Evaluates candidate username, global handle, and server nickname independently
/// against target benchmark identity fields, returning granular metrics and identifying
/// the top impersonation vector.
pub fn evaluate_multi_field_metrics(
    candidate: &CandidateIdentityFields,
    benchmark: &BenchmarkIdentityFields,
) -> MultiFieldSimilarityResult {
    // 1. Username vs Canonical Username
    let username_vs_canonical =
        evaluate_string_metrics(candidate.username, benchmark.canonical_username);
    let mut top_score = username_vs_canonical.composite_score;
    let mut top_pair = FieldMatchPair::UsernameVsCanonical;
    let mut top_metrics = username_vs_canonical.clone();

    // 2. Username vs Benchmark Nickname
    let username_vs_nickname = benchmark.server_nickname.map(|bm_nick| {
        let res = evaluate_string_metrics(candidate.username, bm_nick);
        if res.composite_score > top_score {
            top_score = res.composite_score;
            top_pair = FieldMatchPair::UsernameVsNickname;
            top_metrics = res.clone();
        }
        res
    });

    // 3. Global Name vs Canonical Username
    let global_vs_canonical = candidate.global_name.map(|glob| {
        let res = evaluate_string_metrics(glob, benchmark.canonical_username);
        if res.composite_score > top_score {
            top_score = res.composite_score;
            top_pair = FieldMatchPair::GlobalNameVsCanonical;
            top_metrics = res.clone();
        }
        res
    });

    // 4. Global Name vs Benchmark Nickname
    let global_vs_nickname = match (candidate.global_name, benchmark.server_nickname) {
        (Some(glob), Some(bm_nick)) => {
            let res = evaluate_string_metrics(glob, bm_nick);
            if res.composite_score > top_score {
                top_score = res.composite_score;
                top_pair = FieldMatchPair::GlobalNameVsNickname;
                top_metrics = res.clone();
            }
            Some(res)
        }
        _ => None,
    };

    // 5. Server Nickname vs Canonical Username
    let nickname_vs_canonical = candidate.server_nickname.map(|cand_nick| {
        let res = evaluate_string_metrics(cand_nick, benchmark.canonical_username);
        if res.composite_score > top_score {
            top_score = res.composite_score;
            top_pair = FieldMatchPair::NicknameVsCanonical;
            top_metrics = res.clone();
        }
        res
    });

    // 6. Server Nickname vs Benchmark Nickname
    let nickname_vs_nickname = match (candidate.server_nickname, benchmark.server_nickname) {
        (Some(cand_nick), Some(bm_nick)) => {
            let res = evaluate_string_metrics(cand_nick, bm_nick);
            if res.composite_score > top_score {
                top_score = res.composite_score;
                top_pair = FieldMatchPair::NicknameVsNickname;
                top_metrics = res.clone();
            }
            Some(res)
        }
        _ => None,
    };

    MultiFieldSimilarityResult {
        username_vs_canonical,
        username_vs_nickname,
        global_vs_canonical,
        global_vs_nickname,
        nickname_vs_canonical,
        nickname_vs_nickname,
        top_composite_score: top_score,
        top_field_pair: top_pair,
        top_metrics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaro_winkler_prefix_emphasis() {
        // Shared prefix vs shared suffix
        let with_prefix = evaluate_string_metrics("PastorJohn", "PastorJahn");
        let with_suffix = evaluate_string_metrics("JohnPastor", "JahnPastor");

        // Jaro-Winkler gives a bonus for common prefixes up to 4 chars
        assert!(
            with_prefix.jaro_winkler_score >= with_suffix.jaro_winkler_score,
            "Common prefix J-W score ({}) should be >= common suffix J-W score ({})",
            with_prefix.jaro_winkler_score,
            with_suffix.jaro_winkler_score
        );
        assert!(with_prefix.composite_score >= with_suffix.composite_score);
    }

    #[test]
    fn test_damerau_levenshtein_transposition() {
        // Damerau-Levenshtein treats adjacent character swap as distance 1 (not 2)
        let swap = evaluate_string_metrics_raw("elder", "edler");
        assert_eq!(swap.damerau_distance, 1);
        assert_eq!(swap.normalized_damerau_score, 0.8); // 1.0 - (1 / 5)

        let swap2 = evaluate_string_metrics_raw("admin", "admni");
        assert_eq!(swap2.damerau_distance, 1);
        assert_eq!(swap2.normalized_damerau_score, 0.8);
    }

    #[test]
    fn test_damerau_levenshtein_insertion_and_deletion() {
        // Insertion of single character
        let insert = evaluate_string_metrics_raw("mark", "markk");
        assert_eq!(insert.damerau_distance, 1);
        assert_eq!(insert.normalized_damerau_score, 0.8); // 1.0 - (1 / 5)

        // Deletion of single character
        let delete = evaluate_string_metrics_raw("pastor", "pstor");
        assert_eq!(delete.damerau_distance, 1);
        assert_eq!(delete.normalized_damerau_score, 0.8333);
    }

    #[test]
    fn test_composite_score_formula_adherence() {
        let cand = "PastorMark";
        let bm = "PastorMarkk";

        let metrics = evaluate_string_metrics(cand, bm);
        let expected =
            (metrics.jaro_winkler_score * 0.70) + (metrics.normalized_damerau_score * 0.30);
        let diff = (metrics.composite_score - expected).abs();

        assert!(
            diff < 0.0005,
            "Composite score ({}) should match formula (0.70*JW + 0.30*Damerau = {})",
            metrics.composite_score,
            expected
        );
    }

    #[test]
    fn test_dual_field_evaluation_username_vs_canonical() {
        let cand = CandidateIdentityFields {
            username: "elder_markk",
            global_name: None,
            server_nickname: None,
        };
        let bm = BenchmarkIdentityFields {
            canonical_username: "elder_mark",
            server_nickname: Some("Elder Mark [Lead]"),
        };

        let result = evaluate_multi_field_metrics(&cand, &bm);
        assert_eq!(result.top_field_pair, FieldMatchPair::UsernameVsCanonical);
        assert!(result.top_composite_score > 0.90);
        assert_eq!(result.username_vs_canonical.damerau_distance, 1);
    }

    #[test]
    fn test_dual_field_evaluation_nickname_vs_canonical() {
        // Candidate has unrelated username ("random_user_99") but spoofed nickname ("Pastor John")
        let cand = CandidateIdentityFields {
            username: "random_user_99",
            global_name: Some("Regular User"),
            server_nickname: Some("Pastor John"),
        };
        let bm = BenchmarkIdentityFields {
            canonical_username: "Pastor John",
            server_nickname: None,
        };

        let result = evaluate_multi_field_metrics(&cand, &bm);
        assert_eq!(result.top_field_pair, FieldMatchPair::NicknameVsCanonical);
        assert_eq!(result.top_composite_score, 1.0);
        assert_eq!(result.top_metrics.damerau_distance, 0);
    }

    #[test]
    fn test_dual_field_evaluation_nickname_vs_nickname() {
        let cand = CandidateIdentityFields {
            username: "user_a",
            global_name: None,
            server_nickname: Some("Tech Lead Mark"),
        };
        let bm = BenchmarkIdentityFields {
            canonical_username: "Mark",
            server_nickname: Some("Tech Lead Mark"),
        };

        let result = evaluate_multi_field_metrics(&cand, &bm);
        assert_eq!(result.top_field_pair, FieldMatchPair::NicknameVsNickname);
        assert_eq!(result.top_composite_score, 1.0);
    }

    #[test]
    fn test_dual_field_evaluation_global_name_matching() {
        let cand = CandidateIdentityFields {
            username: "different_handle",
            global_name: Some("Community Lead Rachel"),
            server_nickname: None,
        };
        let bm = BenchmarkIdentityFields {
            canonical_username: "Community Lead Rachel",
            server_nickname: None,
        };

        let result = evaluate_multi_field_metrics(&cand, &bm);
        assert_eq!(result.top_field_pair, FieldMatchPair::GlobalNameVsCanonical);
        assert_eq!(result.top_composite_score, 1.0);
    }

    #[test]
    fn test_empty_and_identical_strings() {
        let identical = evaluate_string_metrics("truthbeacon", "truthbeacon");
        assert_eq!(identical.composite_score, 1.0);
        assert_eq!(identical.damerau_distance, 0);

        let empty = evaluate_string_metrics("", "");
        assert_eq!(empty.composite_score, 1.0);
        assert_eq!(empty.damerau_distance, 0);
    }

    #[test]
    fn test_adversarial_authority_affix_spoofing() {
        // DanWard_Official vs DanWard (should trigger high composite score >= 0.94)
        let res = evaluate_string_metrics("danward_official", "danward");
        assert!(res.composite_score >= 0.94);

        // OfficialDanWard vs DanWard
        let res_prefix = evaluate_string_metrics("official_danward", "danward");
        assert!(res_prefix.composite_score >= 0.94);

        // Unrelated string ending with _official should not be boosted
        let unrelated = evaluate_string_metrics("randomgaming_official", "danward");
        assert!(unrelated.composite_score < 0.50);
    }

    #[test]
    fn test_tr39_skeleton_fuzzy_comparison() {
        // Cyrillic lookalike: 'DаnWard' (Cyrillic а) vs 'DanWard'
        let cyrillic_cand = "D\u{0430}nWard";
        let res_exact = evaluate_string_metrics(cyrillic_cand, "DanWard");
        assert_eq!(
            res_exact.damerau_distance, 0,
            "TR39 skeleton canonicalizes Cyrillic lookalike to distance 0"
        );
        assert_eq!(res_exact.composite_score, 1.0);

        // Cyrillic lookalike + 1-character typo/insertion: 'DаnWarrd' vs 'DanWard'
        let cyrillic_typo = "D\u{0430}nWarrd";
        let res_typo = evaluate_string_metrics(cyrillic_typo, "DanWard");
        assert_eq!(
            res_typo.damerau_distance, 1,
            "TR39 skeleton resolves confusable first, so distance reflects only the single edit"
        );
        assert!(res_typo.composite_score > 0.90);

        // Cross-script confusable with authority affix: 'DаnWard_Official' vs 'DanWard'
        let affix_confusable = "D\u{0430}nWard_Official";
        let res_affix = evaluate_string_metrics(affix_confusable, "DanWard");
        assert!(res_affix.composite_score >= 0.94);
    }
}
