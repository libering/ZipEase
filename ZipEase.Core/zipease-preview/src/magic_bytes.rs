// Magic byte validation — implemented in task 3.1
//
// Validates that file header bytes match the known magic signature
// for the claimed file extension. This prevents extension-spoofing attacks
// where a malicious file is renamed to a supported image extension.

use crate::error::PreviewError;

/// A known magic byte signature for an image format.
pub struct MagicSignature {
    /// The file extension this signature corresponds to (lowercase, no dot).
    pub extension: &'static str,
    /// Byte offset where the signature starts in the file header.
    pub offset: usize,
    /// The expected byte sequence at the given offset.
    pub bytes: &'static [u8],
}

/// Known magic byte signatures for all supported image formats.
///
/// Some formats (like WebP) require checking multiple offsets — they have
/// multiple entries in this table and ALL must match for validation to pass.
pub static SIGNATURES: &[MagicSignature] = &[
    // PNG: 8-byte signature
    MagicSignature {
        extension: "png",
        offset: 0,
        bytes: &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
    },
    // JPEG: starts with FF D8 FF
    MagicSignature {
        extension: "jpg",
        offset: 0,
        bytes: &[0xFF, 0xD8, 0xFF],
    },
    MagicSignature {
        extension: "jpeg",
        offset: 0,
        bytes: &[0xFF, 0xD8, 0xFF],
    },
    // GIF: "GIF8" (covers GIF87a and GIF89a)
    MagicSignature {
        extension: "gif",
        offset: 0,
        bytes: &[0x47, 0x49, 0x46, 0x38],
    },
    // BMP: "BM"
    MagicSignature {
        extension: "bmp",
        offset: 0,
        bytes: &[0x42, 0x4D],
    },
    // WebP: RIFF at offset 0
    MagicSignature {
        extension: "webp",
        offset: 0,
        bytes: &[0x52, 0x49, 0x46, 0x46],
    },
    // WebP: WEBP at offset 8
    MagicSignature {
        extension: "webp_offset8",
        offset: 8,
        bytes: &[0x57, 0x45, 0x42, 0x50],
    },
    // TIFF Little-Endian: "II" + 0x2A 0x00
    MagicSignature {
        extension: "tiff",
        offset: 0,
        bytes: &[0x49, 0x49, 0x2A, 0x00],
    },
    MagicSignature {
        extension: "tif",
        offset: 0,
        bytes: &[0x49, 0x49, 0x2A, 0x00],
    },
    // TIFF Big-Endian: "MM" + 0x00 0x2A
    MagicSignature {
        extension: "tiff_be",
        offset: 0,
        bytes: &[0x4D, 0x4D, 0x00, 0x2A],
    },
    MagicSignature {
        extension: "tif_be",
        offset: 0,
        bytes: &[0x4D, 0x4D, 0x00, 0x2A],
    },
    // ICO: 0x00 0x00 0x01 0x00
    MagicSignature {
        extension: "ico",
        offset: 0,
        bytes: &[0x00, 0x00, 0x01, 0x00],
    },
];

/// Validates that the given file header bytes match the known magic signature
/// for the claimed extension.
///
/// # Arguments
/// * `header` - The first 12 (or more) bytes of the file
/// * `claimed_extension` - The file extension claimed by the archive entry (without dot)
///
/// # Returns
/// * `Ok(())` if the header matches a known signature for the claimed extension
/// * `Err(PreviewError::MagicByteMismatch)` if no match is found
pub fn validate_magic_bytes(header: &[u8], claimed_extension: &str) -> Result<(), PreviewError> {
    let ext_lower = claimed_extension.to_ascii_lowercase();

    // WebP is special: requires BOTH offset-0 (RIFF) and offset-8 (WEBP) to match
    if ext_lower == "webp" {
        return validate_webp(header);
    }

    // TIFF/TIF: can be either little-endian or big-endian
    if ext_lower == "tiff" || ext_lower == "tif" {
        return validate_tiff(header, &ext_lower);
    }

    // For all other formats, find the matching signature(s) for this extension
    let matching_sigs: Vec<&MagicSignature> = SIGNATURES
        .iter()
        .filter(|sig| sig.extension == ext_lower)
        .collect();

    if matching_sigs.is_empty() {
        return Err(PreviewError::MagicByteMismatch {
            expected: ext_lower.clone(),
            actual: describe_header(header),
        });
    }

    // Check if any matching signature validates
    for sig in &matching_sigs {
        if check_signature(header, sig) {
            return Ok(());
        }
    }

    Err(PreviewError::MagicByteMismatch {
        expected: ext_lower,
        actual: describe_header(header),
    })
}

