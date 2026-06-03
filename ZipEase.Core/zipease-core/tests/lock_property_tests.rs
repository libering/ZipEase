use proptest::prelude::*;
use std::fs;
use tempfile::tempdir;
use zipease_core::lock::manager::LOCK_MANAGER;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))] // Reduce cases for expensive FS operations

    /// Property 1: Successful lock returns a valid handle
    #[test]
    fn property_lock_returns_valid_handle(_ in 0..10) {
        let temp = tempdir().unwrap();
        let path = temp.path().to_path_buf();
        
        let result = LOCK_MANAGER.lock_directory(path);
        prop_assert!(result.is_ok());
        
        let handle = result.unwrap();
        prop_assert!(handle.as_raw() > 0);
        
        // Cleanup
        LOCK_MANAGER.unlock_directory(handle).unwrap();
    }

    /// Property 2: Lock prevents destructive operations (delete/rename)
    #[test]
    fn property_lock_prevents_destructive_ops(_ in 0..10) {
        let temp = tempdir().unwrap();
        let path = temp.path().to_path_buf();
        
        let handle = LOCK_MANAGER.lock_directory(path.clone()).unwrap();
        
        // Try to delete - should fail on Windows due to the lock
        let delete_res = fs::remove_dir(&path);
        prop_assert!(delete_res.is_err());
        
        // Try to rename - should fail
        let mut new_path = path.clone();
        new_path.set_extension("renamed");
        let rename_res = fs::rename(&path, &new_path);
        prop_assert!(rename_res.is_err());
        
        // Cleanup
        LOCK_MANAGER.unlock_directory(handle).unwrap();
        
        // After unlock, delete should work
        let delete_after = fs::remove_dir(&path);
        prop_assert!(delete_after.is_ok());
    }

    /// Property 3: Lock allows read operations
    #[test]
    fn property_lock_allows_read_ops(_ in 0..10) {
        let temp = tempdir().unwrap();
        let path = temp.path().to_path_buf();
        
        // Create a file inside to read
        let file_path = path.join("test.txt");
        fs::write(&file_path, "content").unwrap();
        
        let handle = LOCK_MANAGER.lock_directory(path.clone()).unwrap();
        
        // Read directory
        let read_dir = fs::read_dir(&path);
        prop_assert!(read_dir.is_ok());
        
        // Read file inside
        let read_file = fs::read_to_string(&file_path);
        prop_assert_eq!(read_file.unwrap(), "content");
        
        // Cleanup
        LOCK_MANAGER.unlock_directory(handle).unwrap();
    }

    /// Property 4: Multiple locks on same directory are independent
    #[test]
    fn property_multiple_locks_independent(_ in 0..10) {
        let temp = tempdir().unwrap();
        let path = temp.path().to_path_buf();
        
        let handle1 = LOCK_MANAGER.lock_directory(path.clone()).unwrap();
        let handle2 = LOCK_MANAGER.lock_directory(path.clone()).unwrap();
        
        prop_assert_ne!(handle1, handle2);
        
        // Unlock one, directory should still be locked by handle2
        LOCK_MANAGER.unlock_directory(handle1).unwrap();
        let delete_res = fs::remove_dir(&path);
        prop_assert!(delete_res.is_err());
        
        // Unlock second, directory should be free
        LOCK_MANAGER.unlock_directory(handle2).unwrap();
        let delete_after = fs::remove_dir(&path);
        prop_assert!(delete_after.is_ok());
    }
}
