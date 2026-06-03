//! Backend implementations for the 7za.dll extraction.
//!
//! Contains: `SevenZaDllBackend`, `SevenZaDllBackendWithClsid`,
//! their `ExtractionBackend` trait impls, and `extract_single_entry`.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;
use std::path::Path;
use std::ptr;

use libloading::{Library, Symbol};

use zipease_shared::LockError;

use super::super::{ArchiveEntryInfo, ExtractionBackend};
use super::types::*;
use super::stream::RustInStream;
use super::callback::{
    ErasedCallback, ErasedCallback2, SingleCb, call_progress, call_progress2,
    ERASED_VTBL, EC2_VTBL, SC_VTBL,
};
use super::{resolve_dll_path, wide_to_string, InArchiveGuard, PropVariantGuard, CreateObjectFn};

// ─── SevenZaDllBackend ────────────────────────────────────────────────────────

/// Zero-sized backend that loads `7za.dll` per operation call.
pub struct SevenZaDllBackend;

/// Backend that uses a specific CLSID — for split archives and non-RAR formats via 7za.dll.
pub struct SevenZaDllBackendWithClsid(pub GUID);

impl ExtractionBackend for SevenZaDllBackend {
    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        let dll_path = resolve_dll_path()?;
        let lib = unsafe {
            Library::new(&dll_path)
                .map_err(|e| LockError::PluginRequired(format!("Cannot load 7za.dll: {}", e)))?
        };
        let create_object: Symbol<CreateObjectFn> = unsafe {
            lib.get(b"CreateObject\0")
                .map_err(|_| LockError::PluginRequired(
                    "7za.dll does not export CreateObject — incompatible or corrupt DLL".into(),
                ))?
        };

