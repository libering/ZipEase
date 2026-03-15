// Lock handle implementation
// Provides type-safe wrapper for directory lock handles

/// A type-safe handle for directory locks
/// 
/// This handle is used to identify and manage directory locks. It wraps
/// a u64 ID internally and provides conversion to/from raw isize values
/// for FFI compatibility.
/// 
/// # FFI Compatibility
/// The handle can be converted to/from isize for use across the FFI boundary.
/// Invalid handles are represented as -1 (INVALID_HANDLE_VALUE in Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LockHandle(u64);

impl LockHandle {
    /// The invalid handle constant, compatible with Windows INVALID_HANDLE_VALUE
    pub const INVALID: isize = -1;
    
    /// Creates a new lock handle from a u64 ID
    /// 
    /// # Arguments
    /// * `id` - The unique identifier for this lock
    /// 
    /// # Returns
    /// A new LockHandle wrapping the given ID
    /// 
    /// # Example
    /// ```
    /// use zipease_core::lock::LockHandle;
    /// let handle = LockHandle::new(42);
    /// assert_eq!(handle.as_raw(), 42);
    /// ```
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Converts the handle to a raw isize value for FFI
    /// 
    /// This is used when passing the handle across the FFI boundary to C#.
    /// 
    /// # Returns
    /// The handle ID as an isize
    /// 
    /// # Example
    /// ```
    /// use zipease_core::lock::LockHandle;
    /// let handle = LockHandle::new(100);
    /// assert_eq!(handle.as_raw(), 100);
    /// ```
    pub fn as_raw(&self) -> isize {
        self.0 as isize
    }
    
    /// Creates a handle from a raw isize value received from FFI
    /// 
    /// This validates that the raw value represents a valid handle.
    /// Values <= 0 are considered invalid and return None.
    /// 
    /// # Arguments
    /// * `raw` - The raw isize value from FFI
    /// 
    /// # Returns
    /// * `Some(LockHandle)` - If the raw value is valid (> 0)
    /// * `None` - If the raw value is invalid (<= 0)
    /// 
    /// # Example
    /// ```
    /// use zipease_core::lock::LockHandle;
    /// 
    /// // Valid handle
    /// let handle = LockHandle::from_raw(42);
    /// assert!(handle.is_some());
    /// assert_eq!(handle.unwrap().as_raw(), 42);
    /// 
    /// // Invalid handles
    /// assert!(LockHandle::from_raw(-1).is_none());
    /// assert!(LockHandle::from_raw(0).is_none());
    /// assert!(LockHandle::from_raw(-100).is_none());
    /// ```
    pub fn from_raw(raw: isize) -> Option<Self> {
        if raw <= 0 {
            None
        } else {
            Some(Self(raw as u64))
        }
    }
    
    /// Returns the internal ID value
    /// 
    /// This is primarily used for internal operations and debugging.
    /// 
    /// # Returns
    /// The internal u64 ID
    pub fn id(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_handle() {
        let handle = LockHandle::new(42);
        assert_eq!(handle.id(), 42);
        assert_eq!(handle.as_raw(), 42);
    }

    #[test]
    fn test_from_raw_valid() {
        let handle = LockHandle::from_raw(100);
        assert!(handle.is_some());
        assert_eq!(handle.unwrap().as_raw(), 100);
    }

    #[test]
    fn test_from_raw_invalid() {
        // Test invalid handle value (-1)
        assert!(LockHandle::from_raw(-1).is_none());
        
        // Test zero
        assert!(LockHandle::from_raw(0).is_none());
        
        // Test negative values
        assert!(LockHandle::from_raw(-100).is_none());
        assert!(LockHandle::from_raw(-999).is_none());
    }

    #[test]
    fn test_handle_equality() {
        let handle1 = LockHandle::new(42);
        let handle2 = LockHandle::new(42);
        let handle3 = LockHandle::new(43);
        
        assert_eq!(handle1, handle2);
        assert_ne!(handle1, handle3);
    }

    #[test]
    fn test_handle_clone() {
        let handle1 = LockHandle::new(42);
        let handle2 = handle1.clone();
        
        assert_eq!(handle1, handle2);
        assert_eq!(handle1.as_raw(), handle2.as_raw());
    }

    #[test]
    fn test_handle_copy() {
        let handle1 = LockHandle::new(42);
        let handle2 = handle1; // Copy, not move
        
        // Both should still be usable
        assert_eq!(handle1.as_raw(), 42);
        assert_eq!(handle2.as_raw(), 42);
    }

    #[test]
    fn test_handle_hash() {
        use std::collections::HashSet;
        
        let mut set = HashSet::new();
        let handle1 = LockHandle::new(1);
        let handle2 = LockHandle::new(2);
        let handle3 = LockHandle::new(1); // Same as handle1
        
        set.insert(handle1);
        set.insert(handle2);
        set.insert(handle3); // Should not add duplicate
        
        assert_eq!(set.len(), 2);
        assert!(set.contains(&handle1));
        assert!(set.contains(&handle2));
    }

    #[test]
    fn test_handle_debug_format() {
        let handle = LockHandle::new(42);
        let debug_str = format!("{:?}", handle);
        
        assert!(debug_str.contains("LockHandle"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_invalid_constant() {
        assert_eq!(LockHandle::INVALID, -1);
    }

    #[test]
    fn test_large_handle_values() {
        // Test with large u64 values
        let large_id = u64::MAX / 2;
        let handle = LockHandle::new(large_id);
        
        assert_eq!(handle.id(), large_id);
        assert_eq!(handle.as_raw(), large_id as isize);
    }

    #[test]
    fn test_roundtrip_conversion() {
        // Test that converting to raw and back preserves the value
        let original = LockHandle::new(12345);
        let raw = original.as_raw();
        let restored = LockHandle::from_raw(raw);
        
        assert!(restored.is_some());
        assert_eq!(restored.unwrap(), original);
    }
}
