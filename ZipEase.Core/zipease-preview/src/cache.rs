//! LRU preview cache for decoded image data.
//!
//! Caches `DecodeResult` entries keyed by `(archive_path, entry_path)` composite key.
//! Enforces a 256 MB total capacity based on pixel buffer sizes, evicting least-recently-used
//! entries when space is needed.

use linked_hash_map::LinkedHashMap;

use crate::decoder::DecodeResult;

/// Maximum cache capacity in bytes (256 MB).
const DEFAULT_MAX_SIZE: usize = 256 * 1024 * 1024;

/// Composite key identifying a cached preview entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Absolute path to the archive file.
    pub archive_path: String,
    /// Relative path of the entry within the archive.
    pub entry_path: String,
}

/// Internal cache entry storing the decode result and its byte size.
struct CacheEntry {
    result: DecodeResult,
    byte_size: usize,
}

/// LRU cache for decoded image previews.
///
/// Uses a `LinkedHashMap` to maintain insertion/access order for LRU eviction.
/// Total capacity is measured as the sum of all cached pixel buffer sizes.
pub struct PreviewCache {
    entries: LinkedHashMap<CacheKey, CacheEntry>,
    current_size: usize,
    max_size: usize,
}

impl PreviewCache {
    /// Creates a new `PreviewCache` with the default 256 MB capacity.
    pub fn new() -> Self {
        Self {
            entries: LinkedHashMap::new(),
            current_size: 0,
            max_size: DEFAULT_MAX_SIZE,
        }
    }

    /// Creates a new `PreviewCache` with a custom capacity (in bytes).
    /// Useful for testing with smaller limits.
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            entries: LinkedHashMap::new(),
            current_size: 0,
            max_size,
        }
    }

    /// Returns the current total cached size in bytes.
    pub fn current_size(&self) -> usize {
        self.current_size
    }

    /// Returns the maximum capacity in bytes.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Returns the number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Retrieves a cached decode result by key, promoting it to most-recently-used position.
    ///
    /// Returns `None` if the key is not in the cache.
    pub fn get(&mut self, key: &CacheKey) -> Option<&DecodeResult> {
        // get_refresh moves the entry to the back (most-recently-used)
        self.entries.get_refresh(key).map(|entry| &entry.result)
    }

    /// Inserts a decode result into the cache.
    ///
    /// - If the entry's pixel buffer exceeds `max_size`, it is not cached (returns silently).
    /// - If the key already exists, the old entry is removed first.
    /// - LRU entries are evicted until there is enough space for the new entry.
    pub fn insert(&mut self, key: CacheKey, value: DecodeResult) {
        let byte_size = value.pixels.len();

        // Skip caching if single entry exceeds max capacity
        if byte_size > self.max_size {
            return;
        }

        // If key already exists, remove old entry first
        if let Some(old_entry) = self.entries.remove(&key) {
            self.current_size -= old_entry.byte_size;
        }

        // Evict LRU entries until there is enough space
        while self.current_size + byte_size > self.max_size {
            if let Some((_evicted_key, evicted_entry)) = self.entries.pop_front() {
                self.current_size -= evicted_entry.byte_size;
            } else {
                // Cache is empty but still can't fit — shouldn't happen since we
                // already checked byte_size <= max_size, but guard anyway
                break;
            }
        }

        // Insert new entry at back (most-recently-used position)
        self.current_size += byte_size;
        self.entries.insert(
            key,
            CacheEntry {
                result: value,
                byte_size,
            },
        );
    }

    /// Removes all cached entries for a given archive path.
    ///
    /// Called when the user switches to a different archive.
    pub fn clear_archive(&mut self, archive_path: &str) {
        let keys_to_remove: Vec<CacheKey> = self
            .entries
            .keys()
            .filter(|k| k.archive_path == archive_path)
            .cloned()
            .collect();

        for key in keys_to_remove {
            if let Some(entry) = self.entries.remove(&key) {
                self.current_size -= entry.byte_size;
            }
        }
    }

    /// Removes all cached entries and resets the size counter.
    pub fn clear_all(&mut self) {
        self.entries.clear();
        self.current_size = 0;
    }
}

