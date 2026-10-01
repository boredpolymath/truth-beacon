//! Visual Perceptual Hashing & Hamming Distance Engine (Phase 13.4).
//!
//! Provides memory-safe visual similarity evaluation:
//! - **In-Memory Image Loader**: Decodes raw image bytes safely via `image` crate in memory (no disk writes).
//! - **64-bit DCT Perceptual Hashing**: Computes 64-bit Discrete Cosine Transform perceptual hashes via `img_hash`.
//! - **Bitwise Hamming Distance**: Measures bit divergence between 64-bit hexadecimal hashes (`0..=64`).
//! - **Hamming Classification**:
//!   - `0..=4`: **High Duplicate** (identical or near-lossless clone).
//!   - `5..=10`: **Notable Similarity** (minor tint, crop, watermark, or compression artifact).
//!   - `> 10`: **Dissimilar** (distinct visual assets).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors occurring during perceptual hash calculation or comparison.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum HashError {
    #[error("Failed to decode image buffer")]
    DecodeFailed,
    #[error("Failed to parse hex hash")]
    ParseFailed,
}

/// Visual similarity classification based on bitwise Hamming distance between 64-bit DCT perceptual hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualSimilarityTier {
    /// Hamming distance 0..=4: High visual duplicate (clone, pixel-identical or minimal compression artifact).
    HighDuplicate,
    /// Hamming distance 5..=10: Notable visual similarity (minor crop, tint, re-encoding, or subtle watermark).
    NotableSimilarity,
    /// Hamming distance > 10: Dissimilar images (distinct visual assets).
    Dissimilar,
}

/// Comprehensive perceptual hash comparison report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerceptualHashComparison {
    /// First 64-bit hexadecimal perceptual hash.
    pub hash_a: String,
    /// Second 64-bit hexadecimal perceptual hash.
    pub hash_b: String,
    /// Exact bitwise Hamming distance in [0, 64].
    pub hamming_distance: u32,
    /// Classification tier based on the standard threshold brackets.
    pub similarity_tier: VisualSimilarityTier,
    /// True if hamming distance is within high duplicate threshold (<= 4).
    pub is_high_duplicate: bool,
    /// True if hamming distance is within notable similarity threshold (<= 10).
    pub is_notable_similarity: bool,
    /// Human-readable explanation.
    pub explanation: String,
}

/// Classifies a bitwise Hamming distance into standardized visual similarity tiers:
/// - `0..=4`: `VisualSimilarityTier::HighDuplicate`
/// - `5..=10`: `VisualSimilarityTier::NotableSimilarity`
/// - `_`: `VisualSimilarityTier::Dissimilar`
#[inline]
pub fn classify_hamming_distance(distance: u32) -> VisualSimilarityTier {
    match distance {
        0..=4 => VisualSimilarityTier::HighDuplicate,
        5..=10 => VisualSimilarityTier::NotableSimilarity,
        _ => VisualSimilarityTier::Dissimilar,
    }
}

/// Computes Hamming distance between two 64-bit hex hash strings.
/// Lower distance indicates higher visual similarity.
pub fn calculate_hamming_distance(hash_a: &str, hash_b: &str) -> Result<u32, HashError> {
    let val_a = u64::from_str_radix(hash_a.trim(), 16).map_err(|_| HashError::ParseFailed)?;
    let val_b = u64::from_str_radix(hash_b.trim(), 16).map_err(|_| HashError::ParseFailed)?;
    Ok((val_a ^ val_b).count_ones())
}

/// Compares two 64-bit perceptual hashes and returns a comprehensive [`PerceptualHashComparison`].
pub fn compare_perceptual_hashes(
    hash_a: &str,
    hash_b: &str,
) -> Result<PerceptualHashComparison, HashError> {
    let distance = calculate_hamming_distance(hash_a, hash_b)?;
    let tier = classify_hamming_distance(distance);
    let is_high_duplicate = tier == VisualSimilarityTier::HighDuplicate;
    let is_notable_similarity = matches!(
        tier,
        VisualSimilarityTier::HighDuplicate | VisualSimilarityTier::NotableSimilarity
    );

    let explanation = match tier {
        VisualSimilarityTier::HighDuplicate => {
            format!(
                "High duplicate visual clone detected: Hamming distance {} (threshold 0-4)",
                distance
            )
        }
        VisualSimilarityTier::NotableSimilarity => {
            format!(
                "Notable visual similarity detected: Hamming distance {} (threshold 5-10)",
                distance
            )
        }
        VisualSimilarityTier::Dissimilar => {
            format!(
                "Dissimilar avatar assets: Hamming distance {} (> 10)",
                distance
            )
        }
    };

    Ok(PerceptualHashComparison {
        hash_a: hash_a.trim().to_lowercase(),
        hash_b: hash_b.trim().to_lowercase(),
        hamming_distance: distance,
        similarity_tier: tier,
        is_high_duplicate,
        is_notable_similarity,
        explanation,
    })
}

