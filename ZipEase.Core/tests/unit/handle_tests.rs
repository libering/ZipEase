// Handle unit tests
// Tests for LockHandle type-safe wrapper

use zipease_core::lock::LockHandle;
use std::collections::{HashMap, HashSet};

#[test]
fn test_create_valid_handle() {
    let handle = LockHandle::new(1);
    assert_eq!(handle.id(), 1);
    assert_eq!(handle.as_raw(), 1);
}

#[test]
fn test_create_handle_with_large_id() {
    let large_id = 999_999_999;
    let handle = LockHandle::new(large_id);
    assert_eq!(handle.id(), large_id);
    assert_eq!(handle.as_raw(), large_id as isize);
}

#[test]
fn test_from_raw_valid_positive() {
    let raw = 42;
    let handle = LockHandle::from_raw(raw);
    
    assert!(handle.is_some());
    assert_eq!(handle.unwrap().as_raw(), raw);
}

#[test]
fn test_from_raw_invalid_negative_one() {
    // -1 is INVALID_HANDLE_VALUE
    let handle = LockHandle::from_raw(-1);
    assert!(handle.is_none());
}

#[test]
fn test_from_raw_invalid_zero() {
    // 0 is also invalid
    let handle = LockHandle::from_raw(0);
    assert!(handle.is_none());
}

#[test]
fn test_from_raw_invalid_negative() {
    // Any negative value should be invalid
    let invalid_values = vec![-1, -2, -10, -100, -999];
    
    for raw in invalid_values {
        let handle = LockHandle::from_raw(raw);
        assert!(handle.is_none(), "Expected None for raw value {}", raw);
    }
}

#[test]
fn test_handle_equality() {
    let handle1 = LockHandle::new(42);
    let handle2 = LockHandle::new(42);
    let handle3 = LockHandle::new(43);
    
    // Same ID should be equal
    assert_eq!(handle1, handle2);
    
    // Different ID should not be equal
    assert_ne!(handle1, handle3);
    assert_ne!(handle2, handle3);
}

#[test]
fn test_handle_clone() {
    let original = LockHandle::new(100);
    let cloned = original.clone();
    
    assert_eq!(original, cloned);
    assert_eq!(original.id(), cloned.id());
    assert_eq!(original.as_raw(), cloned.as_raw());
}

#[test]
fn test_handle_copy_semantics() {
    let handle1 = LockHandle::new(50);
    let handle2 = handle1; // This is a copy, not a move
    
    // Both should still be usable (proving Copy trait works)
    assert_eq!(handle1.as_raw(), 50);
    assert_eq!(handle2.as_raw(), 50);
    assert_eq!(handle1, handle2);
}

#[test]
fn test_handle_in_hashmap() {
    let mut map = HashMap::new();
    
    let handle1 = LockHandle::new(1);
    let handle2 = LockHandle::new(2);
    let handle3 = LockHandle::new(3);
    
    map.insert(handle1, "first");
    map.insert(handle2, "second");
    map.insert(handle3, "third");
    
    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&handle1), Some(&"first"));
    assert_eq!(map.get(&handle2), Some(&"second"));
    assert_eq!(map.get(&handle3), Some(&"third"));
}

#[test]
fn test_handle_in_hashset() {
    let mut set = HashSet::new();
    
    let handle1 = LockHandle::new(1);
    let handle2 = LockHandle::new(2);
    let handle3 = LockHandle::new(1); // Duplicate of handle1
    
    set.insert(handle1);
    set.insert(handle2);
    set.insert(handle3); // Should not add duplicate
    
    assert_eq!(set.len(), 2);
    assert!(set.contains(&handle1));
    assert!(set.contains(&handle2));
    assert!(set.contains(&handle3)); // Same as handle1
}

#[test]
fn test_handle_hash_consistency() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let handle1 = LockHandle::new(42);
    let handle2 = LockHandle::new(42);
    
    let mut hasher1 = DefaultHasher::new();
    let mut hasher2 = DefaultHasher::new();
    
    handle1.hash(&mut hasher1);
    handle2.hash(&mut hasher2);
    
    // Same handles should produce same hash
    assert_eq!(hasher1.finish(), hasher2.finish());
}

#[test]
fn test_handle_debug_format() {
    let handle = LockHandle::new(123);
    let debug_str = format!("{:?}", handle);
    
    // Debug output should contain the type name and value
    assert!(debug_str.contains("LockHandle"));
    assert!(debug_str.contains("123"));
}

#[test]
fn test_invalid_constant_value() {
    assert_eq!(LockHandle::INVALID, -1);
}

#[test]
fn test_roundtrip_conversion() {
    // Test that we can convert to raw and back without loss
    let original_ids = vec![1, 42, 100, 999, 12345, 999999];
    
    for id in original_ids {
        let handle = LockHandle::new(id);
        let raw = handle.as_raw();
        let restored = LockHandle::from_raw(raw);
        
        assert!(restored.is_some());
        assert_eq!(restored.unwrap(), handle);
        assert_eq!(restored.unwrap().id(), id);
    }
}

#[test]
fn test_handle_ordering_in_collections() {
    let handles = vec![
        LockHandle::new(3),
        LockHandle::new(1),
        LockHandle::new(2),
    ];
    
    // Handles should be sortable (they implement Eq and Hash)
    // We can at least verify they can be stored and retrieved
    let set: HashSet<_> = handles.iter().copied().collect();
    assert_eq!(set.len(), 3);
}

#[test]
fn test_multiple_handles_independence() {
    let handle1 = LockHandle::new(1);
    let handle2 = LockHandle::new(2);
    let handle3 = LockHandle::new(3);
    
    // Each handle should maintain its own ID
    assert_eq!(handle1.id(), 1);
    assert_eq!(handle2.id(), 2);
    assert_eq!(handle3.id(), 3);
    
    // They should all be different
    assert_ne!(handle1, handle2);
    assert_ne!(handle2, handle3);
    assert_ne!(handle1, handle3);
}

#[test]
fn test_handle_with_max_safe_value() {
    // Test with a large but safe value
    let large_id = (isize::MAX / 2) as u64;
    let handle = LockHandle::new(large_id);
    
    assert_eq!(handle.id(), large_id);
    
    let raw = handle.as_raw();
    assert!(raw > 0);
    
    let restored = LockHandle::from_raw(raw);
    assert!(restored.is_some());
    assert_eq!(restored.unwrap(), handle);
}

