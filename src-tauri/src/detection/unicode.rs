//! Unicode Normalization & Deobfuscation Engine (Phase 13.1).
//!
//! Provides multi-stage sanitization against adversarial Unicode obfuscation attacks:
//! - **Unicode Compatibility Decomposition (NFKD)**: Resolves ligature ligatures, fullwidth forms,
//!   circled numbers, and mathematical styling into canonical decomposed base equivalents.
//! - **Zero-Width Character Eradication**: Strips invisible zero-width spaces, joiners, non-joiners,
//!   and formatting markers (`\u{200B}`-`\u{200F}`, `\u{FEFF}`, `\u{2060}`, `\u{00AD}`).
//! - **Bidirectional Override Stripping**: Eradicates directional embedding, overrides, and isolates
//!   (`\u{202A}`-`\u{202E}`, `\u{2066}`-`\u{2069}`) used in Trojan Source / visual reverse spoofing.
//! - **ASCII & C1 Control Code Stripping**: Filters out non-printable ASCII control characters
//!   (`\u{0000}`-`\u{001F}`) and C1 control characters (`\u{007F}`-`\u{009F}`).
//! - **Invisible Whitespace Normalization**: Maps non-breaking spaces (`\u{00A0}`), em/en quad/spaces
//!   (`\u{2000}`-`\u{200A}`), narrow spaces (`\u{202F}`, `\u{205F}`), and fullwidth ideographic
//!   spaces (`\u{3000}`) into standard ASCII space (`\u{0020}`), collapsing redundant whitespace.
//! - **Homoglyph Script Transliteration**: De-unicodes Cyrillic, Greek, and visually similar
//!   alphabets into Latin ASCII for cross-script discrepancy matching.

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

/// Diagnostic summary of Unicode anomalies and obfuscation vectors detected in a string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnicodeAnomalyReport {
    /// Whether any zero-width characters were discovered and stripped.
    pub has_zero_width: bool,
    /// Exact count of zero-width characters stripped.
    pub zero_width_count: usize,
    /// Whether bidirectional override or embedding markers were detected.
    pub has_bidi_override: bool,
    /// Exact count of bidirectional override characters stripped.
    pub bidi_override_count: usize,
    /// Whether ASCII or C1 control characters were detected.
    pub has_control_chars: bool,
    /// Exact count of control characters stripped.
    pub control_char_count: usize,
    /// Whether invisible or alternate Unicode whitespace was normalized.
    pub has_invisible_whitespace: bool,
    /// Exact count of invisible whitespace characters mapped.
    pub invisible_whitespace_count: usize,
    /// Whether combining diacritical marks or Zalgo characters were detected.
    #[serde(default)]
    pub has_combining_marks: bool,
    /// Exact count of combining marks stripped.
    #[serde(default)]
    pub combining_mark_count: usize,
    /// Whether NFKD decomposition altered the input character sequence.
    pub has_nfkd_decomposition: bool,
    /// The fully deobfuscated and normalized string result.
    pub cleaned: String,
}

/// Returns `true` if the character is a zero-width space, joiner, non-joiner, or invisible separator.
#[inline]
pub fn is_zero_width(ch: char) -> bool {
    matches!(
        ch,
        '\u{200B}'
            ..='\u{200F}' // Zero-width space, zero-width non-joiner, zero-width joiner, LRM, RLM
        | '\u{FEFF}'              // Zero-width no-break space / BOM
        | '\u{2060}'              // Word joiner
        | '\u{180E}'              // Mongolian vowel separator
        | '\u{00AD}' // Soft hyphen (used as invisible breaker)
    )
}

/// Returns `true` if the character is a bidirectional embedding, override, or isolate code.
#[inline]
pub fn is_bidi_override(ch: char) -> bool {
    matches!(
        ch,
        '\u{202A}'..='\u{202E}' // LRE, RLE, PDF, LRO, RLO
        | '\u{2066}'..='\u{2069}' // LRI, RLI, FSI, PDI
    )
}

/// Returns `true` if the character is an ASCII control code (C0) or extended C1 control code.
#[inline]
pub fn is_control_character(ch: char) -> bool {
    matches!(
        ch,
        '\u{0000}'..='\u{001F}' // C0 control codes (NUL, BEL, BS, TAB, LF, CR, ESC, etc.)
        | '\u{007F}'..='\u{009F}' // DEL and C1 control codes
    )
}

/// Returns `true` if the character is an alternate/invisible Unicode whitespace character.
#[inline]
pub fn is_invisible_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{00A0}'              // Non-breaking space
        | '\u{2000}'
            ..='\u{200A}' // En quad, em quad, en space, em space, 3-per-em, 4-per-em, 6-per-em, fig space, punct space, thin space, hair space
        | '\u{202F}'              // Narrow no-break space
        | '\u{205F}'              // Medium mathematical space
        | '\u{3000}' // Ideographic (CJK) fullwidth space
    )
}

