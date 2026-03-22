use zipease_shared::LockError;
use crate::lock::handle::LockHandle;
use crate::platform::WindowsDirectoryLock;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
struct SendableLock(WindowsDirectoryLock);
unsafe impl Send for SendableLock {}
impl SendableLock {
    fn new(lock: WindowsDirectoryLock) -> Self { Self(lock) }
}

pub struct LockManager {
    locks: Arc<Mutex<HashMap<LockHandle, (SendableLock, PathBuf)>>>,
    next_id: Arc<Mutex<u64>>,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    pub fn lock_directory(&self, path: PathBuf) -> Result<LockHandle, LockError> {
        let lock = WindowsDirectoryLock::lock(&path)?;
        let handle_id = {
            let mut next_id = self.next_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };
        let handle = LockHandle::new(handle_id);
        self.locks.lock().unwrap().insert(handle, (SendableLock::new(lock), path));
        Ok(handle)
    }

    pub fn unlock_directory(&self, handle: LockHandle) -> Result<(), LockError> {
        let mut locks = self.locks.lock().unwrap();
        if locks.remove(&handle).is_some() {
            Ok(())
        } else {
            Err(LockError::InvalidHandle)
        }
    }

    pub fn lock_count(&self) -> usize {
        self.locks.lock().unwrap().len()
    }
}

impl Default for LockManager {
    fn default() -> Self { Self::new() }
}

pub static LOCK_MANAGER: Lazy<LockManager> = Lazy::new(LockManager::new);
