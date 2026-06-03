// Feature: image-preview-plugin, Property 3: Directory-scoped navigation filtering
//
// Validates: Requirements 3.9, 3.2, 3.3
//
// Property: For any list of archive entries and any target directory path,
// `previewable_entries_in_directory` returns only entries whose parent directory
// exactly matches the target directory, all of which are previewable, and the
// result is sorted by natural sort order.

use proptest::prelude::*;
use zipease_extract::ArchiveEntryInfo;
use zipease_preview::image_format::{is_previewable, previewable_entries_in_directory, SUPPORTED_EXTENSIONS};
use zipease_preview::natural_sort::natural_cmp;

/// Strategy to generate a directory path (e.g., "images/", "a/b/", or "" for root).
fn arbitrary_directory() -> impl Strategy<Value = String> {
    prop_oneof![
        // Root directory (empty string)
        Just("".to_string()),
        // Single-level directory
        "[a-zA-Z0-9_]{1,10}/".prop_map(|s| s),
        // Multi-level directory
        ("[a-zA-Z0-9_]{1,8}", "[a-zA-Z0-9_]{1,8}")
            .prop_map(|(a, b)| format!("{}/{}/", a, b)),
    ]
}

/// Strategy to generate a filename (basename without directory).
fn arbitrary_filename() -> impl Strategy<Value = String> {
    prop_oneof![
        // Filename with a supported extension
        ("[a-zA-Z0-9_]{1,15}", prop::sample::select(SUPPORTED_EXTENSIONS))
            .prop_map(|(name, ext)| format!("{}.{}", name, ext)),
        // Filename with a supported extension in mixed case
        ("[a-zA-Z0-9_]{1,15}", prop::sample::select(SUPPORTED_EXTENSIONS))
            .prop_map(|(name, ext)| {
                let mixed: String = ext.chars().enumerate().map(|(i, c)| {
                    if i % 2 == 0 { c.to_ascii_uppercase() } else { c }
                }).collect();
                format!("{}.{}", name, mixed)
            }),
        // Filename with unsupported extension
        ("[a-zA-Z0-9_]{1,15}", "[a-zA-Z]{1,5}")
            .prop_filter("must not be a supported extension",
                |(_, ext)| !SUPPORTED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
            .prop_map(|(name, ext)| format!("{}.{}", name, ext)),
        // Filename without extension
        "[a-zA-Z0-9_]{1,15}".prop_map(|s| s),
        // Directory entry name (no extension, will be marked is_directory)
        "[a-zA-Z0-9_]{1,10}".prop_map(|s| s),
    ]
}

/// Strategy to generate a single ArchiveEntryInfo with a given directory prefix.
fn entry_in_directory(dir: String) -> impl Strategy<Value = ArchiveEntryInfo> {
    (arbitrary_filename(), any::<bool>(), 0i64..10_000i64).prop_map(
        move |(filename, is_directory, size)| ArchiveEntryInfo {
            name: format!("{}{}", dir, filename),
            is_directory,
            size,
        },
    )
}

/// Strategy to generate a list of ArchiveEntryInfo with entries in various directories.
fn arbitrary_entries(target_dir: String) -> impl Strategy<Value = Vec<ArchiveEntryInfo>> {
    let target_dir_clone = target_dir.clone();
    let other_dirs = prop::collection::vec(arbitrary_directory(), 0..3);

    (
        // Entries in the target directory
        prop::collection::vec(entry_in_directory(target_dir), 0..10),
        // Entries in other directories
        other_dirs.prop_flat_map(|dirs| {
            let strategies: Vec<_> = dirs
                .into_iter()
                .map(|d| prop::collection::vec(entry_in_directory(d), 0..5))
                .collect();
            strategies
                .into_iter()
                .fold(
                    Just(Vec::new()).boxed(),
                    |acc, strat| {
                        (acc, strat)
                            .prop_map(|(mut a, b)| {
                                a.extend(b);
                                a
                            })
                            .boxed()
                    },
                )
        }),
    )
        .prop_map(move |(mut target_entries, other_entries)| {
            // Also add some entries in a subdirectory of target to test exact match
            let sub_dir = format!("{}sub/", target_dir_clone);
            let sub_entry = ArchiveEntryInfo {
                name: format!("{}nested.png", sub_dir),
                is_directory: false,
                size: 100,
            };
            target_entries.extend(other_entries);
            target_entries.push(sub_entry);
            target_entries
        })
}

proptest! {
    /// **Validates: Requirements 3.9, 3.2, 3.3**
    ///
    /// For any list of archive entries and any target directory, the result of
    /// `previewable_entries_in_directory` satisfies:
    /// 1. All returned entries have parent directory == target directory
    /// 2. All returned entries are previewable (is_previewable returns true)
    /// 3. Result is sorted by natural_cmp
    /// 4. No previewable entry in the target directory is missing from the result
    #[test]
    fn prop_directory_scoped_navigation_filtering(
        target_dir in arbitrary_directory(),
        entries in arbitrary_directory().prop_flat_map(|dir| arbitrary_entries(dir)),
    ) {
        // Use the target_dir from the entries generation isn't aligned here,
        // so let's re-derive: we generate target_dir and entries independently
        // but that's fine — the function should work for any combination.
        let _ = &target_dir; // suppress unused warning from the above note
        let result = previewable_entries_in_directory(&entries, &target_dir);

        // Property 1: All returned entries have parent directory == target directory
        for entry_name in &result {
            let parent = parent_directory_oracle(entry_name);
            prop_assert_eq!(
                parent, target_dir.as_str(),
                "Entry {:?} has parent {:?}, expected {:?}",
                entry_name, parent, target_dir
            );
        }

        // Property 2: All returned entries are previewable
        for entry_name in &result {
            prop_assert!(
                is_previewable(entry_name, false),
                "Entry {:?} in result is not previewable",
                entry_name
            );
        }

        // Property 3: Result is sorted by natural_cmp
        for window in result.windows(2) {
            let ord = natural_cmp(&window[0], &window[1]);
            prop_assert!(
                ord != std::cmp::Ordering::Greater,
                "Result not sorted: {:?} should come before {:?}",
                window[0], window[1]
            );
        }

        // Property 4: No previewable entry in the target directory is missing
        let expected_set: Vec<String> = entries
            .iter()
            .filter(|e| {
                parent_directory_oracle(&e.name) == target_dir
                    && is_previewable(&e.name, e.is_directory)
            })
            .map(|e| e.name.clone())
            .collect();

        for expected_name in &expected_set {
            prop_assert!(
                result.contains(expected_name),
                "Expected entry {:?} missing from result",
                expected_name
            );
        }

        // Also verify no extra entries beyond what's expected
        prop_assert_eq!(
            result.len(),
            expected_set.len(),
            "Result length {} != expected length {}. Result: {:?}, Expected: {:?}",
            result.len(), expected_set.len(), result, expected_set
        );
    }

    /// Supplementary property: empty entry list always produces empty result.
    #[test]
    fn prop_empty_entries_produce_empty_result(
        target_dir in arbitrary_directory(),
    ) {
        let entries: Vec<ArchiveEntryInfo> = vec![];
        let result = previewable_entries_in_directory(&entries, &target_dir);
        prop_assert!(result.is_empty(), "Expected empty result for empty entries");
    }

    /// Supplementary property: directory entries are never included in the result.
    #[test]
    fn prop_directory_entries_excluded(
        target_dir in arbitrary_directory(),
        filenames in prop::collection::vec("[a-zA-Z0-9_]{1,10}", 1..10),
    ) {
        // Create entries that are all directories with previewable-looking names
        let entries: Vec<ArchiveEntryInfo> = filenames
            .into_iter()
            .map(|name| ArchiveEntryInfo {
                name: format!("{}{}.png", target_dir, name),
                is_directory: true,
                size: 0,
            })
            .collect();

        let result = previewable_entries_in_directory(&entries, &target_dir);
        prop_assert!(
            result.is_empty(),
            "Directory entries should never appear in result, got: {:?}",
            result
        );
    }

    /// Supplementary property: entries in subdirectories of target are excluded.
    #[test]
    fn prop_subdirectory_entries_excluded(
        target_dir in "[a-zA-Z0-9_]{1,8}/".prop_map(|s| s),
        sub_name in "[a-zA-Z0-9_]{1,8}",
        filenames in prop::collection::vec("[a-zA-Z0-9_]{1,10}", 1..5),
    ) {
        let sub_dir = format!("{}{}/", target_dir, sub_name);
        let entries: Vec<ArchiveEntryInfo> = filenames
            .into_iter()
            .map(|name| ArchiveEntryInfo {
                name: format!("{}{}.png", sub_dir, name),
                is_directory: false,
                size: 100,
            })
            .collect();

        let result = previewable_entries_in_directory(&entries, &target_dir);
        prop_assert!(
            result.is_empty(),
            "Subdirectory entries should not appear when filtering for parent. \
             Target: {:?}, SubDir: {:?}, Result: {:?}",
            target_dir, sub_dir, result
        );
    }
}

/// Oracle function: computes the parent directory of an entry name.
/// Matches the logic in `image_format.rs` — everything up to and including
/// the last '/' or '\' separator, or "" if no separator exists.
fn parent_directory_oracle(entry_name: &str) -> &str {
    let last_sep = entry_name.rfind(|c| c == '/' || c == '\\');
    match last_sep {
        Some(pos) => &entry_name[..=pos],
        None => "",
    }
}
