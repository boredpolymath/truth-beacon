//! Homoglyph & Script Mapping Engine (Phase 13.2).
//!
//! Provides advanced detection for deceptive visual impersonation attacks:
//! - **Script Transliteration**: De-unicodes Cyrillic, Greek, Cherokee, and mathematical alphanumeric
//!   symbols (fraktur, double-struck, bold italic) into canonical Latin ASCII.
//! - **Specialized Visual Lookalike Dictionary**: Maps confusable single characters and digraphs
//!   into canonical visual skeletons (`0` <-> `O`, `1` <-> `l` <-> `I`, `rn` <-> `m`, `vv` <-> `w`, `cl` <-> `d`).
//! - **Exact Match Detection Post-Normalization**: Accurately flags pure homoglyph and visual confusable
//!   spoofing while preventing false positives on legitimate Latin names.

use crate::detection::unicode::normalize_and_deobfuscate;
use serde::{Deserialize, Serialize};

/// Classification of homoglyph spoofing vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HomoglyphCategory {
    /// No homoglyph or confusable spoofing detected.
    None,
    /// Cross-script transliteration homoglyph (Cyrillic, Greek, mathematical lookalikes).
    ScriptTransliteration,
    /// Visual lookalike / confusable dictionary match (digits for letters, digraphs like `rn` for `m`).
    VisualConfusable,
    /// Compound spoofing employing both cross-script characters and visual confusables.
    Compound,
}

/// Comprehensive evaluation result for candidate vs target benchmark homoglyph analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomoglyphMatchResult {
    /// Whether an exact homoglyph or visual confusable match was detected.
    pub is_homoglyph_match: bool,
    /// Categorization of the spoofing technique used.
    pub category: HomoglyphCategory,
    /// Original raw candidate string.
    pub candidate_raw: String,
    /// Original raw target benchmark string.
    pub target_raw: String,
    /// Candidate string after Unicode deobfuscation and transliteration.
    pub candidate_normalized: String,
    /// Target benchmark string after Unicode deobfuscation and transliteration.
    pub target_normalized: String,
    /// Candidate string transformed into canonical visual skeleton.
    pub candidate_skeleton: String,
    /// Target benchmark string transformed into canonical visual skeleton.
    pub target_skeleton: String,
    /// List of specific visual confusable substitutions detected (e.g. "0 <-> o", "rn <-> m").
    pub substitutions_detected: Vec<String>,
    /// Human-readable explanation suitable for audit logging and UI display.
    pub explanation: String,
}

impl HomoglyphMatchResult {
    /// Construct a negative match result (no homoglyph detected).
    pub fn none(candidate: &str, target: &str) -> Self {
        let norm_cand = normalize_and_deobfuscate(candidate);
        let norm_tgt = normalize_and_deobfuscate(target);
        let skel_cand = to_visual_skeleton(&norm_cand);
        let skel_tgt = to_visual_skeleton(&norm_tgt);

        Self {
            is_homoglyph_match: false,
            category: HomoglyphCategory::None,
            candidate_raw: candidate.to_string(),
            target_raw: target.to_string(),
            candidate_normalized: norm_cand,
            target_normalized: norm_tgt,
            candidate_skeleton: skel_cand,
            target_skeleton: skel_tgt,
            substitutions_detected: Vec::new(),
            explanation: "No homoglyph spoofing detected".to_string(),
        }
    }
}

