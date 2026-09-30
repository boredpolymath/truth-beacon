use strsim::{damerau_levenshtein, jaro_winkler};

#[derive(Debug, Clone)]
pub struct StringSimilarityResult {
    pub jaro_winkler_score: f64,
    pub damerau_distance: usize,
    pub composite_score: f64,
}

/// Evaluates string distance between two normalized candidate identifiers
pub fn evaluate_string_metrics(candidate: &str, benchmark: &str) -> StringSimilarityResult {
    let jw = jaro_winkler(candidate, benchmark);
    let damerau = damerau_levenshtein(candidate, benchmark);

    let max_len = candidate.chars().count().max(benchmark.chars().count());
    let damerau_normalized = if max_len == 0 {
        1.0
    } else {
        1.0 - (damerau as f64 / max_len as f64)
    };

    // Composite weighted score: 70% Jaro-Winkler, 30% normalized Damerau
    let composite = (jw * 0.7) + (damerau_normalized * 0.3);

    StringSimilarityResult {
        jaro_winkler_score: jw,
        damerau_distance: damerau,
        composite_score: composite,
    }
}
