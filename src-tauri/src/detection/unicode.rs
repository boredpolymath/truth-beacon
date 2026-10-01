use unicode_normalization::UnicodeNormalization;

/// Normalizes strings by applying compatibility decomposition, stripping zero-width characters,
/// bidirectional overrides, invisible spacing, and control codes.
pub fn normalize_and_deobfuscate(input: &str) -> String {
    let mut cleaned = String::with_capacity(input.len());

    for ch in input.nfkd() {
        // Filter out zero-width characters, control characters, and bidirectional overrides
        match ch {
            '\u{200B}'..='\u{200F}' // Zero-width space, joiner, non-joiner, marks
            | '\u{202A}'..='\u{202E}' // Bidirectional embedding/override marks
            | '\u{FEFF}'              // Zero-width no-break space (BOM)
            | '\u{0000}'..='\u{001F}' // ASCII control characters
            | '\u{007F}'..='\u{009F}' => continue,
            _ => cleaned.push(ch),
        }
    }

    // Transliterate homoglyphs (Cyrillic, Greek, mathematical lookalikes) to standard ASCII
    deunicode::deunicode(&cleaned).trim().to_lowercase()
}