/// Transforms an input string into a canonical visual skeleton by reducing digraphs
/// and mapping visually confusable characters into canonical equivalence classes.
pub fn to_visual_skeleton(input: &str) -> String {
    // 1. First normalize Unicode and lowercase
    let lower = normalize_and_deobfuscate(input);

    // 2. Reduce common visual digraphs
    // "rn" -> "m", "vv" -> "w", "cl" -> "d", "nn" -> "m", "cj" -> "g", "ol" -> "d", "lo" -> "b"
    let working = lower
        .replace("rn", "m")
        .replace("vv", "w")
        .replace("cl", "d")
        .replace("nn", "m")
        .replace("cj", "g")
        .replace("ol", "d")
        .replace("lo", "b");

    // 3. Map individual visually confusable characters to canonical representatives
    let mut skeleton = String::with_capacity(working.len());
    for ch in working.chars() {
        let mapped = match ch {
            // '0' looks like 'o'
            '0' => 'o',
            // '1', '!', '|', 'i' confusable with vertical bar / lowercase 'l'
            '1' | '!' | '|' | 'i' => 'l',
            // '3' looks like 'e'
            '3' => 'e',
            // '4', '@' look like 'a'
            '4' | '@' => 'a',
            // '5', '$' look like 's'
            '5' | '$' => 's',
            // '8' looks like 'b'
            '8' => 'b',
            // '+' looks like 't'
            '+' => 't',
            _ => ch,
        };
        skeleton.push(mapped);
    }

    // Collapse multiple spaces
    let mut collapsed = String::with_capacity(skeleton.len());
    let mut last_was_space = false;
    for ch in skeleton.chars() {
        if ch == ' ' {
            if !last_was_space {
                collapsed.push(' ');
                last_was_space = true;
            }
        } else {
            collapsed.push(ch);
            last_was_space = false;
        }
    }

    collapsed.trim().to_string()
}

/// Identifies which specific visual substitutions exist between candidate and target.
pub fn detect_visual_substitutions(candidate: &str, target: &str) -> Vec<String> {
    let cand_lower = candidate.to_lowercase();
    let tgt_lower = target.to_lowercase();
    let mut subs = Vec::new();

    // Check digraph lookalikes
    if cand_lower.contains("rn") && tgt_lower.contains('m') {
        subs.push("rn <-> m".to_string());
    }
    if cand_lower.contains("vv") && tgt_lower.contains('w') {
        subs.push("vv <-> w".to_string());
    }
    if cand_lower.contains("cl") && tgt_lower.contains('d') {
        subs.push("cl <-> d".to_string());
    }
    if cand_lower.contains("nn") && tgt_lower.contains('m') {
        subs.push("nn <-> m".to_string());
    }
    if cand_lower.contains("cj") && tgt_lower.contains('g') {
        subs.push("cj <-> g".to_string());
    }

    // Check single character lookalikes
    if cand_lower.contains('0') && tgt_lower.contains('o') {
        subs.push("0 <-> o".to_string());
    }
    if (cand_lower.contains('1')
        || cand_lower.contains('|')
        || cand_lower.contains('!')
        || (cand_lower.contains('i') && tgt_lower.contains('l'))
        || (cand_lower.contains('l') && tgt_lower.contains('i')))
        && (tgt_lower.contains('l') || tgt_lower.contains('i'))
    {
        subs.push("1/|/!/I <-> l/I".to_string());
    }
    if cand_lower.contains('3') && tgt_lower.contains('e') {
        subs.push("3 <-> e".to_string());
    }
    if (cand_lower.contains('4') || cand_lower.contains('@')) && tgt_lower.contains('a') {
        subs.push("4/@ <-> a".to_string());
    }
    if (cand_lower.contains('5') || cand_lower.contains('$')) && tgt_lower.contains('s') {
        subs.push("5/$ <-> s".to_string());
    }
    if cand_lower.contains('8') && tgt_lower.contains('b') {
        subs.push("8 <-> b".to_string());
    }
    if cand_lower.contains('+') && tgt_lower.contains('t') {
        subs.push("+ <-> t".to_string());
    }

    // Check cross-script lookalikes (Cyrillic / Greek / Mathematical symbols)
    let has_cross_script = candidate.chars().any(|c| {
        ('\u{0400}'..='\u{04FF}').contains(&c) // Cyrillic
            || ('\u{0370}'..='\u{03FF}').contains(&c) // Greek
            || ('\u{1D400}'..='\u{1D7FF}').contains(&c) // Mathematical Alphanumeric
    });
    if has_cross_script {
        subs.push("Cross-script (Cyrillic/Greek/Math)".to_string());
    }

    subs
}