/// Decodes an image in-memory safely (no temporary files or disk writes) and calculates
/// a 64-bit Discrete Cosine Transform (DCT) perceptual hash formatted as a 16-char hex string.
pub fn compute_perceptual_hash(image_bytes: &[u8]) -> Result<String, HashError> {
    // Decode via modern image crate supporting PNG, JPEG, WebP, GIF, AVIF
    let hash = if let Ok(dyn_img) = image::load_from_memory(image_bytes) {
        let rgba = dyn_img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let raw = rgba.into_raw();
        let img = img_hash::image::ImageBuffer::<img_hash::image::Rgba<u8>, _>::from_raw(w, h, raw)
            .ok_or(HashError::DecodeFailed)?;
        let hasher = img_hash::HasherConfig::new().hash_size(8, 8).to_hasher();
        hasher.hash_image(&img)
    } else {
        // Fallback to img_hash's bundled decoder
        let img =
            img_hash::image::load_from_memory(image_bytes).map_err(|_| HashError::DecodeFailed)?;
        let hasher = img_hash::HasherConfig::new().hash_size(8, 8).to_hasher();
        hasher.hash_image(&img)
    };

    let bytes = hash.as_bytes();
    if bytes.len() == 8 {
        let val = u64::from_be_bytes(bytes.try_into().unwrap());
        Ok(format!("{:016x}", val))
    } else {
        Ok(hash.to_base64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hamming_distance_identical_hashes() {
        let hash = "d4c3b2a10000ffff";
        let dist = calculate_hamming_distance(hash, hash).unwrap();
        assert_eq!(dist, 0);
    }

    #[test]
    fn test_hamming_distance_small_perturbation() {
        // Differing by 1 bit in hex: 'f' (1111) vs 'e' (1110)
        let hash_a = "d4c3b2a10000ffff";
        let hash_b = "d4c3b2a10000fffe";
        let dist = calculate_hamming_distance(hash_a, hash_b).unwrap();
        assert_eq!(dist, 1);
        assert!(dist <= 4);
    }

    #[test]
    fn test_hamming_distance_divergent_hashes() {
        let hash_a = "0000000000000000";
        let hash_b = "ffffffffffffffff";
        let dist = calculate_hamming_distance(hash_a, hash_b).unwrap();
        assert_eq!(dist, 64);
        assert!(dist > 10);
    }

    #[test]
    fn test_classify_hamming_distance_thresholds() {
        // 0-4: High duplicate
        assert_eq!(
            classify_hamming_distance(0),
            VisualSimilarityTier::HighDuplicate
        );
        assert_eq!(
            classify_hamming_distance(1),
            VisualSimilarityTier::HighDuplicate
        );
        assert_eq!(
            classify_hamming_distance(4),
            VisualSimilarityTier::HighDuplicate
        );

        // 5-10: Notable similarity
        assert_eq!(
            classify_hamming_distance(5),
            VisualSimilarityTier::NotableSimilarity
        );
        assert_eq!(
            classify_hamming_distance(7),
            VisualSimilarityTier::NotableSimilarity
        );
        assert_eq!(
            classify_hamming_distance(10),
            VisualSimilarityTier::NotableSimilarity
        );

        // > 10: Dissimilar
        assert_eq!(
            classify_hamming_distance(11),
            VisualSimilarityTier::Dissimilar
        );
        assert_eq!(
            classify_hamming_distance(20),
            VisualSimilarityTier::Dissimilar
        );
        assert_eq!(
            classify_hamming_distance(64),
            VisualSimilarityTier::Dissimilar
        );
    }

    #[test]
    fn test_compare_perceptual_hashes_report() {
        let hash_a = "d4c3b2a10000ffff";
        let hash_b = "d4c3b2a10000fffe"; // 1 bit diff

        let comparison = compare_perceptual_hashes(hash_a, hash_b).unwrap();
        assert_eq!(comparison.hamming_distance, 1);
        assert_eq!(
            comparison.similarity_tier,
            VisualSimilarityTier::HighDuplicate
        );
        assert!(comparison.is_high_duplicate);
        assert!(comparison.is_notable_similarity);
        assert!(comparison.explanation.contains("High duplicate"));

        // Notable similarity (e.g. 6 bits diff: 'f' -> '0' is 4 bits + 2 bits elsewhere)
        let hash_notable = "d4c3b2a10003fff0"; // 6 bits diff
        let notable_comp = compare_perceptual_hashes(hash_a, hash_notable).unwrap();
        assert_eq!(notable_comp.hamming_distance, 6);
        assert_eq!(
            notable_comp.similarity_tier,
            VisualSimilarityTier::NotableSimilarity
        );
        assert!(!notable_comp.is_high_duplicate);
        assert!(notable_comp.is_notable_similarity);

        // Dissimilar
        let hash_dissimilar = "00000000ffff0000";
        let dis_comp = compare_perceptual_hashes(hash_a, hash_dissimilar).unwrap();
        assert!(dis_comp.hamming_distance > 10);
        assert_eq!(dis_comp.similarity_tier, VisualSimilarityTier::Dissimilar);
        assert!(!dis_comp.is_high_duplicate);
        assert!(!dis_comp.is_notable_similarity);
    }

    #[test]
    fn test_in_memory_image_loader_no_disk_writes() {
        // Generate in-memory PNG bytes using image crate
        let mut img = image::RgbImage::new(16, 16);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let val = if (x + y) % 2 == 0 { 255 } else { 0 };
            *pixel = image::Rgb([val, val, val]);
        }
        let mut cursor = std::io::Cursor::new(Vec::new());
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        let png_bytes = cursor.into_inner();

        let hash_str = compute_perceptual_hash(&png_bytes).unwrap();
        assert_eq!(hash_str.len(), 16);
        assert!(u64::from_str_radix(&hash_str, 16).is_ok());
    }

    #[test]
    fn test_invalid_hex_handling() {
        let res = calculate_hamming_distance("invalid_hex_str", "d4c3b2a10000ffff");
        assert!(res.is_err());
    }

    #[test]
    fn test_corrupt_image_buffer_rejection() {
        // Prevents crashes or panics on corrupt image buffers (DoS mitigation)
        let corrupt_bytes = [0xde, 0xad, 0xbe, 0xef, 0x00, 0x12];
        let res = compute_perceptual_hash(&corrupt_bytes);
        assert!(res.is_err());
    }
}
