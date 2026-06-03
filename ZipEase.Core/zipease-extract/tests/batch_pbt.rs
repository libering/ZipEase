//! Property-based tests for batch extraction.
//!
//! Feature: batch-extraction, Property 2: Error isolation — failures do not block remaining archives
//! Validates: Requirements 4.1, 4.4

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use proptest::prelude::*;

use zipease_extract::batch::{ArchiveStatus, BatchProgress, BatchResult};

/// A testable version of batch_extract that accepts a generic extraction function.
/// This mirrors the production `batch_extract` logic but allows injecting mock behavior.
fn batch_extract_with_extractor<F, E>(
    archives: &[PathBuf],
    output_dir: &Path,
    cancel_flag: &AtomicBool,
    progress_fn: impl Fn(BatchProgress),
    extractor: E,
) -> BatchResult
where
    E: Fn(&Path, &Path, &dyn Fn(usize, usize, &str)) -> Result<(), F>,
    F: std::fmt::Display,
{
    let archive_count = archives.len() as u32;
    let mut results: Vec<(PathBuf, ArchiveStatus)> = Vec::with_capacity(archives.len());
    let mut cancelled = false;

    for (index, archive_path) in archives.iter().enumerate() {
        // Check cancel flag before processing each archive
        if cancel_flag.load(Ordering::Relaxed) {
            cancelled = true;
            for remaining in &archives[index..] {
                results.push((remaining.clone(), ArchiveStatus::Skipped));
            }
            break;
        }

        let archive_index = index as u32;
        let file_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        progress_fn(BatchProgress {
            archive_index,
            archive_count,
            file_percent: 0,
            current_file_name: file_name.clone(),
        });

        let batch_progress_wrapper = |current: usize, total: usize, entry_name: &str| {
            let percent = if total > 0 {
                ((current as f64 / total as f64) * 100.0) as i32
            } else {
                0
            };
            progress_fn(BatchProgress {
                archive_index,
                archive_count,
                file_percent: percent.clamp(0, 100),
                current_file_name: entry_name.to_string(),
            });
        };

        let status = match extractor(archive_path, output_dir, &batch_progress_wrapper) {
            Ok(()) => {
                progress_fn(BatchProgress {
                    archive_index,
                    archive_count,
                    file_percent: 100,
                    current_file_name: file_name,
                });
                ArchiveStatus::Success
            }
            Err(e) => ArchiveStatus::Failed(e.to_string()),
        };

        results.push((archive_path.clone(), status));
    }

    let total_files_extracted = results
        .iter()
        .filter(|(_, s)| matches!(s, ArchiveStatus::Success))
        .count() as u32;

    BatchResult {
        results,
        cancelled,
        total_files_extracted,
    }
}