/// Evaluates whether a candidate username or nickname is an exact post-normalization
/// homoglyph or visual confusable spoof of a target benchmark identity.
pub fn evaluate_homoglyph_spoof(candidate: &str, target: &str) -> HomoglyphMatchResult {
    let cand_norm = normalize_and_deobfuscate(candidate);
    let tgt_norm = normalize_and_deobfuscate(target);
    let cand_skel = to_visual_skeleton(&cand_norm);
    let tgt_skel = to_visual_skeleton(&tgt_norm);

    evaluate_homoglyph_spoof_with_precomputed(
        candidate, target, &cand_norm, &tgt_norm, &cand_skel, &tgt_skel,
    )
}

/// Evaluates homoglyph spoofing with precomputed normalized and skeleton strings
/// for ultra-low latency in batch evaluation loops.
pub fn evaluate_homoglyph_spoof_with_precomputed(
    candidate: &str,
    target: &str,
    cand_norm: &str,
    tgt_norm: &str,
    cand_skel: &str,
    tgt_skel: &str,
) -> HomoglyphMatchResult {
    // If literal strings match exactly (case-insensitive), this is an identical name, not homoglyph spoofing
    if candidate.trim().eq_ignore_ascii_case(target.trim()) {
        return HomoglyphMatchResult::none(candidate, target);
    }

    // 1. Check Script Transliteration Match:
    // Candidate differs from target in raw form, but after Unicode deobfuscation and script transliteration
    // (e.g. Cyrillic/Greek/Math lookalikes), they are strictly identical.
    let script_match = cand_norm == tgt_norm && candidate != cand_norm;

    // 2. Check Visual Confusable Skeleton Match:
    // When digraphs and leetspeak / font lookalikes are reduced to canonical skeleton,
    // they match identically.
    let skeleton_match = cand_skel == tgt_skel && !cand_skel.is_empty();

    // Fast path: If neither matches, exit immediately without expensive substitution scans
    if !script_match && !skeleton_match {
        return HomoglyphMatchResult::none(candidate, target);
    }

    let mut subs = detect_visual_substitutions(candidate, target);

    if script_match && !subs.iter().any(|s| s.contains("Cross-script")) {
        subs.push("Cross-script (Cyrillic/Greek/Math)".to_string());
    }

    let has_script = subs.iter().any(|s| s.contains("Cross-script")) || script_match;
    let has_visual = subs.iter().any(|s| !s.contains("Cross-script"));

    let category = if has_script && has_visual {
        HomoglyphCategory::Compound
    } else if has_script {
        HomoglyphCategory::ScriptTransliteration
    } else {
        HomoglyphCategory::VisualConfusable
    };

    let explanation = if !subs.is_empty() {
        format!(
            "Homoglyph spoof detected ({:?}): '{}' -> '{}' [substitutions: {}]",
            category,
            candidate,
            target,
            subs.join(", ")
        )
    } else {
        format!(
            "Homoglyph spoof detected ({:?}): '{}' -> '{}'",
            category, candidate, target
        )
    };

    HomoglyphMatchResult {
        is_homoglyph_match: true,
        category,
        candidate_raw: candidate.to_string(),
        target_raw: target.to_string(),
        candidate_normalized: cand_norm.to_string(),
        target_normalized: tgt_norm.to_string(),
        candidate_skeleton: cand_skel.to_string(),
        target_skeleton: tgt_skel.to_string(),
        substitutions_detected: subs,
        explanation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_transliteration_cyrillic() {
        // Cyrillic Small Letter O (\u{043E})
        let candidate = "Pastor J\u{043E}hn";
        let target = "Pastor John";

        let result = evaluate_homoglyph_spoof(candidate, target);
        assert!(result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::ScriptTransliteration);
        assert_eq!(result.candidate_normalized, "pastor john");
        assert_eq!(result.target_normalized, "pastor john");
    }

    #[test]
    fn test_script_transliteration_greek() {
        // Greek Small Letter Alpha (\u{03B1})
        let candidate = "St\u{03B1}ff Lead";
        let target = "Staff Lead";

        let result = evaluate_homoglyph_spoof(candidate, target);
        assert!(result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::ScriptTransliteration);
        assert_eq!(result.candidate_normalized, "staff lead");
    }

    #[test]
    fn test_script_transliteration_mathematical() {
        // Mathematical Bold Italic
        let candidate = "𝑷𝒂𝒔𝒕𝒐𝒓 𝑱𝒐𝒉𝒏";
        let target = "Pastor John";

        let result = evaluate_homoglyph_spoof(candidate, target);
        assert!(result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::ScriptTransliteration);
        assert_eq!(result.candidate_normalized, "pastor john");
    }

    #[test]
    fn test_visual_lookalike_digit_zero_for_o() {
        let candidate = "Pastor J0hn";
        let target = "Pastor John";

        let result = evaluate_homoglyph_spoof(candidate, target);
        assert!(result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::VisualConfusable);
        assert!(result
            .substitutions_detected
            .contains(&"0 <-> o".to_string()));
    }

    #[test]
    fn test_visual_lookalike_one_and_vertical_for_l() {
        // '1' for 'l'
        let candidate_digit = "E1der John";
        let target = "Elder John";

        let result1 = evaluate_homoglyph_spoof(candidate_digit, target);
        assert!(result1.is_homoglyph_match);
        assert_eq!(result1.category, HomoglyphCategory::VisualConfusable);

        // Uppercase 'I' for 'l' in "Elder" -> "EIdel" or "Eider"
        let candidate_i = "EIder John";
        let result2 = evaluate_homoglyph_spoof(candidate_i, target);
        assert!(result2.is_homoglyph_match);
        assert_eq!(result2.category, HomoglyphCategory::VisualConfusable);
    }

    #[test]
    fn test_visual_lookalike_digraph_rn_for_m() {
        let candidate = "Pastor rnark";
        let target = "Pastor mark";

        let result = evaluate_homoglyph_spoof(candidate, target);
        assert!(result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::VisualConfusable);
        assert!(result
            .substitutions_detected
            .contains(&"rn <-> m".to_string()));
    }

    #[test]
    fn test_visual_lookalike_digraph_vv_for_w() {
        let candidate = "vvumpus";
        let target = "wumpus";

        let result = evaluate_homoglyph_spoof(candidate, target);
        assert!(result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::VisualConfusable);
        assert!(result
            .substitutions_detected
            .contains(&"vv <-> w".to_string()));
    }

    #[test]
    fn test_visual_lookalike_digraph_cl_for_d() {
        let candidate = "Discorcl Moclerator";
        let target = "Discord Moderator";

        let result = evaluate_homoglyph_spoof(candidate, target);
        assert!(result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::VisualConfusable);
        assert!(result
            .substitutions_detected
            .contains(&"cl <-> d".to_string()));
    }

    #[test]
    fn test_compound_spoofing() {
        // Cyrillic 'о' AND digit '1' for 'l'
        let candidate = "E1der J\u{043E}hn";
        let target = "Elder John";

        let result = evaluate_homoglyph_spoof(candidate, target);
        assert!(result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::Compound);
    }

    #[test]
    fn test_clean_latin_names_no_false_positives() {
        let pairs = [
            ("Alexander Smith", "Pastor John"),
            ("Benjamin Clark", "Elder Mark"),
            ("Catherine Davis", "Staff Leader"),
            ("Daniel Evans", "Discord Moderator"),
            ("Elizabeth Frank", "TruthBeacon Operator"),
        ];

        for (cand, target) in pairs {
            let result = evaluate_homoglyph_spoof(cand, target);
            assert!(
                !result.is_homoglyph_match,
                "Expected no homoglyph match for {} vs {}",
                cand, target
            );
            assert_eq!(result.category, HomoglyphCategory::None);
        }
    }

    #[test]
    fn test_literal_identical_names_not_flagged_as_homoglyphs() {
        let result = evaluate_homoglyph_spoof("Pastor John", "Pastor John");
        assert!(!result.is_homoglyph_match);
        assert_eq!(result.category, HomoglyphCategory::None);

        let result_case = evaluate_homoglyph_spoof("pastor john", "Pastor John");
        assert!(!result_case.is_homoglyph_match);
        assert_eq!(result_case.category, HomoglyphCategory::None);
    }
}
