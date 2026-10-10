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

use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::AnimationDecoder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use thiserror::Error;

/// Errors occurring during perceptual hash calculation or comparison.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum HashError {
    #[error("Failed to decode image buffer")]
    DecodeFailed,
    #[error("Failed to parse hex hash")]
    ParseFailed,
    #[error("Image buffer exceeds maximum allowed size ({size} bytes, limit {limit} bytes)")]
    BufferTooLarge { size: usize, limit: usize },
    #[error("Image dimensions exceed allowed bounds ({width}x{height}, max dimension {max_dim}, max pixels {max_pixels})")]
    DimensionsExceeded {
        width: u32,
        height: u32,
        max_dim: u32,
        max_pixels: u64,
    },
}

/// Maximum allowed avatar byte buffer size (8 MB).
pub const MAX_AVATAR_BYTE_SIZE: usize = 8 * 1024 * 1024;

/// Maximum allowed single dimension (width or height in pixels) for avatar analysis.
pub const MAX_IMAGE_DIMENSION: u32 = 4096;

/// Maximum total allowed pixels (16 Megapixels, e.g. 4096 x 4096).
pub const MAX_TOTAL_PIXELS: u64 = 16_777_216;

/// Validates that an image's dimensions and pixel count do not exceed resource limits.
#[inline]
pub fn validate_frame_dimensions(img: &image::DynamicImage) -> Result<(), HashError> {
    let w = img.width();
    let h = img.height();
    let pixels = (w as u64) * (h as u64);
    if w > MAX_IMAGE_DIMENSION || h > MAX_IMAGE_DIMENSION || pixels > MAX_TOTAL_PIXELS {
        return Err(HashError::DimensionsExceeded {
            width: w,
            height: h,
            max_dim: MAX_IMAGE_DIMENSION,
            max_pixels: MAX_TOTAL_PIXELS,
        });
    }
    Ok(())
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

/// Parses a 16-hex string into a 64-bit integer and calculates bitwise XOR popcount.
fn calculate_single_hash_hamming(h_a: &str, h_b: &str) -> Result<u32, HashError> {
    let val_a = u64::from_str_radix(h_a.trim(), 16).map_err(|_| HashError::ParseFailed)?;
    let val_b = u64::from_str_radix(h_b.trim(), 16).map_err(|_| HashError::ParseFailed)?;
    Ok((val_a ^ val_b).count_ones())
}

/// Computes the visual Hamming distance between two frame representations.
///
/// Combines DCT perceptual hashing (pHash) with average hashing (aHash) and color histogram checks
/// to prevent false negatives from simple contrast or brightness manipulations.
fn calculate_frame_distance(f_a: &str, f_b: &str) -> Result<u32, HashError> {
    let parts_a: Vec<&str> = f_a.split(':').collect();
    let parts_b: Vec<&str> = f_b.split(':').collect();

    // Standard 64-bit hex legacy comparison
    if parts_a.len() == 1 && parts_b.len() == 1 {
        return calculate_single_hash_hamming(parts_a[0], parts_b[0]);
    }

    // pHash check (DCT frequency domain)
    let dist_p = calculate_single_hash_hamming(parts_a[0], parts_b[0])?;

    // aHash check (luminance mean threshold domain, highly invariant to contrast/brightness)
    let dist_a = if parts_a.len() >= 2 && parts_b.len() >= 2 {
        Some(calculate_single_hash_hamming(parts_a[1], parts_b[1])?)
    } else {
        None
    };

    // Color histogram check (chromatic palette domain)
    let dist_c = if parts_a.len() >= 3 && parts_b.len() >= 3 {
        Some(calculate_single_hash_hamming(parts_a[2], parts_b[2])?)
    } else {
        None
    };

    let mut min_dist = dist_p;
    if let Some(da) = dist_a {
        min_dist = min_dist.min(da);
    }
    if let Some(dc) = dist_c {
        min_dist = min_dist.min(dc);
    }

    Ok(min_dist)
}

/// Computes Hamming distance between two perceptual hash strings.
/// Supports single 64-bit hex hashes, composite pHash:aHash:color hashes, and multi-keyframe animations.
/// Lower distance indicates higher visual similarity.
pub fn calculate_hamming_distance(hash_a: &str, hash_b: &str) -> Result<u32, HashError> {
    let keyframes_a: Vec<&str> = hash_a.trim().split(';').filter(|s| !s.is_empty()).collect();
    let keyframes_b: Vec<&str> = hash_b.trim().split(';').filter(|s| !s.is_empty()).collect();

    if keyframes_a.is_empty() || keyframes_b.is_empty() {
        return Err(HashError::ParseFailed);
    }

    let mut min_dist = u32::MAX;
    for ka in &keyframes_a {
        for kb in &keyframes_b {
            let dist = calculate_frame_distance(ka, kb)?;
            if dist < min_dist {
                min_dist = dist;
            }
        }
    }

    Ok(min_dist)
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

/// Computes a 64-bit color histogram palette hash.
/// Quantizes RGB into 64 bins (4 bins/channel) and flags bins above average density.
pub fn compute_color_hash(img: &image::DynamicImage) -> String {
    let rgba = img.to_rgba8();
    let mut bins = [0u32; 64];
    let total_pixels = (rgba.width() * rgba.height()).max(1);

    for pixel in rgba.pixels() {
        let r_bin = (pixel[0] / 64).min(3) as usize;
        let g_bin = (pixel[1] / 64).min(3) as usize;
        let b_bin = (pixel[2] / 64).min(3) as usize;
        let bin = (r_bin << 4) | (g_bin << 2) | b_bin;
        bins[bin] += 1;
    }

    let threshold = total_pixels / 64;
    let mut hash_val: u64 = 0;
    for (i, count) in bins.iter().enumerate() {
        if *count > threshold {
            hash_val |= 1u64 << (63 - i);
        }
    }

    format!("{:016x}", hash_val)
}

/// Samples 2–3 keyframes from animated frames:
/// - If 1 frame: samples `[0]`
/// - If 2 frames: samples `[0, 1]`
/// - If >= 3 frames: samples `[0, N / 2, N - 1]` (start, middle, end)
///
/// Thwarts adversaries who place an innocuous first frame followed by an impersonating animation.
fn sample_keyframes(frames: Vec<image::DynamicImage>) -> Vec<image::DynamicImage> {
    let n = frames.len();
    if n <= 2 {
        frames
    } else {
        vec![
            frames[0].clone(),
            frames[n / 2].clone(),
            frames[n - 1].clone(),
        ]
    }
}

/// Maximum animation frames to inspect for keyframe extraction (prevents decompression bombs).
pub const MAX_KEYFRAME_CANDIDATE_FRAMES: usize = 30;

/// Safely extracts sampled keyframes from image bytes without writing to disk.
/// Inspects animated GIF, animated WebP, APNG, or falls back to static decoding.
fn extract_sampled_keyframes(image_bytes: &[u8]) -> Result<Vec<image::DynamicImage>, HashError> {
    if image_bytes.len() > MAX_AVATAR_BYTE_SIZE {
        return Err(HashError::BufferTooLarge {
            size: image_bytes.len(),
            limit: MAX_AVATAR_BYTE_SIZE,
        });
    }

    // 1. Animated GIF decoder (bounded frame iteration)
    if let Ok(decoder) = GifDecoder::new(Cursor::new(image_bytes)) {
        let mut sample_list = Vec::new();
        for frame in decoder
            .into_frames()
            .take(MAX_KEYFRAME_CANDIDATE_FRAMES)
            .flatten()
        {
            let img = image::DynamicImage::ImageRgba8(frame.into_buffer());
            validate_frame_dimensions(&img)?;
            sample_list.push(img);
        }
        if !sample_list.is_empty() {
            return Ok(sample_keyframes(sample_list));
        }
    }

    // 2. Animated WebP decoder (bounded frame iteration)
    if let Ok(decoder) = WebPDecoder::new(Cursor::new(image_bytes)) {
        let mut sample_list = Vec::new();
        for frame in decoder
            .into_frames()
            .take(MAX_KEYFRAME_CANDIDATE_FRAMES)
            .flatten()
        {
            let img = image::DynamicImage::ImageRgba8(frame.into_buffer());
            validate_frame_dimensions(&img)?;
            sample_list.push(img);
        }
        if !sample_list.is_empty() {
            return Ok(sample_keyframes(sample_list));
        }
    }

    // 3. APNG (Animated PNG) decoder (bounded frame iteration)
    if let Ok(decoder) = PngDecoder::new(Cursor::new(image_bytes)) {
        if let Ok(apng) = decoder.apng() {
            let mut sample_list = Vec::new();
            for frame in apng
                .into_frames()
                .take(MAX_KEYFRAME_CANDIDATE_FRAMES)
                .flatten()
            {
                let img = image::DynamicImage::ImageRgba8(frame.into_buffer());
                validate_frame_dimensions(&img)?;
                sample_list.push(img);
            }
            if !sample_list.is_empty() {
                return Ok(sample_keyframes(sample_list));
            }
        }
    }

    // 4. Standard static image decoding (PNG, JPEG, WebP, etc.)
    if let Ok(dyn_img) = image::load_from_memory(image_bytes) {
        validate_frame_dimensions(&dyn_img)?;
        return Ok(vec![dyn_img]);
    }

    // 5. Fallback bundled decoder
    if let Ok(bundled_img) = img_hash::image::load_from_memory(image_bytes) {
        let rgba = bundled_img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let raw = rgba.into_raw();
        if let Some(buf) = image::RgbaImage::from_raw(w, h, raw) {
            let dyn_img = image::DynamicImage::ImageRgba8(buf);
            validate_frame_dimensions(&dyn_img)?;
            return Ok(vec![dyn_img]);
        }
    }

    Err(HashError::DecodeFailed)
}

/// Computes the composite hash representation for a single frame:
/// `<phash_hex>:<ahash_hex>:<color_hex>`
fn compute_single_frame_hash(dyn_img: &image::DynamicImage) -> Result<String, HashError> {
    validate_frame_dimensions(dyn_img)?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let raw = rgba.into_raw();
    let img = img_hash::image::ImageBuffer::<img_hash::image::Rgba<u8>, _>::from_raw(w, h, raw)
        .ok_or(HashError::DecodeFailed)?;

    // 1. 64-bit Discrete Cosine Transform (DCT) Perceptual Hash (pHash)
    let p_hasher = img_hash::HasherConfig::new()
        .preproc_dct()
        .hash_alg(img_hash::HashAlg::Gradient)
        .hash_size(8, 8)
        .to_hasher();
    let hash_p = p_hasher.hash_image(&img);
    let bytes_p = hash_p.as_bytes();
    let val_p = if bytes_p.len() == 8 {
        u64::from_be_bytes(bytes_p.try_into().unwrap())
    } else {
        0
    };
    let phash_hex = format!("{:016x}", val_p);

    // 2. 64-bit Average Hash (aHash) - Mean luminance thresholding invariant to brightness & contrast
    let a_hasher = img_hash::HasherConfig::new()
        .hash_alg(img_hash::HashAlg::Mean)
        .hash_size(8, 8)
        .to_hasher();
    let hash_a = a_hasher.hash_image(&img);
    let bytes_a = hash_a.as_bytes();
    let val_a = if bytes_a.len() == 8 {
        u64::from_be_bytes(bytes_a.try_into().unwrap())
    } else {
        0
    };
    let ahash_hex = format!("{:016x}", val_a);

    // 3. 64-bit Color Histogram Hash
    let color_hex = compute_color_hash(dyn_img);

    Ok(format!("{}:{}:{}", phash_hex, ahash_hex, color_hex))
}

/// Decodes an image in-memory safely (no temporary files or disk writes) and calculates
/// visual perceptual hashes across 2-3 sampled keyframes using Rayon parallel worker pools.
pub fn compute_perceptual_hash(image_bytes: &[u8]) -> Result<String, HashError> {
    let keyframes = extract_sampled_keyframes(image_bytes)?;

    // Image resizing and DCT calculations execute on Rayon dedicated worker pool
    let hashes: Vec<String> = keyframes
        .into_par_iter()
        .map(|frame| compute_single_frame_hash(&frame))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(hashes.join(";"))
}

/// Computes perceptual hash asynchronously off the Tokio event loop via `spawn_blocking`.
pub async fn compute_perceptual_hash_async(image_bytes: Vec<u8>) -> Result<String, HashError> {
    tokio::task::spawn_blocking(move || compute_perceptual_hash(&image_bytes))
        .await
        .map_err(|_| HashError::DecodeFailed)?
}

/// Compares two perceptual hashes asynchronously off the Tokio event loop via `spawn_blocking`.
pub async fn compare_perceptual_hashes_async(
    hash_a: String,
    hash_b: String,
) -> Result<PerceptualHashComparison, HashError> {
    tokio::task::spawn_blocking(move || compare_perceptual_hashes(&hash_a, &hash_b))
        .await
        .map_err(|_| HashError::ParseFailed)?
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
        let parts: Vec<&str> = hash_str.split(':').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].len(), 16);
        assert_eq!(parts[1].len(), 16);
        assert_eq!(parts[2].len(), 16);
        assert!(u64::from_str_radix(parts[0], 16).is_ok());
        assert!(u64::from_str_radix(parts[1], 16).is_ok());
        assert!(u64::from_str_radix(parts[2], 16).is_ok());
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

    #[test]
    fn test_adversarial_avatar_perturbations_subtle_rotation_crop_color_noise() {
        use image::imageops::{crop_imm, resize, FilterType};
        use image::{Rgba, RgbaImage};

        // 1. Generate base 64x64 staff avatar image
        let width = 64;
        let height = 64;
        let mut base_img = RgbaImage::new(width, height);
        for x in 0..width {
            for y in 0..height {
                let r = ((x * 255) / width) as u8;
                let g = ((y * 255) / height) as u8;
                let b = if (x + y) % 8 < 4 { 200 } else { 50 };
                base_img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }

        let mut base_bytes = std::io::Cursor::new(Vec::new());
        base_img
            .write_to(&mut base_bytes, image::ImageFormat::Png)
            .unwrap();
        let base_hash = compute_perceptual_hash(base_bytes.get_ref()).unwrap();

        // 2. Subtle rotation simulation (subtle coordinate shear / 2-3 degree perturbation)
        let mut rotated_img = RgbaImage::new(width, height);
        for x in 0..width {
            for y in 0..height {
                let shift_x = (y as f64 * 0.04).round() as i32;
                let src_x = ((x as i32 - shift_x).clamp(0, width as i32 - 1)) as u32;
                rotated_img.put_pixel(x, y, *base_img.get_pixel(src_x, y));
            }
        }
        let mut rot_bytes = std::io::Cursor::new(Vec::new());
        rotated_img
            .write_to(&mut rot_bytes, image::ImageFormat::Png)
            .unwrap();
        let rot_hash = compute_perceptual_hash(rot_bytes.get_ref()).unwrap();
        let rot_dist = calculate_hamming_distance(&base_hash, &rot_hash).unwrap();
        assert!(
            rot_dist <= 10,
            "Rotated avatar Hamming distance {} should be <= 10",
            rot_dist
        );

        // 3. Slight cropping (crop 4px border: 64x64 -> 56x56 -> resized back to 64x64)
        let cropped_view = crop_imm(&base_img, 4, 4, 56, 56).to_image();
        let cropped_resized = resize(&cropped_view, width, height, FilterType::Triangle);
        let mut crop_bytes = std::io::Cursor::new(Vec::new());
        cropped_resized
            .write_to(&mut crop_bytes, image::ImageFormat::Png)
            .unwrap();
        let crop_hash = compute_perceptual_hash(crop_bytes.get_ref()).unwrap();
        let crop_dist = calculate_hamming_distance(&base_hash, &crop_hash).unwrap();
        assert!(
            crop_dist <= 10,
            "Cropped avatar Hamming distance {} should be <= 10",
            crop_dist
        );

        // 4. Color balance shifts (+15 Red, -10 Green, +20 Blue)
        let mut color_img = base_img.clone();
        for pixel in color_img.pixels_mut() {
            let r = pixel[0].saturating_add(15);
            let g = pixel[1].saturating_sub(10);
            let b = pixel[2].saturating_add(20);
            *pixel = Rgba([r, g, b, 255]);
        }
        let mut color_bytes = std::io::Cursor::new(Vec::new());
        color_img
            .write_to(&mut color_bytes, image::ImageFormat::Png)
            .unwrap();
        let color_hash = compute_perceptual_hash(color_bytes.get_ref()).unwrap();
        let color_dist = calculate_hamming_distance(&base_hash, &color_hash).unwrap();
        assert!(
            color_dist <= 8,
            "Color shifted avatar Hamming distance {} should be <= 8",
            color_dist
        );

        // 5. Noise overlays (jittering 20% of pixels with subtle noise)
        let mut noise_img = base_img.clone();
        for (i, pixel) in noise_img.pixels_mut().enumerate() {
            if i % 5 == 0 {
                let delta = if i % 2 == 0 { 20 } else { 236 };
                let r = pixel[0].wrapping_add(delta);
                let g = pixel[1].wrapping_add(delta);
                let b = pixel[2].wrapping_add(delta);
                *pixel = Rgba([r, g, b, 255]);
            }
        }
        let mut noise_bytes = std::io::Cursor::new(Vec::new());
        noise_img
            .write_to(&mut noise_bytes, image::ImageFormat::Png)
            .unwrap();
        let noise_hash = compute_perceptual_hash(noise_bytes.get_ref()).unwrap();
        let noise_dist = calculate_hamming_distance(&base_hash, &noise_hash).unwrap();
        assert!(
            noise_dist <= 8,
            "Noise overlay avatar Hamming distance {} should be <= 8",
            noise_dist
        );

        // 6. Compound perturbations: rotation + slight crop + color shift + noise
        let mut compound_img = cropped_resized;
        for (i, pixel) in compound_img.pixels_mut().enumerate() {
            let r = pixel[0].saturating_add(10);
            let g = pixel[1].saturating_sub(8);
            let b = pixel[2].saturating_add(15);
            let noise = if i % 7 == 0 { 15 } else { 0 };
            *pixel = Rgba([r.saturating_add(noise), g, b, 255]);
        }
        let mut compound_bytes = std::io::Cursor::new(Vec::new());
        compound_img
            .write_to(&mut compound_bytes, image::ImageFormat::Png)
            .unwrap();
        let compound_hash = compute_perceptual_hash(compound_bytes.get_ref()).unwrap();
        let compound_dist = calculate_hamming_distance(&base_hash, &compound_hash).unwrap();
        assert!(
            compound_dist <= 10,
            "Compound perturbed avatar Hamming distance {} should be <= 10",
            compound_dist
        );
    }

    #[test]
    fn test_animated_avatar_keyframe_sampling_catches_first_frame_evasion() {
        use image::codecs::gif::GifEncoder;
        use image::{Frame, Rgba, RgbaImage};

        let width = 64;
        let height = 64;

        // 1. Create canonical benchmark avatar
        let mut benchmark_img = RgbaImage::new(width, height);
        for x in 0..width {
            for y in 0..height {
                let r = ((x * 255) / width) as u8;
                let g = ((y * 255) / height) as u8;
                let b = if (x + y) % 8 < 4 { 220 } else { 40 };
                benchmark_img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
        let mut bm_cursor = std::io::Cursor::new(Vec::new());
        benchmark_img
            .write_to(&mut bm_cursor, image::ImageFormat::Png)
            .unwrap();
        let benchmark_hash = compute_perceptual_hash(bm_cursor.get_ref()).unwrap();

        // 2. Adversary builds animated avatar:
        // - Frame 0: Completely innocent decoy pattern (distinct checkerboard avatar)
        // - Frame 1: Impersonating benchmark clone
        // - Frame 2: Impersonating clone with minor compression artifact
        let mut decoy_frame = RgbaImage::new(width, height);
        for x in 0..width {
            for y in 0..height {
                let v = if (x / 8) % 2 == (y / 8) % 2 { 240 } else { 20 };
                decoy_frame.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }

        let clone_frame = benchmark_img.clone();

        let mut clone_artifact_frame = benchmark_img.clone();
        for (i, p) in clone_artifact_frame.pixels_mut().enumerate() {
            if i % 10 == 0 {
                *p = Rgba([p[0].saturating_add(5), p[1], p[2], 255]);
            }
        }

        // Encode multi-frame GIF in memory
        let mut gif_bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut gif_bytes);
            encoder
                .set_repeat(image::codecs::gif::Repeat::Infinite)
                .unwrap();
            encoder
                .encode_frame(Frame::new(decoy_frame.clone()))
                .unwrap();
            encoder.encode_frame(Frame::new(clone_frame)).unwrap();
            encoder
                .encode_frame(Frame::new(clone_artifact_frame))
                .unwrap();
        }

        // Compute perceptual hash for the animated avatar
        let anim_hash = compute_perceptual_hash(&gif_bytes).unwrap();

        // Multi-frame animated hash contains keyframes separated by ';'
        let keyframe_segments: Vec<&str> = anim_hash.split(';').collect();
        assert_eq!(
            keyframe_segments.len(),
            3,
            "Must sample 3 keyframes (first, middle, last)"
        );

        let frame0_dist =
            calculate_hamming_distance(keyframe_segments[0], &benchmark_hash).unwrap();
        assert!(
            frame0_dist > 10,
            "Frame 0 alone has high distance {} (innocuous decoy)",
            frame0_dist
        );

        // But overall multi-keyframe analysis checks all sampled frames and detects the clone!
        let distance = calculate_hamming_distance(&benchmark_hash, &anim_hash).unwrap();
        assert_eq!(
            distance, 0,
            "Keyframe sampling must identify the impersonating clone in keyframes"
        );
        let comparison = compare_perceptual_hashes(&benchmark_hash, &anim_hash).unwrap();
        assert_eq!(
            comparison.similarity_tier,
            VisualSimilarityTier::HighDuplicate
        );
        assert!(comparison.is_high_duplicate);
    }

    #[test]
    fn test_contrast_and_brightness_manipulation_resilience() {
        use image::{Rgba, RgbaImage};

        let width = 64;
        let height = 64;
        let mut base_img = RgbaImage::new(width, height);
        for x in 0..width {
            for y in 0..height {
                let r = ((x * 255) / width) as u8;
                let g = ((y * 255) / height) as u8;
                let b = if (x + y) % 8 < 4 { 180 } else { 60 };
                base_img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }

        let mut base_bytes = std::io::Cursor::new(Vec::new());
        base_img
            .write_to(&mut base_bytes, image::ImageFormat::Png)
            .unwrap();
        let base_hash = compute_perceptual_hash(base_bytes.get_ref()).unwrap();

        // Apply severe contrast stretching (1.8x) and brightness shift (+50)
        let mut cb_img = base_img.clone();
        for p in cb_img.pixels_mut() {
            let r = (((p[0] as f32 - 128.0) * 1.8 + 128.0) + 50.0).clamp(0.0, 255.0) as u8;
            let g = (((p[1] as f32 - 128.0) * 1.8 + 128.0) + 50.0).clamp(0.0, 255.0) as u8;
            let b = (((p[2] as f32 - 128.0) * 1.8 + 128.0) + 50.0).clamp(0.0, 255.0) as u8;
            *p = Rgba([r, g, b, 255]);
        }

        let mut cb_bytes = std::io::Cursor::new(Vec::new());
        cb_img
            .write_to(&mut cb_bytes, image::ImageFormat::Png)
            .unwrap();
        let cb_hash = compute_perceptual_hash(cb_bytes.get_ref()).unwrap();

        // Combined pHash + aHash ensures distance remains low (<= 10), preventing false negatives
        let dist = calculate_hamming_distance(&base_hash, &cb_hash).unwrap();
        assert!(
            dist <= 10,
            "Contrast & brightness shifted image Hamming distance {} must be <= 10",
            dist
        );
        let comparison = compare_perceptual_hashes(&base_hash, &cb_hash).unwrap();
        assert!(
            comparison.is_notable_similarity,
            "Must be classified as notable similarity or duplicate"
        );
    }

    #[tokio::test]
    async fn test_worker_pool_async_offloading() {
        let mut img = image::RgbImage::new(16, 16);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb([120, 200, 50]);
        }
        let mut cursor = std::io::Cursor::new(Vec::new());
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        let png_bytes = cursor.into_inner();

        // Asynchronously offloads to dedicated worker pool
        let hash_a = compute_perceptual_hash_async(png_bytes.clone())
            .await
            .unwrap();
        let hash_b = compute_perceptual_hash_async(png_bytes).await.unwrap();

        let comparison = compare_perceptual_hashes_async(hash_a, hash_b)
            .await
            .unwrap();
        assert_eq!(comparison.hamming_distance, 0);
        assert_eq!(
            comparison.similarity_tier,
            VisualSimilarityTier::HighDuplicate
        );
    }

    #[test]
    fn test_oversized_avatar_buffer_rejection() {
        let oversized = vec![0u8; MAX_AVATAR_BYTE_SIZE + 1];
        let err = compute_perceptual_hash(&oversized).unwrap_err();
        match err {
            HashError::BufferTooLarge { size, limit } => {
                assert_eq!(size, MAX_AVATAR_BYTE_SIZE + 1);
                assert_eq!(limit, MAX_AVATAR_BYTE_SIZE);
            }
            _ => panic!("Expected BufferTooLarge error, got {:?}", err),
        }
    }

    #[test]
    fn test_oversized_avatar_dimensions_rejection() {
        let img = image::DynamicImage::new_rgba8(MAX_IMAGE_DIMENSION + 1, 10);
        let err = validate_frame_dimensions(&img).unwrap_err();
        match err {
            HashError::DimensionsExceeded { width, max_dim, .. } => {
                assert_eq!(width, MAX_IMAGE_DIMENSION + 1);
                assert_eq!(max_dim, MAX_IMAGE_DIMENSION);
            }
            _ => panic!("Expected DimensionsExceeded error, got {:?}", err),
        }
    }

    #[test]
    fn test_malformed_image_buffer_safely_rejected() {
        let malformed = vec![0x89, 0x50, 0x4E, 0x47, 0x00, 0xDE, 0xAD, 0xBE, 0xEF]; // Corrupt PNG header
        let err = compute_perceptual_hash(&malformed).unwrap_err();
        assert_eq!(err, HashError::DecodeFailed);
    }
}
