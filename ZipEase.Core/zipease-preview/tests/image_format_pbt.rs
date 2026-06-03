// Feature: image-preview-plugin, Property 1: Extension classification correctness
//
// Validates: Requirements 1.1, 1.2, 1.3, 1.4
//
// Property: For any archive entry with any filename string and any is_directory flag,
// `is_previewable` returns true if and only if:
//   1. The entry is not a directory
//   2. The filename contains at least one '.' character
//   3. The substring after the last '.' (compared case-insensitively) is one of the
//      supported extensions: png, jpg, jpeg, gif, bmp, webp, tiff, tif, ico

use proptest::prelude::*;
use zipease_preview::image_format::{is_previewable, SUPPORTED_EXTENSIONS};

/// Oracle function: independently computes the expected result of `is_previewable`.
fn expected_is_previewable(entry_name: &str, is_directory: bool) -> bool {
    if is_directory {
        return false;
    }

    let dot_pos = match entry_name.rfind('.') {
        Some(pos) => pos,
        None => return false,
    };

    let extension = &entry_name[dot_pos + 1..];
    if extension.is_empty() {
        return false;
    }

    let ext_lower = extension.to_ascii_lowercase();
    SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str())
}

/// Strategy that generates arbitrary filenames — a mix of random strings,
/// paths with directories, and filenames with various extensions.
fn arbitrary_filename() -> impl Strategy<Value = String> {
    prop_oneof![
        // Completely random ASCII strings (may or may not contain dots)
        "[a-zA-Z0-9_./\\\\\\- ]{0,50}",
        // Filename with a random extension
        ("[a-zA-Z0-9_]{1,20}", "[a-zA-Z0-9]{0,10}").prop_map(|(name, ext)| {
            format!("{}.{}", name, ext)
        }),
        // Filename with a supported extension (various cases)
        ("[a-zA-Z0-9_/]{1,30}", prop::sample::select(SUPPORTED_EXTENSIONS))
            .prop_map(|(name, ext)| {
                format!("{}.{}", name, ext)
            }),
        // Filename with a supported extension in random case
        ("[a-zA-Z0-9_/]{1,30}", prop::sample::select(SUPPORTED_EXTENSIONS))
            .prop_map(|(name, ext)| {
                let mixed: String = ext.chars().enumerate().map(|(i, c)| {
                    if i % 2 == 0 { c.to_ascii_uppercase() } else { c }
                }).collect();
                format!("{}.{}", name, mixed)
            }),
        // Multiple dots in filename
        ("[a-zA-Z0-9_]{1,10}", "[a-zA-Z0-9]{1,5}", "[a-zA-Z0-9]{1,5}")
            .prop_map(|(a, b, c)| format!("{}.{}.{}", a, b, c)),
        // Path-like names with directory separators
        "[a-zA-Z0-9_]{1,10}/[a-zA-Z0-9_]{1,10}\\.[a-zA-Z]{1,5}"
            .prop_map(|s| s),
        // Edge cases: trailing dot, dot-only, hidden files
        prop_oneof![
            Just(".".to_string()),
            Just("..".to_string()),
            Just("".to_string()),
            "[a-zA-Z0-9_]{1,10}\\.".prop_map(|s| s),
            "\\.[a-zA-Z0-9]{1,10}".prop_map(|s| s),
        ],
    ]
}

proptest! {
    /// **Validates: Requirements 1.1, 1.2, 1.3, 1.4**
    ///
    /// For any filename and is_directory flag, `is_previewable` matches the oracle.
    #[test]
    fn prop_extension_classification_correctness(
        filename in arbitrary_filename(),
        is_dir in any::<bool>(),
    ) {
        let actual = is_previewable(&filename, is_dir);
        let expected = expected_is_previewable(&filename, is_dir);
        prop_assert_eq!(
            actual, expected,
            "Mismatch for entry_name={:?}, is_directory={}: got {}, expected {}",
            filename, is_dir, actual, expected
        );
    }

    /// Supplementary property: directories are NEVER previewable regardless of name.
    #[test]
    fn prop_directories_never_previewable(
        filename in "[a-zA-Z0-9_./ ]{0,50}",
    ) {
        prop_assert!(!is_previewable(&filename, true),
            "Directory entry {:?} should never be previewable", filename);
    }

    /// Supplementary property: filenames without any '.' are NEVER previewable.
    #[test]
    fn prop_no_dot_never_previewable(
        filename in "[a-zA-Z0-9_/ ]{1,50}",
    ) {
        // This regex never produces a dot, so the result must be false
        prop_assert!(!is_previewable(&filename, false),
            "Filename without dot {:?} should never be previewable", filename);
    }

    /// Supplementary property: any supported extension (in any case) is previewable
    /// when the entry is not a directory and the name has a proper dot-extension.
    #[test]
    fn prop_supported_extension_always_previewable(
        base in "[a-zA-Z0-9_]{1,20}",
        ext in prop::sample::select(SUPPORTED_EXTENSIONS),
        upper_mask in prop::collection::vec(any::<bool>(), 1..=5),
    ) {
        // Apply random casing to the extension
        let mixed_ext: String = ext.chars().enumerate().map(|(i, c)| {
            if i < upper_mask.len() && upper_mask[i] {
                c.to_ascii_uppercase()
            } else {
                c
            }
        }).collect();

        let filename = format!("{}.{}", base, mixed_ext);
        prop_assert!(is_previewable(&filename, false),
            "File {:?} with supported extension should be previewable", filename);
    }
}