        let mut archive_ptr: *mut IInArchive = ptr::null_mut();
        let hr = unsafe {
            create_object(
                &CLSID_RAR_HANDLER, &IID_IIN_ARCHIVE,
                &mut archive_ptr as *mut *mut IInArchive as *mut *mut c_void,
            )
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!("CreateObject HRESULT {:#x}", hr)));
        }
        let _guard = InArchiveGuard { ptr: archive_ptr };

        let stream = RustInStream::new(archive_path)?;
        let stream_ptr = stream.as_ptr();
        let _ = Box::into_raw(stream);

        let hr = unsafe {
            ((*(*archive_ptr).vtbl).open)(archive_ptr, stream_ptr, ptr::null(), ptr::null_mut())
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!("IInArchive::Open HRESULT {:#x}", hr)));
        }

        let mut count: u32 = 0;
        unsafe { ((*(*archive_ptr).vtbl).get_number_of_items)(archive_ptr, &mut count); }
        crate::zlog(&format!("[zipease-debug] SevenZaDllBackend::list_entries count={}", count));

        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut pv_dir = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_IS_DIR, &mut pv_dir.0);
            }
            let is_dir = pv_dir.0.vt == VT_BOOL && unsafe {
                ptr::read_unaligned(pv_dir.0.data.as_ptr() as *const i16) != 0
            };
            crate::zlog(&format!("[zipease-debug]   entry[{}] vt_dir={} is_dir={}", i, pv_dir.0.vt, is_dir));
            if is_dir { continue; }

            let mut pv = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_PATH, &mut pv.0);
            }
            crate::zlog(&format!("[zipease-debug]   entry[{}] vt_path={}", i, pv.0.vt));
            if pv.0.vt == VT_BSTR {
                let bstr = unsafe { ptr::read_unaligned(pv.0.data.as_ptr() as *const *const u16) };
                let name = wide_to_string(bstr);
                crate::zlog(&format!("[zipease-debug]   entry[{}] name={:?}", i, name));
                entries.push(name);
            }
        }
        Ok(entries)
    }

    fn extract_with_progress<F>(
        &self,
        archive_path: &Path,
        output_dir: &Path,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str),
    {
        let dll_path = resolve_dll_path()?;
        let lib = unsafe {
            Library::new(&dll_path)
                .map_err(|e| LockError::PluginRequired(format!("Cannot load 7za.dll: {}", e)))?
        };
        let create_object: Symbol<CreateObjectFn> = unsafe {
            lib.get(b"CreateObject\0")
                .map_err(|_| LockError::PluginRequired(
                    "7za.dll does not export CreateObject — incompatible or corrupt DLL".into(),
                ))?
        };

        let mut archive_ptr: *mut IInArchive = ptr::null_mut();
        let hr = unsafe {
            create_object(
                &CLSID_RAR_HANDLER, &IID_IIN_ARCHIVE,
                &mut archive_ptr as *mut *mut IInArchive as *mut *mut c_void,
            )
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!("CreateObject HRESULT {:#x}", hr)));
        }
        let _guard = InArchiveGuard { ptr: archive_ptr };

        let stream = RustInStream::new(archive_path)?;
        let stream_ptr = stream.as_ptr();
        let _ = Box::into_raw(stream);

        let hr = unsafe {
            ((*(*archive_ptr).vtbl).open)(archive_ptr, stream_ptr, ptr::null(), ptr::null_mut())
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!("IInArchive::Open HRESULT {:#x}", hr)));
        }

        let mut count: u32 = 0;
        unsafe { ((*(*archive_ptr).vtbl).get_number_of_items)(archive_ptr, &mut count); }
        crate::zlog(&format!("[zipease-debug] SevenZaDllBackend::extract_with_progress count={}", count));
        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut pv = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_PATH, &mut pv.0);
            }
            let name = if pv.0.vt == VT_BSTR {
                let bstr = unsafe { ptr::read_unaligned(pv.0.data.as_ptr() as *const *const u16) };
                wide_to_string(bstr)
            } else {
                crate::zlog(&format!("[zipease-debug]   extract entry[{}] vt_path={} (not VT_BSTR)", i, pv.0.vt));
                String::new()
            };
            entries.push(name);
        }

        let mut callback = ErasedCallback {
            vtbl:           &ERASED_VTBL,
            output_dir:     output_dir.to_path_buf(),
            entries,
            progress_call:  call_progress::<F>,
            progress_data:  &progress_fn as *const F as *const (),
            current:        std::cell::Cell::new(0),
            current_stream: std::cell::Cell::new(ptr::null_mut()),
            error:          std::cell::Cell::new(None),
        };

        let callback_ptr = &mut callback as *mut ErasedCallback as *mut IArchiveExtractCallback;
        let hr = unsafe {
            ((*(*archive_ptr).vtbl).extract)(archive_ptr, ptr::null(), 0xFFFFFFFF, 0, callback_ptr)
        };
        if hr != S_OK && hr != S_FALSE {
            return Err(LockError::ExtractionFailed(format!("IInArchive::Extract HRESULT {:#x}", hr)));
        }
        if let Some(err) = callback.error.take() { return Err(err); }
        Ok(())
    }

    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }
}

// ─── SevenZaDllBackend::extract_single_entry ──────────────────────────────────