/// Returns `true` if the character is a combining diacritical mark or symbol mark (Zalgo vectors).
#[inline]
pub fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036F}' // Combining Diacritical Marks
        | '\u{1AB0}'..='\u{1AFF}' // Combining Diacritical Marks Extended
        | '\u{1DC0}'..='\u{1DFF}' // Combining Diacritical Marks Supplement
        | '\u{20D0}'..='\u{20FF}' // Combining Diacritical Marks for Symbols
        | '\u{FE20}'..='\u{FE2F}' // Combining Half Marks
    )
}

/// Sanitizes a single Unicode character:
/// - Returns `None` if the character should be stripped (zero-width, bidi, control, combining marks).
/// - Returns `Some(' ')` if the character is an invisible whitespace character.
/// - Returns `Some(ch)` if the character is a legitimate character.
#[inline]
pub fn sanitize_char(ch: char) -> Option<char> {
    if is_zero_width(ch)
        || is_bidi_override(ch)
        || is_control_character(ch)
        || is_combining_mark(ch)
    {
        None
    } else if is_invisible_whitespace(ch) {
        Some(' ')
    } else {
        Some(ch)
    }
}

/// Performs forensic inspection on the given string, identifying all obfuscation vectors
/// and returning a detailed [`UnicodeAnomalyReport`].
pub fn inspect_unicode_anomalies(input: &str) -> UnicodeAnomalyReport {
    let mut zero_width_count = 0;
    let mut bidi_override_count = 0;
    let mut control_char_count = 0;
    let mut invisible_whitespace_count = 0;
    let mut combining_mark_count = 0;

    for ch in input.chars() {
        if is_zero_width(ch) {
            zero_width_count += 1;
        } else if is_bidi_override(ch) {
            bidi_override_count += 1;
        } else if is_control_character(ch) {
            control_char_count += 1;
        } else if is_invisible_whitespace(ch) {
            invisible_whitespace_count += 1;
        } else if is_combining_mark(ch) {
            combining_mark_count += 1;
        }
    }

    let nfkd_str: String = input.nfkd().collect();
    let has_nfkd_decomposition = nfkd_str != input;
    let final_cleaned = strip_deobfuscate_unicode(input);

    UnicodeAnomalyReport {
        has_zero_width: zero_width_count > 0,
        zero_width_count,
        has_bidi_override: bidi_override_count > 0,
        bidi_override_count,
        has_control_chars: control_char_count > 0,
        control_char_count,
        has_invisible_whitespace: invisible_whitespace_count > 0,
        invisible_whitespace_count,
        has_combining_marks: combining_mark_count > 0,
        combining_mark_count,
        has_nfkd_decomposition,
        cleaned: final_cleaned,
    }
}

/// Applies NFKD decomposition, strips zero-width characters, bidirectional overrides,
/// and control characters, while normalizing invisible whitespace into standard single spaces.
pub fn strip_deobfuscate_unicode(input: &str) -> String {
    let mut cleaned = String::with_capacity(input.len());
    let mut last_was_space = false;

    for ch in input.nfkd() {
        match sanitize_char(ch) {
            None => continue,
            Some(' ') => {
                if !last_was_space {
                    cleaned.push(' ');
                    last_was_space = true;
                }
            }
            Some(c) => {
                cleaned.push(c);
                last_was_space = false;
            }
        }
    }

    cleaned.trim().to_string()
}

/// Normalizes strings by applying compatibility decomposition (NFKD), stripping zero-width characters,
/// bidirectional overrides, ASCII control codes, normalizing invisible whitespace, and transliterating
/// homoglyphs to lowercase Latin ASCII.
///
/// This serves as the primary text preprocessing pipeline for username and nickname impersonation checks.
pub fn normalize_and_deobfuscate(input: &str) -> String {
    let stripped = strip_deobfuscate_unicode(input);
    // Transliterate homoglyphs (Cyrillic, Greek, mathematical lookalikes) to standard ASCII
    deunicode::deunicode(&stripped).trim().to_lowercase()
}

