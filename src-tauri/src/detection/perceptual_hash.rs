use thiserror::Error;

#[derive(Error, Debug)]
pub enum HashError {
    #[error("Failed to decode image buffer")]
    DecodeFailed,
    #[error("Failed to parse hex hash")]
    ParseFailed,
}

/// Computes Hamming distance between two 64-bit hex hash strings.
/// Lower distance indicates higher visual similarity.
pub fn calculate_hamming_distance(hash_a: &str, hash_b: &str) -> Result<u32, HashError> {
    let val_a = u64::from_str_radix(hash_a, 16).map_err(|_| HashError::ParseFailed)?;
    let val_b = u64::from_str_radix(hash_b, 16).map_err(|_| HashError::ParseFailed)?;
    Ok((val_a ^ val_b).count_ones())
}

/// Decodes an image in-memory safely and calculates a 64-bit DCT perceptual hash.
pub fn compute_perceptual_hash(image_bytes: &[u8]) -> Result<String, HashError> {
    let img = image::load_from_memory(image_bytes).map_err(|_| HashError::DecodeFailed)?;
    let hasher = img_hash::HasherConfig::new().hash_size(8, 8).to_hasher();
    let hash = hasher.hash_image(&img);
    Ok(hash.to_base64())
}
