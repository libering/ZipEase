use zipease_core::lock::LockHandle;
use std::collections::HashSet;

#[test]
fn test_create_valid_handle() {
    let handle = LockHandle::new(1);
    assert_eq!(handle.id(), 1);
    assert_eq!(handle.as_raw(), 1);
}

#[test]
fn test_from_raw_valid_positive() {
    let raw = 42;
    let handle = LockHandle::from_raw(raw);
    assert!(handle.is_some());
    assert_eq!(handle.unwrap().as_raw(), raw);
}

#[test]
fn test_from_raw_invalid_values() {
    assert!(LockHandle::from_raw(-1).is_none());
    assert!(LockHandle::from_raw(0).is_none());
    assert!(LockHandle::from_raw(-100).is_none());
}

#[test]
fn test_handle_equality_and_hash() {
    let handle1 = LockHandle::new(42);
    let handle2 = LockHandle::new(42);
    let handle3 = LockHandle::new(43);

    assert_eq!(handle1, handle2);
    assert_ne!(handle1, handle3);

    let mut set = HashSet::new();
    set.insert(handle1);
    set.insert(handle2);
    set.insert(handle3);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_roundtrip_conversion() {
    let original = LockHandle::new(12345);
    let raw = original.as_raw();
    let restored = LockHandle::from_raw(raw);
    assert!(restored.is_some());
    assert_eq!(restored.unwrap(), original);
}
