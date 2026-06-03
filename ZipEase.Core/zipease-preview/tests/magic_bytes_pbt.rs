// Feature: image-preview-plugin, Property 10: Magic byte validation correctness
//
// **Validates: Requirements 9.1, 9.2**
//
// For any supported image format, if the file header bytes match the known magic
// signature for that format's extension, validation succeeds. If the header bytes
// do not match any known signature for the claimed extension, validation fails
// with a MagicByteMismatch error.

use proptest::prelude::*;
use zipease_preview::magic_bytes::validate_magic_bytes;

/// All supported extensions and their corresponding valid magic byte headers.
/// Each entry is (extension, valid_header_prefix) where the prefix is the minimum
/// bytes needed for validation to succeed.
fn valid_headers() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("png", vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
        ("jpg", vec![0xFF, 0xD8, 0xFF]),
        ("jpeg", vec![0xFF, 0xD8, 0xFF]),
        ("gif", vec![0x47, 0x49, 0x46, 0x38]),
        ("bmp", vec![0x42, 0x4D]),
        // WebP requires RIFF at offset 0 and WEBP at offset 8
        ("webp", vec![0x52, 0x49, 0x46, 0x46, 0x00, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50]),
        ("tiff", vec![0x49, 0x49, 0x2A, 0x00]),
        ("tif", vec![0x49, 0x49, 0x2A, 0x00]),
        ("ico", vec![0x00, 0x00, 0x01, 0x00]),
    ]
}

/// All supported extensions for use in strategies.
fn supported_extensions() -> Vec<&'static str> {
    vec!["png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "ico"]
}

/// Strategy: select a random supported extension index.
fn extension_index_strategy() -> impl Strategy<Value = usize> {
    0..valid_headers().len()
}

/// Strategy: generate random trailing bytes (0 to 64 bytes) to append after magic bytes.
fn trailing_bytes_strategy() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..64)
}

/// Strategy: generate random bytes that are guaranteed NOT to match any known
/// magic signature for a given extension.
fn invalid_header_strategy() -> impl Strategy<Value = Vec<u8>> {
    // Generate 12 random bytes, then we'll verify they don't accidentally match
    proptest::collection::vec(any::<u8>(), 12..=12)
}