impl SevenZaDllBackend {
    /// Extract a single entry by index using IInArchive::Extract([index], 1, ...).
    pub fn extract_single_entry(
        &self,
        archive_path: &Path,
        entry_index: u32,
        output_dir: &Path,
    ) -> Result<(), LockError> {
        let dll_path = resolve_dll_path()?;
        let lib = unsafe {
            Library::new(&dll_path)
                .map_err(|e| LockError::PluginRequired(format!("Cannot load 7za.dll: {}", e)))?
        };
        let create_object: Symbol<CreateObjectFn> = unsafe {
            lib.get(b"CreateObject\0")
                .map_err(|_| LockError::PluginRequired("7za.dll does not export CreateObject".into()))?
        };
        let ext = archive_path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        let clsid = match ext.as_str() {
            "7z" => CLSID_7Z_HANDLER,
            "zip" | "apk" | "jar" | "ipa" => CLSID_ZIP_HANDLER,
            _ => CLSID_RAR_HANDLER,
        };

        let mut archive_ptr: *mut IInArchive = ptr::null_mut();
        let hr = unsafe {
            create_object(
                &clsid, &IID_IIN_ARCHIVE,
                &mut archive_ptr as *mut *mut IInArchive as *mut *mut c_void,
            )
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!("CreateObject HRESULT {:#x}", hr)));
        }
        let _guard = InArchiveGuard { ptr: archive_ptr };

        let stream = RustInStream::new(archive_path)?;
        let stream_ptr = stream.as_ptr();
        let _ = Box::into_raw(stream);
        let hr = unsafe {
            ((*(*archive_ptr).vtbl).open)(archive_ptr, stream_ptr, ptr::null(), ptr::null_mut())
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!("IInArchive::Open HRESULT {:#x}", hr)));
        }

        // Get entry name
        let mut pv = PropVariantGuard(PROPVARIANT::zeroed());
        unsafe {
            ((*(*archive_ptr).vtbl).get_property)(archive_ptr, entry_index, KPID_PATH, &mut pv.0);
        }
        let entry_name = if pv.0.vt == VT_BSTR {
            let bstr = unsafe { ptr::read_unaligned(pv.0.data.as_ptr() as *const *const u16) };
            wide_to_string(bstr)
        } else {
            format!("entry_{}", entry_index)
        };

        let mut cb = SingleCb {
            vtbl:           &SC_VTBL,
            output_dir:     output_dir.to_path_buf(),
            entry_name,
            current_stream: std::cell::Cell::new(ptr::null_mut()),
            error:          std::cell::Cell::new(None),
        };
        let cb_ptr = &mut cb as *mut SingleCb as *mut IArchiveExtractCallback;
        let indices = [entry_index];
        let hr = unsafe {
            ((*(*archive_ptr).vtbl).extract)(archive_ptr, indices.as_ptr(), 1, 0, cb_ptr)
        };
        if hr != S_OK && hr != S_FALSE {
            return Err(LockError::ExtractionFailed(format!("Extract HRESULT {:#x}", hr)));
        }
        if let Some(err) = cb.error.take() { return Err(err); }
        Ok(())
    }
}

// ─── SevenZaDllBackendWithClsid ───────────────────────────────────────────────

impl SevenZaDllBackendWithClsid {
    /// Internal helper: open archive with a given CLSID.
    fn open_archive(
        &self, archive_path: &Path, lib: &Library,
    ) -> Result<(*mut IInArchive, InArchiveGuard), LockError> {
        let create_object: Symbol<CreateObjectFn> = unsafe {
            lib.get(b"CreateObject\0")
                .map_err(|_| LockError::PluginRequired("7za.dll does not export CreateObject".into()))?
        };
        let mut archive_ptr: *mut IInArchive = ptr::null_mut();
        let hr = unsafe {
            create_object(
                &self.0, &IID_IIN_ARCHIVE,
                &mut archive_ptr as *mut *mut IInArchive as *mut *mut c_void,
            )
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!("CreateObject HRESULT {:#x}", hr)));
        }
        let guard = InArchiveGuard { ptr: archive_ptr };

        let stream = RustInStream::new(archive_path)?;
        let stream_ptr = stream.as_ptr();
        let _ = Box::into_raw(stream);
        let hr = unsafe {
            ((*(*archive_ptr).vtbl).open)(archive_ptr, stream_ptr, ptr::null(), ptr::null_mut())
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!("IInArchive::Open HRESULT {:#x}", hr)));
        }
        Ok((archive_ptr, guard))
    }
}

// ─── ExtractionBackend impl for SevenZaDllBackendWithClsid ────────────────────

impl ExtractionBackend for SevenZaDllBackendWithClsid {
    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        let dll_path = resolve_dll_path()?;
        let lib = unsafe {
            Library::new(&dll_path)
                .map_err(|e| LockError::PluginRequired(format!("Cannot load 7za.dll: {}", e)))?
        };
        let (archive_ptr, _guard) = self.open_archive(archive_path, &lib)?;