/// Validates WebP format: requires RIFF at offset 0 AND WEBP at offset 8.
fn validate_webp(header: &[u8]) -> Result<(), PreviewError> {
    let riff_sig: &[u8] = &[0x52, 0x49, 0x46, 0x46];
    let webp_sig: &[u8] = &[0x57, 0x45, 0x42, 0x50];

    let riff_ok = header.len() >= 4 && &header[0..4] == riff_sig;
    let webp_ok = header.len() >= 12 && &header[8..12] == webp_sig;

    if riff_ok && webp_ok {
        Ok(())
    } else {
        Err(PreviewError::MagicByteMismatch {
            expected: "webp".to_string(),
            actual: describe_header(header),
        })
    }
}

/// Validates TIFF format: can be either little-endian (II 2A 00) or big-endian (MM 00 2A).
fn validate_tiff(header: &[u8], ext: &str) -> Result<(), PreviewError> {
    let tiff_le: &[u8] = &[0x49, 0x49, 0x2A, 0x00];
    let tiff_be: &[u8] = &[0x4D, 0x4D, 0x00, 0x2A];

    if header.len() >= 4 && (&header[0..4] == tiff_le || &header[0..4] == tiff_be) {
        Ok(())
    } else {
        Err(PreviewError::MagicByteMismatch {
            expected: ext.to_string(),
            actual: describe_header(header),
        })
    }
}

/// Checks if the header matches a single signature entry.
fn check_signature(header: &[u8], sig: &MagicSignature) -> bool {
    let end = sig.offset + sig.bytes.len();
    if header.len() < end {
        return false;
    }
    &header[sig.offset..end] == sig.bytes
}

/// Produces a hex description of the first few header bytes for error reporting.
fn describe_header(header: &[u8]) -> String {
    let display_len = header.len().min(8);
    header[..display_len]
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_png_header() {
        let header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "png").is_ok());
        assert!(validate_magic_bytes(&header, "PNG").is_ok());
    }

    #[test]
    fn valid_jpeg_header() {
        let header = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "jpg").is_ok());
        assert!(validate_magic_bytes(&header, "jpeg").is_ok());
    }

    #[test]
    fn valid_gif_header() {
        let header = [0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "gif").is_ok());
    }

    #[test]
    fn valid_bmp_header() {
        let header = [0x42, 0x4D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "bmp").is_ok());
    }

    #[test]
    fn valid_webp_header() {
        let header = [0x52, 0x49, 0x46, 0x46, 0x00, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50];
        assert!(validate_magic_bytes(&header, "webp").is_ok());
    }

    #[test]
    fn valid_tiff_le_header() {
        let header = [0x49, 0x49, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "tiff").is_ok());
        assert!(validate_magic_bytes(&header, "tif").is_ok());
    }

    #[test]
    fn valid_tiff_be_header() {
        let header = [0x4D, 0x4D, 0x00, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "tiff").is_ok());
        assert!(validate_magic_bytes(&header, "tif").is_ok());
    }

    #[test]
    fn valid_ico_header() {
        let header = [0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "ico").is_ok());
    }

    #[test]
    fn mismatched_extension_fails() {
        // PNG header with "jpg" extension
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00];
        let result = validate_magic_bytes(&png_header, "jpg");
        assert!(result.is_err());
        match result.unwrap_err() {
            PreviewError::MagicByteMismatch { .. } => {}
            other => panic!("Expected MagicByteMismatch, got {:?}", other),
        }
    }

    #[test]
    fn random_bytes_fail() {
        let header = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "png").is_err());
        assert!(validate_magic_bytes(&header, "jpg").is_err());
        assert!(validate_magic_bytes(&header, "gif").is_err());
    }

    #[test]
    fn empty_header_fails() {
        assert!(validate_magic_bytes(&[], "png").is_err());
        assert!(validate_magic_bytes(&[], "webp").is_err());
    }

    #[test]
    fn webp_partial_match_fails() {
        // RIFF at offset 0 but no WEBP at offset 8
        let header = [0x52, 0x49, 0x46, 0x46, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(validate_magic_bytes(&header, "webp").is_err());
    }
}