proptest! {
    /// Property 10a: For each supported format, valid magic bytes + random trailing
    /// bytes always pass validation.
    #[test]
    fn valid_header_with_trailing_bytes_succeeds(
        ext_idx in extension_index_strategy(),
        trailing in trailing_bytes_strategy(),
    ) {
        let headers = valid_headers();
        let (extension, magic_prefix) = &headers[ext_idx];

        // Build a full header: magic prefix + trailing random bytes
        let mut header = magic_prefix.clone();
        header.extend_from_slice(&trailing);

        // For WebP, the bytes at offset 4..8 can be anything (file size field),
        // but we need to ensure the header is at least 12 bytes
        if *extension == "webp" && header.len() < 12 {
            header.resize(12, 0x00);
            // Re-set the WEBP marker at offset 8
            header[8] = 0x57;
            header[9] = 0x45;
            header[10] = 0x42;
            header[11] = 0x50;
        }

        let result = validate_magic_bytes(&header, extension);
        prop_assert!(
            result.is_ok(),
            "Expected Ok for extension '{}' with valid magic bytes, got {:?}",
            extension,
            result
        );
    }

    /// Property 10b: Random bytes that don't match any known signature for the
    /// claimed extension fail with MagicByteMismatch.
    #[test]
    fn random_non_matching_bytes_fail(
        ext_idx in extension_index_strategy(),
        mut random_header in invalid_header_strategy(),
    ) {
        let headers = valid_headers();
        let (extension, magic_prefix) = &headers[ext_idx];

        // Ensure the random header does NOT accidentally match the valid signature.
        // Corrupt the first byte of the magic prefix position to guarantee mismatch.
        match *extension {
            "png" => {
                // PNG starts with 0x89 — make sure byte 0 is NOT 0x89
                if random_header[0] == 0x89 {
                    random_header[0] = 0x00;
                }
            }
            "jpg" | "jpeg" => {
                // JPEG starts with 0xFF 0xD8 0xFF
                if random_header[0] == 0xFF && random_header[1] == 0xD8 && random_header[2] == 0xFF {
                    random_header[0] = 0x00;
                }
            }
            "gif" => {
                // GIF starts with 0x47 0x49 0x46 0x38
                if random_header[0] == 0x47 && random_header[1] == 0x49
                    && random_header[2] == 0x46 && random_header[3] == 0x38
                {
                    random_header[0] = 0x00;
                }
            }
            "bmp" => {
                // BMP starts with 0x42 0x4D
                if random_header[0] == 0x42 && random_header[1] == 0x4D {
                    random_header[0] = 0x00;
                }
            }
            "webp" => {
                // WebP needs RIFF at 0 AND WEBP at 8
                // Corrupt offset 0 to ensure RIFF doesn't match
                if random_header[0] == 0x52 && random_header[1] == 0x49
                    && random_header[2] == 0x46 && random_header[3] == 0x46
                {
                    random_header[0] = 0x00;
                }
                // Also corrupt offset 8 to ensure WEBP doesn't match
                if random_header.len() > 11
                    && random_header[8] == 0x57 && random_header[9] == 0x45
                    && random_header[10] == 0x42 && random_header[11] == 0x50
                {
                    random_header[8] = 0x00;
                }
            }
            "tiff" | "tif" => {
                // TIFF LE: 0x49 0x49 0x2A 0x00 or TIFF BE: 0x4D 0x4D 0x00 0x2A
                let is_le = random_header[0] == 0x49 && random_header[1] == 0x49
                    && random_header[2] == 0x2A && random_header[3] == 0x00;
                let is_be = random_header[0] == 0x4D && random_header[1] == 0x4D
                    && random_header[2] == 0x00 && random_header[3] == 0x2A;
                if is_le || is_be {
                    random_header[0] = 0x00;
                }
            }
            "ico" => {
                // ICO: 0x00 0x00 0x01 0x00
                if random_header[0] == 0x00 && random_header[1] == 0x00
                    && random_header[2] == 0x01 && random_header[3] == 0x00
                {
                    random_header[2] = 0xFF;
                }
            }
            _ => {}
        }

        let result = validate_magic_bytes(&random_header, extension);
        prop_assert!(
            result.is_err(),
            "Expected MagicByteMismatch for extension '{}' with non-matching bytes {:?}",
            extension,
            &random_header[..magic_prefix.len().min(random_header.len())]
        );

        // Verify it's specifically a MagicByteMismatch error
        if let Err(e) = result {
            let is_mismatch = matches!(e, zipease_preview::error::PreviewError::MagicByteMismatch { .. });
            prop_assert!(
                is_mismatch,
                "Expected MagicByteMismatch variant, got {:?}",
                e
            );
        }
    }

    /// Property 10c: Mismatched extension + header combinations always fail.
    /// E.g., PNG header with "jpg" extension, JPEG header with "gif" extension, etc.
    #[test]
    fn mismatched_extension_header_fails(
        ext_idx in extension_index_strategy(),
        trailing in trailing_bytes_strategy(),
    ) {
        let headers = valid_headers();
        let (source_ext, magic_prefix) = &headers[ext_idx];

        // Build a valid header for source_ext
        let mut header = magic_prefix.clone();
        header.extend_from_slice(&trailing);

        // For WebP, ensure minimum 12 bytes with correct markers
        if *source_ext == "webp" && header.len() < 12 {
            header.resize(12, 0x00);
            header[8] = 0x57;
            header[9] = 0x45;
            header[10] = 0x42;
            header[11] = 0x50;
        }

        // Try validating against every OTHER extension
        let all_exts = supported_extensions();
        for target_ext in &all_exts {
            // Skip if it's the same extension or an alias (jpg/jpeg, tiff/tif)
            if is_compatible_extension(source_ext, target_ext) {
                continue;
            }

            let result = validate_magic_bytes(&header, target_ext);
            prop_assert!(
                result.is_err(),
                "Expected failure when validating {} header against '{}' extension",
                source_ext,
                target_ext
            );

            if let Err(e) = result {
                let is_mismatch = matches!(e, zipease_preview::error::PreviewError::MagicByteMismatch { .. });
                prop_assert!(
                    is_mismatch,
                    "Expected MagicByteMismatch for {} header vs '{}' ext, got {:?}",
                    source_ext,
                    target_ext,
                    e
                );
            }
        }
    }
}

/// Checks if two extensions are compatible (same format, different spelling).
fn is_compatible_extension(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    // jpg and jpeg are aliases
    if (a == "jpg" || a == "jpeg") && (b == "jpg" || b == "jpeg") {
        return true;
    }
    // tiff and tif are aliases
    if (a == "tiff" || a == "tif") && (b == "tiff" || b == "tif") {
        return true;
    }
    false
}