        let mut count: u32 = 0;
        unsafe { ((*(*archive_ptr).vtbl).get_number_of_items)(archive_ptr, &mut count); }

        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut pv_dir = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_IS_DIR, &mut pv_dir.0);
            }
            if pv_dir.0.vt == VT_BOOL {
                let val = unsafe { ptr::read_unaligned(pv_dir.0.data.as_ptr() as *const i16) };
                if val != 0 { continue; }
            }
            let mut pv = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_PATH, &mut pv.0);
            }
            if pv.0.vt == VT_BSTR {
                let bstr = unsafe { ptr::read_unaligned(pv.0.data.as_ptr() as *const *const u16) };
                entries.push(wide_to_string(bstr));
            }
        }
        Ok(entries)
    }

    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
        let dll_path = resolve_dll_path()?;
        let lib = unsafe {
            Library::new(&dll_path)
                .map_err(|e| LockError::PluginRequired(format!("Cannot load 7za.dll: {}", e)))?
        };
        let (archive_ptr, _guard) = self.open_archive(archive_path, &lib)?;

        let mut count: u32 = 0;
        unsafe { ((*(*archive_ptr).vtbl).get_number_of_items)(archive_ptr, &mut count); }

        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut pv_dir = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_IS_DIR, &mut pv_dir.0);
            }
            let is_dir = pv_dir.0.vt == VT_BOOL && unsafe {
                ptr::read_unaligned(pv_dir.0.data.as_ptr() as *const i16) != 0
            };
            let mut pv = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_PATH, &mut pv.0);
            }
            let name = if pv.0.vt == VT_BSTR {
                let bstr = unsafe { ptr::read_unaligned(pv.0.data.as_ptr() as *const *const u16) };
                wide_to_string(bstr)
            } else { String::new() };

            let size: i64 = if is_dir {
                -1
            } else {
                let mut pv_size = PropVariantGuard(PROPVARIANT::zeroed());
                unsafe {
                    ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_SIZE, &mut pv_size.0);
                }
                if pv_size.0.vt == VT_UI8 {
                    unsafe { ptr::read_unaligned(pv_size.0.data.as_ptr() as *const u64) as i64 }
                } else {
                    -1
                }
            };

            entries.push(ArchiveEntryInfo { name, is_directory: is_dir, size });
        }
        Ok(entries)
    }

    fn extract_with_progress<F>(
        &self,
        archive_path: &Path,
        output_dir: &Path,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str),
    {
        let dll_path = resolve_dll_path()?;
        let lib = unsafe {
            Library::new(&dll_path)
                .map_err(|e| LockError::PluginRequired(format!("Cannot load 7za.dll: {}", e)))?
        };
        let (archive_ptr, _guard) = self.open_archive(archive_path, &lib)?;

        let mut count: u32 = 0;
        unsafe { ((*(*archive_ptr).vtbl).get_number_of_items)(archive_ptr, &mut count); }

        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut pv = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(archive_ptr, i, KPID_PATH, &mut pv.0);
            }
            let name = if pv.0.vt == VT_BSTR {
                let bstr = unsafe { ptr::read_unaligned(pv.0.data.as_ptr() as *const *const u16) };
                wide_to_string(bstr)
            } else { String::new() };
            entries.push(name);
        }

        let mut callback = ErasedCallback2 {
            vtbl:           &EC2_VTBL,
            output_dir:     output_dir.to_path_buf(),
            entries,
            progress_call:  call_progress2::<F>,
            progress_data:  &progress_fn as *const F as *const (),
            current:        std::cell::Cell::new(0),
            current_stream: std::cell::Cell::new(ptr::null_mut()),
            error:          std::cell::Cell::new(None),
        };

        let callback_ptr = &mut callback as *mut ErasedCallback2 as *mut IArchiveExtractCallback;
        let hr = unsafe {
            ((*(*archive_ptr).vtbl).extract)(archive_ptr, ptr::null(), 0xFFFFFFFF, 0, callback_ptr)
        };
        if hr != S_OK && hr != S_FALSE {
            return Err(LockError::ExtractionFailed(format!("IInArchive::Extract HRESULT {:#x}", hr)));
        }
        if let Some(err) = callback.error.take() { return Err(err); }
        Ok(())
    }

    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }
}
