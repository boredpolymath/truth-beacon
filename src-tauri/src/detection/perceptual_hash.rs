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
    let img =
        img_hash::image::load_from_memory(image_bytes).map_err(|_| HashError::DecodeFailed)?;
    let hasher = img_hash::HasherConfig::new().hash_size(8, 8).to_hasher();
    let hash = hasher.hash_image(&img);
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
        assert!(
            dist <= 12,
            "Small perturbation within visual clone threshold"
        );
    }

    #[test]
    fn test_hamming_distance_divergent_hashes() {
        let hash_a = "0000000000000000";
        let hash_b = "ffffffffffffffff";
        let dist = calculate_hamming_distance(hash_a, hash_b).unwrap();
        assert_eq!(dist, 64);
        assert!(
            dist > 12,
            "Completely different images exceed clone threshold"
        );
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