impl Default for PreviewCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a CacheKey.
    fn make_key(archive: &str, entry: &str) -> CacheKey {
        CacheKey {
            archive_path: archive.to_string(),
            entry_path: entry.to_string(),
        }
    }

    /// Helper to create a DecodeResult with a pixel buffer of the given size.
    fn make_result(size: usize, width: u32, height: u32) -> DecodeResult {
        DecodeResult {
            pixels: vec![0u8; size],
            width,
            height,
        }
    }

    #[test]
    fn test_new_cache_is_empty() {
        let cache = PreviewCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.current_size(), 0);
        assert_eq!(cache.max_size(), DEFAULT_MAX_SIZE);
    }

    #[test]
    fn test_insert_and_get() {
        let mut cache = PreviewCache::with_capacity(1024);
        let key = make_key("/archive.zip", "images/photo.png");
        let result = make_result(100, 5, 5);

        cache.insert(key.clone(), result.clone());

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.current_size(), 100);

        let retrieved = cache.get(&key).unwrap();
        assert_eq!(retrieved.width, 5);
        assert_eq!(retrieved.height, 5);
        assert_eq!(retrieved.pixels.len(), 100);
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let mut cache = PreviewCache::with_capacity(1024);
        let key = make_key("/archive.zip", "missing.png");
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_skip_caching_when_entry_exceeds_max_size() {
        let mut cache = PreviewCache::with_capacity(100);
        let key = make_key("/archive.zip", "huge.png");
        let result = make_result(101, 10, 10); // exceeds 100 byte capacity

        cache.insert(key.clone(), result);

        assert!(cache.is_empty());
        assert_eq!(cache.current_size(), 0);
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = PreviewCache::with_capacity(200);

        let key1 = make_key("/a.zip", "img1.png");
        let key2 = make_key("/a.zip", "img2.png");
        let key3 = make_key("/a.zip", "img3.png");

        cache.insert(key1.clone(), make_result(80, 4, 5));
        cache.insert(key2.clone(), make_result(80, 4, 5));
        // current_size = 160, fits within 200

        // Insert a third entry that requires eviction
        cache.insert(key3.clone(), make_result(80, 4, 5));
        // Need 80 more bytes, but only 40 available. Must evict key1 (LRU).

        assert!(cache.get(&key1).is_none()); // evicted
        assert!(cache.get(&key2).is_some()); // still present
        assert!(cache.get(&key3).is_some()); // just inserted
        assert!(cache.current_size() <= 200);
    }

    #[test]
    fn test_get_promotes_to_mru() {
        let mut cache = PreviewCache::with_capacity(200);

        let key1 = make_key("/a.zip", "img1.png");
        let key2 = make_key("/a.zip", "img2.png");
        let key3 = make_key("/a.zip", "img3.png");

        cache.insert(key1.clone(), make_result(80, 4, 5));
        cache.insert(key2.clone(), make_result(80, 4, 5));

        // Access key1 to promote it to MRU
        assert!(cache.get(&key1).is_some());

        // Now insert key3 which requires eviction — key2 should be evicted (it's now LRU)
        cache.insert(key3.clone(), make_result(80, 4, 5));

        assert!(cache.get(&key1).is_some()); // promoted, not evicted
        assert!(cache.get(&key2).is_none()); // evicted (was LRU)
        assert!(cache.get(&key3).is_some()); // just inserted
    }

    #[test]
    fn test_insert_existing_key_updates_entry() {
        let mut cache = PreviewCache::with_capacity(1024);
        let key = make_key("/a.zip", "img.png");

        cache.insert(key.clone(), make_result(100, 5, 5));
        assert_eq!(cache.current_size(), 100);

        // Re-insert with different size
        cache.insert(key.clone(), make_result(200, 10, 5));
        assert_eq!(cache.current_size(), 200);
        assert_eq!(cache.len(), 1);

        let retrieved = cache.get(&key).unwrap();
        assert_eq!(retrieved.width, 10);
        assert_eq!(retrieved.pixels.len(), 200);
    }

    #[test]
    fn test_clear_archive() {
        let mut cache = PreviewCache::with_capacity(1024);

        cache.insert(make_key("/a.zip", "img1.png"), make_result(50, 5, 5));
        cache.insert(make_key("/a.zip", "img2.png"), make_result(50, 5, 5));
        cache.insert(make_key("/b.zip", "img3.png"), make_result(50, 5, 5));

        assert_eq!(cache.len(), 3);
        assert_eq!(cache.current_size(), 150);

        cache.clear_archive("/a.zip");

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.current_size(), 50);
        assert!(cache.get(&make_key("/a.zip", "img1.png")).is_none());
        assert!(cache.get(&make_key("/a.zip", "img2.png")).is_none());
        assert!(cache.get(&make_key("/b.zip", "img3.png")).is_some());
    }

    #[test]
    fn test_clear_all() {
        let mut cache = PreviewCache::with_capacity(1024);

        cache.insert(make_key("/a.zip", "img1.png"), make_result(50, 5, 5));
        cache.insert(make_key("/b.zip", "img2.png"), make_result(50, 5, 5));

        cache.clear_all();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.current_size(), 0);
    }

    #[test]
    fn test_capacity_never_exceeded() {
        let mut cache = PreviewCache::with_capacity(500);

        // Insert many entries
        for i in 0..20 {
            let key = make_key("/a.zip", &format!("img{}.png", i));
            cache.insert(key, make_result(100, 5, 5));
            assert!(
                cache.current_size() <= 500,
                "Cache size {} exceeded max 500 after inserting entry {}",
                cache.current_size(),
                i
            );
        }
    }

    #[test]
    fn test_entry_exactly_at_max_size_is_cached() {
        let mut cache = PreviewCache::with_capacity(100);
        let key = make_key("/a.zip", "exact.png");
        let result = make_result(100, 10, 10); // exactly at limit

        cache.insert(key.clone(), result);

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.current_size(), 100);
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn test_multiple_evictions_for_large_entry() {
        let mut cache = PreviewCache::with_capacity(300);

        // Insert 3 small entries (100 bytes each = 300 total)
        cache.insert(make_key("/a.zip", "s1.png"), make_result(100, 5, 5));
        cache.insert(make_key("/a.zip", "s2.png"), make_result(100, 5, 5));
        cache.insert(make_key("/a.zip", "s3.png"), make_result(100, 5, 5));
        assert_eq!(cache.current_size(), 300);

        // Insert a 250-byte entry — must evict multiple entries
        cache.insert(make_key("/a.zip", "big.png"), make_result(250, 25, 10));

        assert!(cache.current_size() <= 300);
        assert!(cache.get(&make_key("/a.zip", "big.png")).is_some());
        // At least s1 and s2 should be evicted (need to free 250 bytes from 300)
        assert!(cache.get(&make_key("/a.zip", "s1.png")).is_none());
        assert!(cache.get(&make_key("/a.zip", "s2.png")).is_none());
    }

    #[test]
    fn test_clear_archive_nonexistent_is_noop() {
        let mut cache = PreviewCache::with_capacity(1024);
        cache.insert(make_key("/a.zip", "img.png"), make_result(50, 5, 5));

        cache.clear_archive("/nonexistent.zip");

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.current_size(), 50);
    }

    #[test]
    fn test_zero_size_entry() {
        let mut cache = PreviewCache::with_capacity(100);
        let key = make_key("/a.zip", "empty.png");
        let result = make_result(0, 0, 0);

        cache.insert(key.clone(), result);

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.current_size(), 0);
        assert!(cache.get(&key).is_some());
    }
}
