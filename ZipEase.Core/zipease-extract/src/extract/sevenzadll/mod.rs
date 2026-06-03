//! `sevenzadll` module — COM-based 7za.dll extraction backend.
//!
//! Sub-module layout:
//!   - `types`    — Primitive aliases, GUID, PROPVARIANT, vtable struct definitions, constants
//!   - `stream`   — RustInStream, RustOutStream, their vtable functions and static vtables
//!   - `callback` — RustExtractCallback, ErasedCallback, callback vtable functions
//!   - `backend`  — SevenZaDllBackend, SevenZaDllBackendWithClsid, ExtractionBackend impls

#![allow(non_snake_case, non_camel_case_types, dead_code)]

mod types;
mod stream;
mod callback;
mod backend;

// Re-exports to preserve the public API surface:
pub use backend::{SevenZaDllBackend, SevenZaDllBackendWithClsid};
pub use types::{
    GUID, CLSID_7Z_HANDLER, CLSID_ZIP_HANDLER, CLSID_RAR_HANDLER,
    CLSID_XZ_HANDLER, CLSID_LZMA_HANDLER, CLSID_WIM_HANDLER, CLSID_VHD_HANDLER
};

use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;

use zipease_shared::LockError;
use types::*;

// ─── CreateObject function type ───────────────────────────────────────────────

pub(crate) type CreateObjectFn = unsafe extern "system" fn(
    class_id:     *const GUID,
    interface_id: *const GUID,
    out_object:   *mut *mut std::ffi::c_void,
) -> HRESULT;

// ─── RAII wrappers ────────────────────────────────────────────────────────────

/// RAII guard for an `IInArchive` COM pointer.
pub(crate) struct InArchiveGuard {
    pub ptr: *mut IInArchive,
}

unsafe impl Send for InArchiveGuard {}

impl Drop for InArchiveGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                let vtbl = &*(*self.ptr).vtbl;
                let _ = (vtbl.close)(self.ptr);
                (vtbl.release)(self.ptr);
            }
        }
    }
}

/// RAII guard for a `PROPVARIANT`.
pub(crate) struct PropVariantGuard(pub PROPVARIANT);

extern "system" {
    fn SysFreeString(bstr: *mut u16);
}

impl Drop for PropVariantGuard {
    fn drop(&mut self) {
        if self.0.vt == VT_BSTR {
            unsafe {
                let bstr = ptr::read_unaligned(self.0.data.as_ptr() as *const BSTR);
                if !bstr.is_null() {
                    SysFreeString(bstr);
                }
            }
        }
    }
}

// ─── DLL path resolution ──────────────────────────────────────────────────────

/// Resolves the path to `7za.dll` relative to `zipease_core.dll`.
pub fn resolve_dll_path() -> Result<PathBuf, LockError> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetModuleFileNameW};
    use windows::core::PCWSTR;

    let dir = unsafe {
        let name: Vec<u16> = "zipease_core.dll\0".encode_utf16().collect();
        let hmod = GetModuleHandleW(PCWSTR(name.as_ptr()));

        if let Ok(hmod) = hmod {
            let mut buf = vec![0u16; 32768];
            let len = GetModuleFileNameW(hmod, &mut buf) as usize;
            if len > 0 {
                let path = std::ffi::OsString::from_wide(&buf[..len]);
                PathBuf::from(path)
                    .parent()
                    .map(|p| p.to_path_buf())
            } else {
                None
            }
        } else {
            None
        }
    };

    let dir = dir.or_else(|| {
        std::env::current_exe().ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    });

    let dir = dir.ok_or_else(|| {
        LockError::PluginRequired("Cannot determine application directory for 7za.dll".into())
    })?;

    let dll_path = dir.join("7za.dll");

    if !dll_path.exists() {
        return Err(LockError::PluginRequired(format!(
            "7za.dll not found in application directory: {}",
            dll_path.display()
        )));
    }

    Ok(dll_path)
}

// ─── Helper: wide string to Rust String ──────────────────────────────────────

/// Reads a null-terminated UTF-16 string from `ptr` and converts to `String`.
pub(crate) fn wide_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16_lossy(slice)
    }
}