/// Strategy that guarantees at least one failure in the batch.
fn batch_with_failures_strategy() -> impl Strategy<Value = Vec<(PathBuf, bool)>> {
    // Generate 2..30 archives
    proptest::collection::vec(
        (
            "[a-zA-Z0-9_]{1,16}\\.(zip|7z|tar\\.gz|rar)",
            any::<bool>(),
        ),
        2..30,
    )
    .prop_map(|mut entries| {
        // Ensure at least one failure exists (not the last one, so we can verify continuation)
        if entries.iter().all(|(_, succeeds)| *succeeds) {
            // Force a failure at a random-ish position (first element)
            entries[0].1 = false;
        }
        entries
            .into_iter()
            .map(|(name, succeeds)| (PathBuf::from(name), succeeds))
            .collect()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: batch-extraction, Property 2: Error isolation — failures do not block remaining archives
    // Validates: Requirements 4.1, 4.4
    //
    // For any batch of N archives where some archives fail, ALL archives are still attempted
    // and BatchResult contains exactly N entries.
    #[test]
    fn prop_error_isolation_all_archives_attempted(
        batch in batch_with_failures_strategy()
    ) {
        let n = batch.len();
        let archives: Vec<PathBuf> = batch.iter().map(|(p, _)| p.clone()).collect();
        let outcomes: Vec<bool> = batch.iter().map(|(_, s)| *s).collect();

        let cancel_flag = AtomicBool::new(false);
        let output_dir = PathBuf::from("C:\\temp\\output");

        // Track which archives were actually attempted
        let attempted = std::sync::Mutex::new(Vec::new());
        // Use a call counter to handle duplicate filenames correctly
        let call_counter = AtomicUsize::new(0);

        let result = batch_extract_with_extractor(
            &archives,
            &output_dir,
            &cancel_flag,
            |_progress| {},  // no-op progress
            |archive_path: &Path, _output: &Path, _progress_cb: &dyn Fn(usize, usize, &str)| -> Result<(), String> {
                attempted.lock().unwrap().push(archive_path.to_path_buf());
                // Use call order to determine success/failure (handles duplicate filenames)
                let idx = call_counter.fetch_add(1, Ordering::Relaxed);
                if outcomes[idx] {
                    Ok(())
                } else {
                    Err(format!("Simulated failure for {:?}", archive_path))
                }
            },
        );

        // Property: BatchResult contains exactly N entries
        prop_assert_eq!(
            result.results.len(),
            n,
            "BatchResult should contain exactly {} entries, got {}",
            n,
            result.results.len()
        );

        // Property: ALL archives were attempted (none skipped due to prior failures)
        let attempted_paths = attempted.lock().unwrap();
        prop_assert_eq!(
            attempted_paths.len(),
            n,
            "All {} archives should be attempted, but only {} were",
            n,
            attempted_paths.len()
        );

        // Property: Each archive's status matches expected outcome
        for (i, (path, status)) in result.results.iter().enumerate() {
            if outcomes[i] {
                prop_assert_eq!(
                    status,
                    &ArchiveStatus::Success,
                    "Archive {} ({:?}) should be Success",
                    i,
                    path
                );
            } else {
                prop_assert!(
                    matches!(status, ArchiveStatus::Failed(_)),
                    "Archive {} ({:?}) should be Failed, got {:?}",
                    i,
                    path,
                    status
                );
            }
        }

        // Property: cancelled flag is false (no cancellation occurred)
        prop_assert!(!result.cancelled, "Batch should not be marked as cancelled");
    }

    // Additional property: even with ALL failures, every archive is still attempted
    // and the result has N entries.
    #[test]
    fn prop_error_isolation_all_failures_still_attempts_all(
        archive_names in proptest::collection::vec(
            "[a-zA-Z0-9_]{1,16}\\.(zip|7z|tar\\.gz|rar)",
            1..30
        )
    ) {
        let archives: Vec<PathBuf> = archive_names.into_iter().map(PathBuf::from).collect();
        let n = archives.len();

        let cancel_flag = AtomicBool::new(false);
        let output_dir = PathBuf::from("C:\\temp\\output");

        let attempt_count = std::sync::Mutex::new(0usize);

        let result = batch_extract_with_extractor(
            &archives,
            &output_dir,
            &cancel_flag,
            |_progress| {},
            |_archive_path: &Path, _output: &Path, _progress_cb: &dyn Fn(usize, usize, &str)| -> Result<(), String> {
                *attempt_count.lock().unwrap() += 1;
                // ALL extractions fail
                Err("Simulated total failure".to_string())
            },
        );

        // Property: BatchResult contains exactly N entries even when all fail
        prop_assert_eq!(
            result.results.len(),
            n,
            "BatchResult should have {} entries, got {}",
            n,
            result.results.len()
        );

        // Property: All N archives were attempted
        let attempts = *attempt_count.lock().unwrap();
        prop_assert_eq!(
            attempts,
            n,
            "All {} archives should be attempted, but only {} were",
            n,
            attempts
        );

        // Property: All statuses are Failed
        for (i, (_, status)) in result.results.iter().enumerate() {
            prop_assert!(
                matches!(status, ArchiveStatus::Failed(_)),
                "Archive {} should be Failed, got {:?}",
                i,
                status
            );
        }
    }
}


// ============================================================================
// Feature: batch-extraction, Property 3: BatchResult completeness and count consistency
// Validates: Requirements 4.4, 5.4, 6.4
// ============================================================================

/// Strategy to generate a random terminal ArchiveStatus (states that appear in a completed batch).
/// Pending and Extracting are transient states and should not appear in final results.
fn terminal_archive_status_strategy() -> impl Strategy<Value = ArchiveStatus> {
    prop_oneof![
        Just(ArchiveStatus::Success),
        "[a-zA-Z0-9_ ]{1,32}".prop_map(ArchiveStatus::Failed),
        Just(ArchiveStatus::PasswordRequired),
        Just(ArchiveStatus::ZipBomb),
        Just(ArchiveStatus::Skipped),
    ]
}

/// Strategy to generate a Vec of (PathBuf, ArchiveStatus) pairs representing a completed batch.
fn completed_batch_results_strategy() -> impl Strategy<Value = Vec<(PathBuf, ArchiveStatus)>> {
    proptest::collection::vec(
        (
            "[a-zA-Z0-9_]{1,16}\\.(zip|7z|rar|tar\\.gz)",
            terminal_archive_status_strategy(),
        ),
        1..100,
    )
    .prop_map(|entries| {
        entries
            .into_iter()
            .map(|(name, status)| (PathBuf::from(name), status))
            .collect()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: batch-extraction, Property 3: BatchResult completeness and count consistency
    // Validates: Requirements 4.4, 5.4, 6.4
    //
    // **Validates: Requirements 4.4, 5.4, 6.4**
    //
    // For any completed batch of N archives, success_count() + failure_count() == N.
    // Skipped/cancelled archives count as failures per the design doc.
    #[test]
    fn prop_batch_result_completeness_and_count_consistency(
        results in completed_batch_results_strategy(),
        cancelled in any::<bool>(),
    ) {
        let n = results.len() as u32;

        // Compute expected success count manually
        let expected_success = results
            .iter()
            .filter(|(_, s)| matches!(s, ArchiveStatus::Success))
            .count() as u32;

        let batch_result = BatchResult {
            total_files_extracted: expected_success,
            results,
            cancelled,
        };

        // Property: success_count() + failure_count() == N
        let success = batch_result.success_count();
        let failure = batch_result.failure_count();

        prop_assert_eq!(
            success + failure,
            n,
            "success_count ({}) + failure_count ({}) should equal N ({}), but got {}",
            success,
            failure,
            n,
            success + failure
        );

        // Additional invariant: success_count matches our manual count
        prop_assert_eq!(
            success,
            expected_success,
            "success_count() should equal the number of Success entries"
        );

        // Additional invariant: BatchResult contains exactly N entries
        prop_assert_eq!(
            batch_result.results.len() as u32,
            n,
            "BatchResult should contain exactly N entries"
        );
    }

    // Additional property: when all archives succeed, failure_count is 0 and success_count == N
    #[test]
    fn prop_batch_result_all_success_counts(
        archive_names in proptest::collection::vec(
            "[a-zA-Z0-9_]{1,16}\\.(zip|7z|rar)",
            1..50
        )
    ) {
        let n = archive_names.len() as u32;
        let results: Vec<(PathBuf, ArchiveStatus)> = archive_names
            .into_iter()
            .map(|name| (PathBuf::from(name), ArchiveStatus::Success))
            .collect();

        let batch_result = BatchResult {
            total_files_extracted: n,
            results,
            cancelled: false,
        };

        prop_assert_eq!(batch_result.success_count(), n);
        prop_assert_eq!(batch_result.failure_count(), 0);
        prop_assert_eq!(
            batch_result.success_count() + batch_result.failure_count(),
            n
        );
    }

    // Additional property: when all archives fail, success_count is 0 and failure_count == N
    #[test]
    fn prop_batch_result_all_failure_counts(
        results in proptest::collection::vec(
            (
                "[a-zA-Z0-9_]{1,16}\\.(zip|7z|rar)",
                prop_oneof![
                    "[a-zA-Z0-9_ ]{1,32}".prop_map(ArchiveStatus::Failed),
                    Just(ArchiveStatus::PasswordRequired),
                    Just(ArchiveStatus::ZipBomb),
                    Just(ArchiveStatus::Skipped),
                ],
            ),
            1..50
        )
    ) {
        let n = results.len() as u32;
        let results: Vec<(PathBuf, ArchiveStatus)> = results
            .into_iter()
            .map(|(name, status)| (PathBuf::from(name), status))
            .collect();

        let batch_result = BatchResult {
            total_files_extracted: 0,
            results,
            cancelled: false,
        };

        prop_assert_eq!(batch_result.success_count(), 0);
        prop_assert_eq!(batch_result.failure_count(), n);
        prop_assert_eq!(
            batch_result.success_count() + batch_result.failure_count(),
            n
        );
    }
}


// ============================================================================
// Feature: batch-extraction, Property 4: Cancellation stops processing at boundary
// Validates: Requirements 7.3, 7.4
// ============================================================================

/// Strategy to generate a batch size (2..30) and a cancellation index within that batch.
/// The cancellation index K means the cancel flag is set while archive K is being processed,
/// so archives at indices > K should be Skipped.
fn cancellation_scenario_strategy() -> impl Strategy<Value = (Vec<PathBuf>, usize)> {
    (2usize..30usize).prop_flat_map(|n| {
        let archives = proptest::collection::vec(
            "[a-zA-Z0-9_]{1,16}\\.(zip|7z|tar\\.gz|rar)",
            n..=n,
        )
        .prop_map(|names| names.into_iter().map(PathBuf::from).collect::<Vec<_>>());

        // cancel_at is the index at which we set the cancel flag (0..n-1)
        // We use 0..n-1 to ensure there's always at least one archive after the cancel point
        let cancel_at = 0..n.saturating_sub(1).max(1);

        (archives, cancel_at)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: batch-extraction, Property 4: Cancellation stops processing at boundary
    // Validates: Requirements 7.3, 7.4
    //
    // **Validates: Requirements 7.3, 7.4**
    //
    // For any batch of N archives, if the cancel flag is set while archive at index K
    // is being processed, then archives at indices > K have status Skipped,
    // the cancelled flag is true, and the return value equals the number of archives
    // that completed successfully before cancellation.
    #[test]
    fn prop_cancellation_stops_processing_at_boundary(
        (archives, cancel_at) in cancellation_scenario_strategy()
    ) {
        let n = archives.len();
        let cancel_flag = AtomicBool::new(false);
        let output_dir = PathBuf::from("C:\\temp\\output");

        // Track which archives were actually processed (extractor was called)
        let processed_indices = std::sync::Mutex::new(Vec::new());

        let result = batch_extract_with_extractor(
            &archives,
            &output_dir,
            &cancel_flag,
            |_progress| {},
            |archive_path: &Path, _output: &Path, _progress_cb: &dyn Fn(usize, usize, &str)| -> Result<(), String> {
                let idx = archives.iter().position(|p| p == archive_path).unwrap();
                processed_indices.lock().unwrap().push(idx);

                // When we reach the cancel_at index, set the cancel flag.
                // The current archive (cancel_at) still completes successfully,
                // but subsequent archives should be skipped.
                if idx == cancel_at {
                    cancel_flag.store(true, Ordering::Relaxed);
                }

                // All processed archives succeed
                Ok(())
            },
        );

        // Property: cancelled flag is true
        prop_assert!(
            result.cancelled,
            "Batch should be marked as cancelled when cancel_flag is set at index {}",
            cancel_at
        );

        // Property: archives at indices > cancel_at have status Skipped
        for i in (cancel_at + 1)..n {
            let (ref path, ref status) = result.results[i];
            prop_assert_eq!(
                status,
                &ArchiveStatus::Skipped,
                "Archive at index {} ({:?}) should be Skipped after cancellation at index {}, got {:?}",
                i,
                path,
                cancel_at,
                status
            );
        }

        // Property: archives at indices <= cancel_at have status Success
        // (since our mock extractor always returns Ok(()))
        for i in 0..=cancel_at {
            let (ref path, ref status) = result.results[i];
            prop_assert_eq!(
                status,
                &ArchiveStatus::Success,
                "Archive at index {} ({:?}) should be Success (processed before/at cancellation), got {:?}",
                i,
                path,
                status
            );
        }

        // Property: return value (total_files_extracted) equals successful archives before cancellation
        let expected_success_count = (cancel_at + 1) as u32;
        prop_assert_eq!(
            result.total_files_extracted,
            expected_success_count,
            "total_files_extracted should be {} (archives 0..={}), got {}",
            expected_success_count,
            cancel_at,
            result.total_files_extracted
        );

        // Property: BatchResult contains exactly N entries
        prop_assert_eq!(
            result.results.len(),
            n,
            "BatchResult should contain exactly {} entries, got {}",
            n,
            result.results.len()
        );

        // Property: only archives 0..=cancel_at were actually processed by the extractor
        let processed = processed_indices.lock().unwrap();
        prop_assert_eq!(
            processed.len(),
            cancel_at + 1,
            "Only {} archives should have been processed, but {} were",
            cancel_at + 1,
            processed.len()
        );
    }

    // Additional property: cancellation at index 0 means only the first archive is processed,
    // all others are Skipped.
    #[test]
    fn prop_cancellation_at_first_archive_skips_rest(
        archive_names in proptest::collection::vec(
            "[a-zA-Z0-9_]{1,16}\\.(zip|7z|tar\\.gz|rar)",
            2..30
        )
    ) {
        let archives: Vec<PathBuf> = archive_names.into_iter().map(PathBuf::from).collect();
        let n = archives.len();
        let cancel_flag = AtomicBool::new(false);
        let output_dir = PathBuf::from("C:\\temp\\output");

        let result = batch_extract_with_extractor(
            &archives,
            &output_dir,
            &cancel_flag,
            |_progress| {},
            |archive_path: &Path, _output: &Path, _progress_cb: &dyn Fn(usize, usize, &str)| -> Result<(), String> {
                let idx = archives.iter().position(|p| p == archive_path).unwrap();
                // Cancel immediately when processing the first archive
                if idx == 0 {
                    cancel_flag.store(true, Ordering::Relaxed);
                }
                Ok(())
            },
        );

        // cancelled flag must be true
        prop_assert!(result.cancelled);

        // First archive should be Success (it completed before the flag was checked again)
        prop_assert_eq!(&result.results[0].1, &ArchiveStatus::Success);

        // All remaining archives should be Skipped
        for i in 1..n {
            prop_assert_eq!(
                &result.results[i].1,
                &ArchiveStatus::Skipped,
                "Archive at index {} should be Skipped, got {:?}",
                i,
                result.results[i].1
            );
        }

        // total_files_extracted should be 1
        prop_assert_eq!(result.total_files_extracted, 1);
    }
}


// ============================================================================
// Feature: batch-extraction, Property 1: Archive filtering preserves only supported formats
// Validates: Requirements 1.1, 1.3
// ============================================================================

use zipease_extract::batch::filter_supported_archives;

/// All extensions that `filter_supported_archives` should accept.
const ALL_SUPPORTED_EXTENSIONS: &[&str] = &[
    "zip", "7z", "rar", "tar", "gz", "bz2", "xz", "zst",
    "cab", "iso", "apk", "ipa", "jar", "war", "ear",
];

/// Extensions that should NOT pass the filter.
const UNSUPPORTED_EXTENSIONS: &[&str] = &[
    "txt", "pdf", "exe", "dll", "png", "jpg", "mp3", "mp4",
    "doc", "xlsx", "html", "css", "js", "rs", "toml", "json",
    "xml", "csv", "log", "bat", "ps1", "py", "md", "yaml",
];

/// Strategy to generate a random supported extension.
fn supported_ext_strategy() -> impl Strategy<Value = String> {
    prop::sample::select(ALL_SUPPORTED_EXTENSIONS)
        .prop_map(|s| s.to_string())
}

/// Strategy to generate a random unsupported extension.
fn unsupported_ext_strategy() -> impl Strategy<Value = String> {
    prop::sample::select(UNSUPPORTED_EXTENSIONS)
        .prop_map(|s| s.to_string())
}

/// Strategy to generate a split archive extension (.001, .z01-.z99).
fn split_archive_ext_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("001".to_string()),
        (1u32..99u32).prop_map(|n| format!("z{:02}", n)),
    ]
}

/// Strategy to generate a random file path with a given extension.
fn path_with_ext(ext: impl Strategy<Value = String>) -> impl Strategy<Value = PathBuf> {
    ("[a-zA-Z0-9_]{1,20}", ext)
        .prop_map(|(name, ext)| PathBuf::from(format!("{}.{}", name, ext)))
}

/// Strategy to generate a mixed list of supported and unsupported file paths.
/// Returns (all_paths, expected_supported_indices).
fn mixed_paths_strategy() -> impl Strategy<Value = (Vec<PathBuf>, Vec<usize>)> {
    // Generate 1..50 entries, each randomly supported or unsupported
    proptest::collection::vec(
        prop_oneof![
            // ~40% supported standard extensions
            path_with_ext(supported_ext_strategy()).prop_map(|p| (p, true)),
            // ~30% unsupported extensions
            path_with_ext(unsupported_ext_strategy()).prop_map(|p| (p, false)),
            // ~15% split archive extensions (supported)
            path_with_ext(split_archive_ext_strategy()).prop_map(|p| (p, true)),
            // ~15% files with no extension (unsupported)
            "[a-zA-Z0-9_]{1,20}".prop_map(|name| (PathBuf::from(name), false)),
        ],
        1..50,
    )
    .prop_map(|entries| {
        let mut paths = Vec::new();
        let mut supported_indices = Vec::new();
        for (i, (path, is_supported)) in entries.into_iter().enumerate() {
            if is_supported {
                supported_indices.push(i);
            }
            paths.push(path);
        }
        (paths, supported_indices)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: batch-extraction, Property 1: Archive filtering preserves only supported formats
    // Validates: Requirements 1.1, 1.3
    //
    // **Validates: Requirements 1.1, 1.3**
    //
    // For any list of file paths with mixed extensions, filtering produces a list
    // containing ONLY files with supported extensions, and no supported files are dropped.
    #[test]
    fn prop_archive_filtering_preserves_only_supported_formats(
        (paths, expected_supported_indices) in mixed_paths_strategy()
    ) {
        let result = filter_supported_archives(&paths);

        // Property 1a: ALL files in the result have supported extensions
        for path in &result {
            let ext = path.extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            let is_valid = ALL_SUPPORTED_EXTENSIONS.contains(&ext.as_str())
                || ext == "001"
                || (ext.starts_with('z') && ext.len() > 1 && ext[1..].parse::<u32>().is_ok());

            prop_assert!(
                is_valid,
                "Filtered result contains unsupported extension: {:?} (ext: {})",
                path,
                ext
            );
        }

        // Property 1b: No supported files are dropped (no false negatives)
        let expected_paths: Vec<&PathBuf> = expected_supported_indices
            .iter()
            .map(|&i| &paths[i])
            .collect();

        prop_assert_eq!(
            result.len(),
            expected_paths.len(),
            "Filter should return exactly {} supported files, got {}",
            expected_paths.len(),
            result.len()
        );

        // Verify each expected supported file is present in the result
        for expected in &expected_paths {
            prop_assert!(
                result.contains(expected),
                "Supported file {:?} was dropped by the filter",
                expected
            );
        }
    }

    // Additional property: filtering an all-supported list returns the same list unchanged.
    #[test]
    fn prop_archive_filtering_all_supported_returns_all(
        paths in proptest::collection::vec(
            path_with_ext(supported_ext_strategy()),
            1..50
        )
    ) {
        let result = filter_supported_archives(&paths);

        prop_assert_eq!(
            result.len(),
            paths.len(),
            "All-supported input of {} files should return {} files, got {}",
            paths.len(),
            paths.len(),
            result.len()
        );

        for (original, filtered) in paths.iter().zip(result.iter()) {
            prop_assert_eq!(
                original,
                filtered,
                "Order should be preserved: expected {:?}, got {:?}",
                original,
                filtered
            );
        }
    }

    // Additional property: filtering an all-unsupported list returns empty.
    #[test]
    fn prop_archive_filtering_all_unsupported_returns_empty(
        paths in proptest::collection::vec(
            path_with_ext(unsupported_ext_strategy()),
            1..50
        )
    ) {
        let result = filter_supported_archives(&paths);

        prop_assert_eq!(
            result.len(),
            0,
            "All-unsupported input should return empty, got {} files: {:?}",
            result.len(),
            result
        );
    }
}


// ============================================================================
// Feature: batch-extraction, Property 5: Progress callback reports correct indices
// Validates: Requirements 3.4, 6.3
// ============================================================================

/// Captured progress callback data (since BatchProgress doesn't derive Clone).
#[derive(Debug, Clone)]
struct CapturedProgress {
    archive_index: u32,
    archive_count: u32,
    file_percent: i32,
    current_file_name: String,
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: batch-extraction, Property 5: Progress callback reports correct indices
    // Validates: Requirements 3.4, 6.3
    //
    // **Validates: Requirements 3.4, 6.3**
    //
    // For any batch of N archives, every progress callback invocation has:
    // - archive_count == N
    // - archive_index in [0, N-1]
    // - file_percent in [0, 100]
    // - archive_index values are monotonically non-decreasing across invocations
    #[test]
    fn prop_progress_callback_reports_correct_indices(
        archive_names in proptest::collection::vec(
            "[a-zA-Z0-9_]{1,16}\\.(zip|7z|tar\\.gz|rar)",
            1..30
        )
    ) {
        let archives: Vec<PathBuf> = archive_names.into_iter().map(PathBuf::from).collect();
        let n = archives.len() as u32;
        let cancel_flag = AtomicBool::new(false);
        let output_dir = PathBuf::from("C:\\temp\\output");

        // Collect all progress callbacks
        let callbacks = std::sync::Mutex::new(Vec::<CapturedProgress>::new());

        let _result = batch_extract_with_extractor(
            &archives,
            &output_dir,
            &cancel_flag,
            |progress: BatchProgress| {
                callbacks.lock().unwrap().push(CapturedProgress {
                    archive_index: progress.archive_index,
                    archive_count: progress.archive_count,
                    file_percent: progress.file_percent,
                    current_file_name: progress.current_file_name,
                });
            },
            // Mock extractor that always succeeds and reports some intermediate progress
            |_archive_path: &Path, _output: &Path, progress_cb: &dyn Fn(usize, usize, &str)| -> Result<(), String> {
                // Simulate extracting 3 files to generate intermediate callbacks
                progress_cb(1, 3, "file1.txt");
                progress_cb(2, 3, "file2.txt");
                progress_cb(3, 3, "file3.txt");
                Ok(())
            },
        );

        let captured = callbacks.lock().unwrap();

        // There should be at least one callback per archive (the initial 0% report)
        prop_assert!(
            !captured.is_empty(),
            "Expected at least one progress callback, got none"
        );

        // Property 5a: archive_count is always equal to N
        for (i, cb) in captured.iter().enumerate() {
            prop_assert_eq!(
                cb.archive_count,
                n,
                "Callback #{}: archive_count should be {}, got {}",
                i,
                n,
                cb.archive_count
            );
        }

        // Property 5b: archive_index is always in range [0, N-1]
        for (i, cb) in captured.iter().enumerate() {
            prop_assert!(
                cb.archive_index < n,
                "Callback #{}: archive_index {} should be in range [0, {})",
                i,
                cb.archive_index,
                n
            );
        }

        // Property 5c: file_percent is always in range [0, 100]
        for (i, cb) in captured.iter().enumerate() {
            prop_assert!(
                cb.file_percent >= 0 && cb.file_percent <= 100,
                "Callback #{}: file_percent {} should be in range [0, 100]",
                i,
                cb.file_percent
            );
        }

        // Property 5d: archive_index values are monotonically non-decreasing
        for i in 1..captured.len() {
            prop_assert!(
                captured[i].archive_index >= captured[i - 1].archive_index,
                "Callback #{}: archive_index {} is less than previous callback's archive_index {} (not monotonically non-decreasing)",
                i,
                captured[i].archive_index,
                captured[i - 1].archive_index
            );
        }
    }

    // Additional property: with mixed success/failure, progress callbacks still report correct indices
    #[test]
    fn prop_progress_callback_correct_indices_with_failures(
        batch in proptest::collection::vec(
            (
                "[a-zA-Z0-9_]{1,16}\\.(zip|7z|tar\\.gz|rar)",
                any::<bool>(),
            ),
            2..30
        )
    ) {
        let archives: Vec<PathBuf> = batch.iter().map(|(name, _)| PathBuf::from(name)).collect();
        let outcomes: Vec<bool> = batch.iter().map(|(_, succeeds)| *succeeds).collect();
        let n = archives.len() as u32;
        let cancel_flag = AtomicBool::new(false);
        let output_dir = PathBuf::from("C:\\temp\\output");

        let callbacks = std::sync::Mutex::new(Vec::<CapturedProgress>::new());

        let _result = batch_extract_with_extractor(
            &archives,
            &output_dir,
            &cancel_flag,
            |progress: BatchProgress| {
                callbacks.lock().unwrap().push(CapturedProgress {
                    archive_index: progress.archive_index,
                    archive_count: progress.archive_count,
                    file_percent: progress.file_percent,
                    current_file_name: progress.current_file_name,
                });
            },
            |archive_path: &Path, _output: &Path, progress_cb: &dyn Fn(usize, usize, &str)| -> Result<(), String> {
                let idx = archives.iter().position(|p| p == archive_path).unwrap();
                if outcomes[idx] {
                    // Simulate some progress before success
                    progress_cb(1, 2, "entry.dat");
                    Ok(())
                } else {
                    // Even failures may report some progress before failing
                    progress_cb(1, 4, "partial.dat");
                    Err(format!("Simulated failure for {:?}", archive_path))
                }
            },
        );

        let captured = callbacks.lock().unwrap();

        // All the same properties must hold even with failures
        for (i, cb) in captured.iter().enumerate() {
            prop_assert_eq!(
                cb.archive_count, n,
                "Callback #{}: archive_count should be {}, got {}",
                i, n, cb.archive_count
            );
            prop_assert!(
                cb.archive_index < n,
                "Callback #{}: archive_index {} out of range [0, {})",
                i, cb.archive_index, n
            );
            prop_assert!(
                cb.file_percent >= 0 && cb.file_percent <= 100,
                "Callback #{}: file_percent {} out of range [0, 100]",
                i, cb.file_percent
            );
        }

        // Monotonically non-decreasing archive_index
        for i in 1..captured.len() {
            prop_assert!(
                captured[i].archive_index >= captured[i - 1].archive_index,
                "Callback #{}: archive_index {} < previous {}",
                i, captured[i].archive_index, captured[i - 1].archive_index
            );
        }
    }
}


// ============================================================================
// Feature: batch-extraction, Property 6: Panic safety across FFI boundary
// Validates: Requirements 6.2
// ============================================================================

use zipease_extract::ffi::batch::zip_ease_batch_extract;

/// Strategy to generate a random "invalid input scenario" for the FFI function.
/// We carefully avoid extreme path_count values with non-null paths_ptr to prevent
/// legitimate OOM aborts (which are not panics and cannot be caught by catch_unwind).
#[derive(Debug, Clone)]
struct FfiInputScenario {
    paths_ptr_null: bool,
    output_dir_null: bool,
    path_count: i32,
    /// If paths_ptr is not null, how many entries to actually allocate
    actual_entry_count: usize,
    /// Indices of entries that should be null pointers
    null_entry_indices: Vec<usize>,
}

fn ffi_input_scenario_strategy() -> impl Strategy<Value = FfiInputScenario> {
    // When paths_ptr is null, we can use any path_count (including extreme values)
    // because the function returns -1 immediately without allocating.
    // When paths_ptr is NOT null AND output_dir is NOT null AND path_count >= 0,
    // path_count must NOT exceed actual_entry_count to avoid reading unallocated memory.
    any::<bool>().prop_flat_map(|paths_ptr_null| {
        let path_count_and_entries = if paths_ptr_null {
            // Safe to use extreme values since null ptr is checked first
            (
                prop_oneof![
                    Just(0i32),
                    Just(-1i32),
                    Just(i32::MIN),
                    Just(i32::MAX),
                    (-1000i32..1000i32),
                ]
                .boxed(),
                (0usize..10usize).boxed(),
            )
        } else {
            // When paths_ptr is not null, use negative counts (caught by validation)
            // or counts that don't exceed actual entries
            (
                prop_oneof![
                    Just(0i32),
                    Just(-1i32),
                    Just(-100i32),
                    (0i32..10i32),
                ]
                .boxed(),
                (0usize..10usize).boxed(),
            )
        };

        (
            Just(paths_ptr_null),
            any::<bool>(),               // output_dir_null
            path_count_and_entries.0,
            path_count_and_entries.1,
            proptest::collection::vec(0usize..10usize, 0..5), // null_entry_indices
        )
            .prop_map(
                |(paths_ptr_null, output_dir_null, path_count, actual_entry_count, null_entry_indices)| {
                    // When paths_ptr is not null and path_count > 0, ensure we have enough entries
                    let safe_entry_count = if !paths_ptr_null && path_count > 0 {
                        actual_entry_count.max(path_count as usize)
                    } else {
                        actual_entry_count
                    };
                    FfiInputScenario {
                        paths_ptr_null,
                        output_dir_null,
                        path_count,
                        actual_entry_count: safe_entry_count,
                        null_entry_indices,
                    }
                },
            )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: batch-extraction, Property 6: Panic safety across FFI boundary
    // Validates: Requirements 6.2
    //
    // **Validates: Requirements 6.2**
    //
    // For any combination of inputs to zip_ease_batch_extract (including null pointers,
    // zero count, negative count, invalid paths), the function NEVER panics.
    // It always returns a valid i32 value (negative error code or non-negative success count).
    #[test]
    fn prop_panic_safety_across_ffi_boundary(
        scenario in ffi_input_scenario_strategy()
    ) {
        // Build a UTF-16 null-terminated output directory string
        let output_dir_wide: Vec<u16> = "C:\\temp\\output\0".encode_utf16().collect();

        // Build path entries (some may be null based on scenario)
        let path_strings: Vec<Vec<u16>> = (0..scenario.actual_entry_count)
            .map(|i| {
                format!("C:\\test\\archive_{}.zip\0", i)
                    .encode_utf16()
                    .collect()
            })
            .collect();

        // Build the pointer array
        let path_ptrs: Vec<*const u16> = path_strings
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if scenario.null_entry_indices.contains(&i) {
                    std::ptr::null()
                } else {
                    s.as_ptr()
                }
            })
            .collect();

        // Determine the actual pointers to pass
        let paths_ptr: *const *const u16 = if scenario.paths_ptr_null {
            std::ptr::null()
        } else if path_ptrs.is_empty() {
            // Empty vec with non-null requirement: use a dangling but aligned pointer
            // This is safe because path_count <= 0 or the function validates before access
            std::ptr::null()
        } else {
            path_ptrs.as_ptr()
        };

        let output_dir_ptr: *const u16 = if scenario.output_dir_null {
            std::ptr::null()
        } else {
            output_dir_wide.as_ptr()
        };

        // Call the FFI function — the key property is that this NEVER panics
        let result = zip_ease_batch_extract(
            paths_ptr,
            scenario.path_count,
            output_dir_ptr,
            None, // no progress callback
            std::ptr::null(), // no cancel flag
        );

        // Property: the function always returns a valid i32
        // The key assertion is that we REACHED this point without panicking/aborting.
        prop_assert!(
            result >= i32::MIN && result <= i32::MAX,
            "Function returned an invalid value (this should be impossible for i32): {}",
            result
        );

        // Additional invariant: with null paths_ptr or null output_dir_ptr or negative path_count,
        // the function should return a negative error code
        if scenario.paths_ptr_null || scenario.output_dir_null || scenario.path_count < 0 {
            prop_assert!(
                result < 0,
                "Expected negative error code for invalid params (paths_null={}, output_null={}, count={}), got {}",
                scenario.paths_ptr_null,
                scenario.output_dir_null,
                scenario.path_count,
                result
            );
        }
    }

    // Additional property: null paths_ptr with various counts must never panic
    // (the function checks null before using path_count to allocate)
    #[test]
    fn prop_panic_safety_null_paths_ptr_with_various_counts(
        path_count in prop_oneof![
            Just(0i32),
            Just(1i32),
            Just(-1i32),
            Just(100i32),
            Just(i32::MAX),
            Just(i32::MIN),
            (-10000i32..10000i32),
        ]
    ) {
        let output_dir_wide: Vec<u16> = "C:\\temp\\output\0".encode_utf16().collect();

        // null paths_ptr with various path_count values — must never panic
        // Safe because the function checks paths_ptr.is_null() BEFORE using path_count
        let result = zip_ease_batch_extract(
            std::ptr::null(),
            path_count,
            output_dir_wide.as_ptr(),
            None,
            std::ptr::null(),
        );

        // Should always return error code (negative) since paths_ptr is null
        prop_assert!(
            result < 0,
            "Expected negative error code for null paths_ptr with count={}, got {}",
            path_count,
            result
        );
    }

    // Additional property: null output_dir_ptr must never panic
    #[test]
    fn prop_panic_safety_null_output_dir(
        entry_count in 0usize..5usize,
        path_count in 0i32..10i32,
    ) {
        let path_strings: Vec<Vec<u16>> = (0..entry_count)
            .map(|i| format!("C:\\test\\file_{}.zip\0", i).encode_utf16().collect())
            .collect();

        let path_ptrs: Vec<*const u16> = path_strings.iter().map(|s| s.as_ptr()).collect();

        let paths_ptr = if path_ptrs.is_empty() {
            std::ptr::null()
        } else {
            path_ptrs.as_ptr()
        };

        // null output_dir_ptr — must never panic
        let result = zip_ease_batch_extract(
            paths_ptr,
            path_count,
            std::ptr::null(), // null output dir
            None,
            std::ptr::null(),
        );

        // Should return error code since output_dir is null
        prop_assert!(
            result < 0,
            "Expected negative error code for null output_dir_ptr, got {}",
            result
        );
    }

    // Additional property: zero path_count with valid pointers should return 0 (not panic)
    #[test]
    fn prop_panic_safety_zero_count_returns_zero(
        _dummy in 0u8..1u8, // proptest requires at least one input
    ) {
        let output_dir_wide: Vec<u16> = "C:\\temp\\output\0".encode_utf16().collect();
        let path_strings: Vec<Vec<u16>> = vec![
            "C:\\test\\a.zip\0".encode_utf16().collect(),
        ];
        let path_ptrs: Vec<*const u16> = path_strings.iter().map(|s| s.as_ptr()).collect();

        // Zero path_count with valid pointers — should return 0
        let result = zip_ease_batch_extract(
            path_ptrs.as_ptr(),
            0, // zero count
            output_dir_wide.as_ptr(),
            None,
            std::ptr::null(),
        );

        prop_assert_eq!(
            result, 0,
            "Expected 0 for zero path_count with valid pointers, got {}",
            result
        );
    }
}