/// Generates canonical visual skeletons using standard Unicode Technical Report #39 (TR39 / UTS #39)
/// confusable data tables (via `unicode_security::confusable_detection::skeleton`).
///
/// Strips invisible/zero-width/bidi artifacts first, decomposes characters according to TR39,
/// and normalizes case/whitespace.
pub fn tr39_canonical_skeleton(input: &str) -> String {
    let stripped = strip_deobfuscate_unicode(input);
    let skel: String = unicode_security::confusable_detection::skeleton(&stripped).collect();
    skel.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nfkd_decomposition() {
        // Fullwidth Latin characters: 'Ａｄｍｉｎ' -> 'Admin'
        let fullwidth = "Ａｄｍｉｎ";
        assert_eq!(strip_deobfuscate_unicode(fullwidth), "Admin");
        assert_eq!(normalize_and_deobfuscate(fullwidth), "admin");

        // Circled numbers: '①②③' -> '123'
        let circled = "①②③";
        assert_eq!(strip_deobfuscate_unicode(circled), "123");
        assert_eq!(normalize_and_deobfuscate(circled), "123");

        // Ligatures: 'ﬁ' -> 'fi', 'ﬂ' -> 'fl'
        let ligature = "oﬃce ﬂag";
        assert_eq!(strip_deobfuscate_unicode(ligature), "office flag");
        assert_eq!(normalize_and_deobfuscate(ligature), "office flag");

        // Mathematical bold italic: '𝑷𝒂𝒔𝒕𝒐𝒓' -> 'Pastor'
        let math = "𝑷𝒂𝒔𝒕𝒐𝒓";
        assert_eq!(normalize_and_deobfuscate(math), "pastor");
    }

    #[test]
    fn test_strip_zero_width_characters() {
        // Zero-width space (\u{200B}) inside name
        let zws = "P\u{200B}astor John";
        assert_eq!(strip_deobfuscate_unicode(zws), "Pastor John");
        assert_eq!(normalize_and_deobfuscate(zws), "pastor john");

        // Zero-width non-joiner (\u{200C}) and joiner (\u{200D})
        let joiners = "Ad\u{200C}m\u{200D}in";
        assert_eq!(strip_deobfuscate_unicode(joiners), "Admin");
        assert_eq!(normalize_and_deobfuscate(joiners), "admin");

        // Directional marks: LRM (\u{200E}), RLM (\u{200F})
        let dir_marks = "\u{200E}Leader\u{200F}";
        assert_eq!(strip_deobfuscate_unicode(dir_marks), "Leader");

        // Byte Order Mark / Zero-width no-break space (\u{FEFF})
        let bom = "\u{FEFF}Moderator\u{FEFF}";
        assert_eq!(strip_deobfuscate_unicode(bom), "Moderator");
        assert_eq!(normalize_and_deobfuscate(bom), "moderator");

        // Word joiner (\u{2060}) and soft hyphen (\u{00AD})
        let wj = "Super\u{2060}User\u{00AD}";
        assert_eq!(strip_deobfuscate_unicode(wj), "SuperUser");
    }

    #[test]
    fn test_strip_bidirectional_overrides() {
        // Left-to-Right / Right-to-Left overrides (\u{202A} - \u{202E})
        let bidi_lro = "Admin\u{202D}Spoof\u{202C}";
        assert_eq!(strip_deobfuscate_unicode(bidi_lro), "AdminSpoof");

        let bidi_rlo = "User\u{202E}dlroW\u{202C}";
        assert_eq!(strip_deobfuscate_unicode(bidi_rlo), "UserdlroW");

        // Isolates (\u{2066} - \u{2069})
        let isolate = "Test\u{2066}Safe\u{2069}";
        assert_eq!(strip_deobfuscate_unicode(isolate), "TestSafe");
    }

    #[test]
    fn test_strip_ascii_control_characters() {
        // C0 control codes (\u{0000} - \u{001F}): NUL, BEL (\u{0007}), ESC (\u{001B})
        let control_c0 = "\u{0000}\u{0007}Official\u{001B}Staff\u{001F}";
        assert_eq!(strip_deobfuscate_unicode(control_c0), "OfficialStaff");
        assert_eq!(normalize_and_deobfuscate(control_c0), "officialstaff");

        // C1 control codes (\u{007F} - \u{009F}): DEL (\u{007F}), CSI (\u{009B})
        let control_c1 = "Sec\u{007F}urity\u{009B}";
        assert_eq!(strip_deobfuscate_unicode(control_c1), "Security");
    }

    #[test]
    fn test_normalize_invisible_whitespace() {
        // Non-breaking space (\u{00A0})
        let nbsp = "Pastor\u{00A0}John";
        assert_eq!(strip_deobfuscate_unicode(nbsp), "Pastor John");
        assert_eq!(normalize_and_deobfuscate(nbsp), "pastor john");

        // En space (\u{2002}), Em space (\u{2003}), Thin space (\u{2009})
        let en_em = "Verified\u{2002}VIP\u{2003}Member\u{2009}One";
        assert_eq!(strip_deobfuscate_unicode(en_em), "Verified VIP Member One");

        // Fullwidth ideographic space (\u{3000})
        let fullwidth_space = "Staff\u{3000}Lead";
        assert_eq!(strip_deobfuscate_unicode(fullwidth_space), "Staff Lead");
        assert_eq!(normalize_and_deobfuscate(fullwidth_space), "staff lead");

        // Multiple consecutive invisible whitespaces should collapse to a single space
        let multi_space = "Elder\u{00A0}\u{2000}\u{3000}Mark";
        assert_eq!(strip_deobfuscate_unicode(multi_space), "Elder Mark");

        // Leading and trailing invisible whitespace should be trimmed
        let padded = "\u{00A0}\u{3000}Trimmed\u{2003}";
        assert_eq!(strip_deobfuscate_unicode(padded), "Trimmed");
    }

    #[test]
    fn test_inspect_unicode_anomalies_reporting() {
        let deceptive = "\u{FEFF}\u{202E}P\u{200B}astor\u{00A0}J\u{043E}hn\u{0007}";
        let report = inspect_unicode_anomalies(deceptive);

        assert!(report.has_zero_width);
        assert_eq!(report.zero_width_count, 2); // \u{FEFF} and \u{200B}
        assert!(report.has_bidi_override);
        assert_eq!(report.bidi_override_count, 1); // \u{202E}
        assert!(report.has_invisible_whitespace);
        assert_eq!(report.invisible_whitespace_count, 1); // \u{00A0}
        assert!(report.has_control_chars);
        assert_eq!(report.control_char_count, 1); // \u{0007}
        assert_eq!(report.cleaned, "Pastor J\u{043E}hn");

        // Normal Latin text has zero anomalies
        let clean = "Pastor John";
        let clean_report = inspect_unicode_anomalies(clean);
        assert!(!clean_report.has_zero_width);
        assert!(!clean_report.has_bidi_override);
        assert!(!clean_report.has_control_chars);
        assert!(!clean_report.has_invisible_whitespace);
        assert_eq!(clean_report.cleaned, "Pastor John");
    }

    #[test]
    fn test_clean_latin_text_preserved() {
        let names = [
            "Alice Smith",
            "Bob Jones",
            "Charlie-Brown_42",
            "Tech Lead 101",
            "TruthBeacon Admin",
        ];

        for name in names {
            assert_eq!(strip_deobfuscate_unicode(name), name);
            assert_eq!(normalize_and_deobfuscate(name), name.to_lowercase());
        }
    }

    #[test]
    fn test_tr39_canonical_skeleton() {
        // Cyrillic lookalikes: 'Р' (Cyrillic Er), 'а' (Cyrillic a), 'о' (Cyrillic o) in 'Pаstоr'
        let cyrillic_pastor = "\u{0420}\u{0430}st\u{043E}r";
        assert_eq!(
            tr39_canonical_skeleton(cyrillic_pastor),
            tr39_canonical_skeleton("pastor")
        );

        // Greek lookalike: 'Αdmin' (Greek Alpha) vs Latin 'admin' vs digraph 'adrnin'
        let greek_admin = "\u{0391}dmin";
        assert_eq!(
            tr39_canonical_skeleton(greek_admin),
            tr39_canonical_skeleton("admin")
        );
        // TR39 canonical tables map 'm' and 'rn' to the same canonical sequence ("rn")
        assert_eq!(
            tr39_canonical_skeleton("admin"),
            tr39_canonical_skeleton("adrnin")
        );

        // Mathematical lookalikes and fullwidth characters
        let math_lead = "𝑷𝒂𝒔𝒕𝒐𝒓";
        assert_eq!(
            tr39_canonical_skeleton(math_lead),
            tr39_canonical_skeleton("pastor")
        );

        // Clean Latin names remain canonical
        assert_eq!(
            tr39_canonical_skeleton("DanWard"),
            tr39_canonical_skeleton("danward")
        );
        assert_eq!(
            tr39_canonical_skeleton("TruthBeacon"),
            tr39_canonical_skeleton("truthbeacon")
        );
    }

    #[test]
    fn test_strip_combining_marks_and_zalgo_text() {
        // Zalgo text injection with stacked combining marks
        let zalgo = "P\u{0336}\u{0353}\u{0357}astor J\u{0300}\u{0301}ohn";
        let cleaned = strip_deobfuscate_unicode(zalgo);
        assert_eq!(cleaned, "Pastor John");
        assert_eq!(normalize_and_deobfuscate(zalgo), "pastor john");

        let report = inspect_unicode_anomalies(zalgo);
        assert!(report.has_combining_marks);
        assert_eq!(report.combining_mark_count, 5);
        assert_eq!(report.cleaned, "Pastor John");
    }
}
