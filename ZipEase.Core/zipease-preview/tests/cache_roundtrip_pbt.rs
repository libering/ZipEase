// Feature: image-preview-plugin, Property 9: Cache round-trip by composite key
//
// Validates: Requirements 8.1, 5.6
//
// Property: For any valid (archive_path, entry_path) pair and RGBA pixel data,
// inserting into the cache and then retrieving with the same composite key returns
// the identical pixel data. Retrieving with a different key returns None.

use proptest::prelude::*;
use zipease_preview::cache::{CacheKey, PreviewCache};
use zipease_preview::decoder::DecodeResult;

/// Strategy to generate non-empty path-like strings for archive and entry paths.
fn arbitrary_path() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple filenames
        "[a-zA-Z0-9_]{1,15}\\.[a-z]{1,5}",
        // Path with directory separators
        "[a-zA-Z0-9_]{1,10}/[a-zA-Z0-9_]{1,10}\\.[a-z]{1,5}",
        // Windows-style paths
        "[A-Z]:\\\\[a-zA-Z0-9_]{1,10}\\\\[a-zA-Z0-9_]{1,10}\\.[a-z]{1,5}",
    ]
}

/// Strategy to generate a CacheKey with random archive and entry paths.
fn arbitrary_cache_key() -> impl Strategy<Value = CacheKey> {
    (arbitrary_path(), arbitrary_path()).prop_map(|(archive_path, entry_path)| CacheKey {
        archive_path,
        entry_path,
    })
}

/// Strategy to generate a DecodeResult with random pixel data of reasonable size.
fn arbitrary_decode_result() -> impl Strategy<Value = DecodeResult> {
    // Generate pixel data between 0 and 500 bytes, with width/height that make sense
    (1u32..=50, 1u32..=50).prop_flat_map(|(width, height)| {
        let pixel_count = (width * height * 4) as usize;
        (
            Just(width),
            Just(height),
            prop::collection::vec(any::<u8>(), pixel_count..=pixel_count),
        )
    }).prop_map(|(width, height, pixels)| DecodeResult {
        pixels,
        width,
        height,
    })
}

/// Strategy to generate two distinct CacheKeys (guaranteed different).
fn two_distinct_keys() -> impl Strategy<Value = (CacheKey, CacheKey)> {
    (arbitrary_cache_key(), arbitrary_cache_key()).prop_filter(
        "keys must be different",
        |(k1, k2)| k1.archive_path != k2.archive_path || k1.entry_path != k2.entry_path,
    )
}

proptest! {
    /// **Validates: Requirements 8.1, 5.6**
    ///
    /// Inserting a decode result and retrieving with the same key returns identical pixel data.
    #[test]
    fn prop_cache_roundtrip_same_key(
        key in arbitrary_cache_key(),
        result in arbitrary_decode_result(),
    ) {
        // Use a large enough cache so entries are never evicted
        let mut cache = PreviewCache::with_capacity(256 * 1024 * 1024);

        let expected_pixels = result.pixels.clone();
        let expected_width = result.width;
        let expected_height = result.height;

        cache.insert(key.clone(), result);

        let retrieved = cache.get(&key);
        prop_assert!(retrieved.is_some(), "Cache should return Some for inserted key");

        let retrieved = retrieved.unwrap();
        prop_assert_eq!(
            &retrieved.pixels, &expected_pixels,
            "Retrieved pixels must be identical to inserted pixels"
        );
        prop_assert_eq!(
            retrieved.width, expected_width,
            "Retrieved width must match inserted width"
        );
        prop_assert_eq!(
            retrieved.height, expected_height,
            "Retrieved height must match inserted height"
        );
    }

    /// **Validates: Requirements 8.1, 5.6**
    ///
    /// Retrieving with a different key than the one used for insertion returns None.
    #[test]
    fn prop_cache_miss_different_key(
        (insert_key, lookup_key) in two_distinct_keys(),
        result in arbitrary_decode_result(),
    ) {
        let mut cache = PreviewCache::with_capacity(256 * 1024 * 1024);

        cache.insert(insert_key, result);

        let retrieved = cache.get(&lookup_key);
        prop_assert!(
            retrieved.is_none(),
            "Cache should return None for a key that was not inserted"
        );
    }

    /// **Validates: Requirements 8.1, 5.6**
    ///
    /// Multiple entries with different keys don't interfere with each other.
    /// Each key retrieves its own data correctly.
    #[test]
    fn prop_cache_multiple_keys_no_interference(
        key1 in arbitrary_cache_key(),
        key2 in arbitrary_cache_key(),
        result1 in arbitrary_decode_result(),
        result2 in arbitrary_decode_result(),
    ) {
        // Skip if keys happen to be the same
        prop_assume!(
            key1.archive_path != key2.archive_path || key1.entry_path != key2.entry_path
        );

        let mut cache = PreviewCache::with_capacity(256 * 1024 * 1024);

        let expected_pixels1 = result1.pixels.clone();
        let expected_width1 = result1.width;
        let expected_height1 = result1.height;

        let expected_pixels2 = result2.pixels.clone();
        let expected_width2 = result2.width;
        let expected_height2 = result2.height;

        cache.insert(key1.clone(), result1);
        cache.insert(key2.clone(), result2);

        // Retrieve key1
        let r1 = cache.get(&key1);
        prop_assert!(r1.is_some(), "key1 should be retrievable after insertion");
        let r1 = r1.unwrap();
        prop_assert_eq!(&r1.pixels, &expected_pixels1, "key1 pixels must match");
        prop_assert_eq!(r1.width, expected_width1, "key1 width must match");
        prop_assert_eq!(r1.height, expected_height1, "key1 height must match");

        // Retrieve key2
        let r2 = cache.get(&key2);
        prop_assert!(r2.is_some(), "key2 should be retrievable after insertion");
        let r2 = r2.unwrap();
        prop_assert_eq!(&r2.pixels, &expected_pixels2, "key2 pixels must match");
        prop_assert_eq!(r2.width, expected_width2, "key2 width must match");
        prop_assert_eq!(r2.height, expected_height2, "key2 height must match");
    }
}
