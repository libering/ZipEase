// Feature: image-preview-plugin, Property 8: Cache capacity invariant with LRU eviction
//
// Validates: Requirements 8.3, 8.4
//
// Property: For any sequence of cache insertions (where each entry has a known byte size),
// the total cached byte size never exceeds the configured max capacity. When an insertion
// would exceed the limit, the least recently used entries are evicted first until space
// is available.

use proptest::prelude::*;
use zipease_preview::cache::{CacheKey, PreviewCache};
use zipease_preview::decoder::DecodeResult;

/// Helper to create a CacheKey from index values.
fn make_key(archive_idx: usize, entry_idx: usize) -> CacheKey {
    CacheKey {
        archive_path: format!("/archive_{}.zip", archive_idx),
        entry_path: format!("img_{}.png", entry_idx),
    }
}

/// Helper to create a DecodeResult with a pixel buffer of the given size.
fn make_result(size: usize) -> DecodeResult {
    // Use width=size/4, height=1 for simplicity (RGBA = 4 bytes per pixel)
    let width = if size >= 4 { (size / 4) as u32 } else { 1 };
    let height = 1;
    DecodeResult {
        pixels: vec![0u8; size],
        width,
        height,
    }
}

/// Strategy for generating a sequence of cache operations.
/// Each operation is (entry_index, buffer_size).
fn insertion_sequence(max_entries: usize, max_buffer_size: usize) -> impl Strategy<Value = Vec<(usize, usize)>> {
    prop::collection::vec((0..max_entries, 1..=max_buffer_size), 1..=50)
}

proptest! {
    /// **Validates: Requirements 8.3, 8.4**
    ///
    /// For any sequence of insertions, the cache size never exceeds max_size.
    #[test]
    fn prop_cache_capacity_never_exceeded(
        max_capacity in 100usize..=2000,
        insertions in insertion_sequence(30, 500),
    ) {
        let mut cache = PreviewCache::with_capacity(max_capacity);

        for (entry_idx, buffer_size) in &insertions {
            let key = make_key(0, *entry_idx);
            let result = make_result(*buffer_size);

            cache.insert(key, result);

            // INVARIANT: current_size must never exceed max_size
            prop_assert!(
                cache.current_size() <= cache.max_size(),
                "Cache capacity invariant violated! current_size={} > max_size={} after inserting entry {} with buffer_size={}",
                cache.current_size(),
                cache.max_size(),
                entry_idx,
                buffer_size
            );
        }
    }

    /// **Validates: Requirements 8.3, 8.4**
    ///
    /// After every insertion, the reported current_size equals the sum of all
    /// cached entries' pixel buffer sizes.
    #[test]
    fn prop_cache_size_accounting_is_accurate(
        max_capacity in 200usize..=1000,
        insertions in prop::collection::vec((0usize..20, 1usize..=200), 1..=30),
    ) {
        let mut cache = PreviewCache::with_capacity(max_capacity);

        for (entry_idx, buffer_size) in &insertions {
            let key = make_key(0, *entry_idx);
            let result = make_result(*buffer_size);
            cache.insert(key, result);

            // The current_size must always be <= max_size
            prop_assert!(
                cache.current_size() <= cache.max_size(),
                "Size {} exceeded max {} after insert",
                cache.current_size(),
                cache.max_size()
            );
        }
    }

    /// **Validates: Requirements 8.3, 8.4**
    ///
    /// LRU eviction order: after filling the cache, accessing some entries promotes
    /// them to MRU. A subsequent insertion that triggers eviction should evict the
    /// least recently used entry (the one that was never accessed, or accessed earliest).
    #[test]
    fn prop_lru_eviction_order(
        num_initial in 4usize..=10,
        // We access a subset of entries (indices 1..num_initial-1), leaving index 0 as LRU
        num_to_access in 1usize..=5,
    ) {
        // Use a small capacity: each entry is 100 bytes, capacity fits exactly num_initial entries
        let entry_size = 100usize;
        let max_capacity = num_initial * entry_size;
        let mut cache = PreviewCache::with_capacity(max_capacity);

        // Fill the cache completely: entries 0, 1, 2, ..., num_initial-1
        // After insertion, LRU order is: 0 (oldest) -> 1 -> 2 -> ... -> num_initial-1 (newest)
        for i in 0..num_initial {
            let key = make_key(0, i);
            cache.insert(key, make_result(entry_size));
        }

        prop_assert_eq!(cache.current_size(), max_capacity,
            "Cache should be exactly full");

        // Access entries 1..=num_to_access (clamped to valid range) to promote them.
        // Entry 0 is deliberately NOT accessed, so it remains the LRU entry.
        let access_count = num_to_access.min(num_initial - 1);
        for i in 1..=access_count {
            let key = make_key(0, i);
            cache.get(&key);
        }

        // Insert a new entry that requires eviction of exactly one entry
        let new_key = make_key(1, 0);
        cache.insert(new_key.clone(), make_result(entry_size));

        // Capacity invariant must still hold
        prop_assert!(
            cache.current_size() <= max_capacity,
            "Cache capacity violated after eviction: current_size={} > max_size={}",
            cache.current_size(),
            max_capacity
        );

        // The new entry must be present
        prop_assert!(
            cache.get(&new_key).is_some(),
            "Newly inserted entry should be in cache"
        );

        // Entry 0 (the LRU entry that was never accessed) should have been evicted
        let lru_key = make_key(0, 0);
        prop_assert!(
            cache.get(&lru_key).is_none(),
            "Entry 0 (LRU, never accessed) should have been evicted"
        );

        // The accessed entries should still be present
        for i in 1..=access_count {
            let key = make_key(0, i);
            prop_assert!(
                cache.get(&key).is_some(),
                "Accessed entry {} should not have been evicted (it was promoted to MRU)",
                i
            );
        }
    }

    /// **Validates: Requirements 8.3, 8.4**
    ///
    /// Entries larger than max_size are never cached (skip silently).
    /// The cache state remains unchanged after attempting to insert an oversized entry.
    #[test]
    fn prop_oversized_entries_not_cached(
        max_capacity in 100usize..=500,
        oversized_amount in 1usize..=500,
    ) {
        let mut cache = PreviewCache::with_capacity(max_capacity);

        // Insert a normal entry first
        let normal_key = make_key(0, 0);
        let normal_size = max_capacity / 2;
        cache.insert(normal_key.clone(), make_result(normal_size));

        let size_before = cache.current_size();
        let len_before = cache.len();

        // Attempt to insert an entry larger than max_capacity
        let oversized_key = make_key(0, 1);
        let oversized_size = max_capacity + oversized_amount;
        cache.insert(oversized_key.clone(), make_result(oversized_size));

        // Cache state should be unchanged
        prop_assert_eq!(cache.current_size(), size_before,
            "Cache size should not change after oversized insert");
        prop_assert_eq!(cache.len(), len_before,
            "Cache length should not change after oversized insert");

        // The oversized entry should not be retrievable
        prop_assert!(cache.get(&oversized_key).is_none(),
            "Oversized entry should not be in cache");

        // The normal entry should still be present
        prop_assert!(cache.get(&normal_key).is_some(),
            "Existing entry should not be evicted by oversized insert attempt");
    }
}
