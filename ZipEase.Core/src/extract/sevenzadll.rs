//! SevenZaDllBackend — loads `7za.dll` at runtime via libloading and implements
//! `ExtractionBackend` for RAR archives using the COM-like IInArchive interface.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use libloading::{Library, Symbol};

use crate::error::types::LockError;
use super::ExtractionBackend;

// ─── Primitive type aliases ───────────────────────────────────────────────────

pub type HRESULT = i32;
pub type PROPID   = u32;
pub type VARTYPE  = u16;
pub type BSTR     = *mut u16;

pub const S_OK:          HRESULT = 0;
pub const S_FALSE:       HRESULT = 1;
pub const E_NOINTERFACE: HRESULT = -2147467262i32;

// ─── PROPVARIANT variant-type constants ───────────────────────────────────────

pub const VT_EMPTY: u16 = 0;
pub const VT_BOOL:  u16 = 11;
pub const VT_BSTR:  u16 = 8;

// ─── Property ID constants ────────────────────────────────────────────────────

pub const KPID_PATH:   u32 = 3;
pub const KPID_IS_DIR: u32 = 6;

// ─── GUID ─────────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

// RAR handler CLSID: {23170F69-40C1-278A-1000-000110030000}
pub const CLSID_RAR_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x03, 0x00, 0x00],
};

// IInArchive IID: {23170F69-40C1-278A-0000-000600600000}
pub const IID_IIN_ARCHIVE: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x00, 0x00, 0x00, 0x06, 0x00, 0x60, 0x00, 0x00],
};

// IInStream IID: {23170F69-40C1-278A-0000-000300030000}
pub const IID_IIN_STREAM: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x00],
};

// IUnknown IID: {00000000-0000-0000-C000-000000000046}
pub const IID_IUNKNOWN: GUID = GUID {
    data1: 0x00000000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

// ─── PROPVARIANT ──────────────────────────────────────────────────────────────

#[repr(C)]
pub struct PROPVARIANT {
    pub vt:   u16,
    pub _pad: [u16; 3],
    pub data: [u8; 8],
}

impl PROPVARIANT {
    pub fn zeroed() -> Self {
        PROPVARIANT {
            vt:   0,
            _pad: [0; 3],
            data: [0; 8],
        }
    }
}

// ─── IInArchive vtable ────────────────────────────────────────────────────────

#[repr(C)]
pub struct IInArchiveVtbl {
    // IUnknown
    pub query_interface: unsafe extern "system" fn(*mut IInArchive, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref:         unsafe extern "system" fn(*mut IInArchive) -> u32,
    pub release:         unsafe extern "system" fn(*mut IInArchive) -> u32,
    // IInArchive
    pub open:                             unsafe extern "system" fn(*mut IInArchive, *mut IInStream, *const u64, *mut IArchiveOpenCallback) -> HRESULT,
    pub close:                            unsafe extern "system" fn(*mut IInArchive) -> HRESULT,
    pub get_number_of_items:              unsafe extern "system" fn(*mut IInArchive, *mut u32) -> HRESULT,
    pub get_property:                     unsafe extern "system" fn(*mut IInArchive, u32, PROPID, *mut PROPVARIANT) -> HRESULT,
    pub extract:                          unsafe extern "system" fn(*mut IInArchive, *const u32, u32, i32, *mut IArchiveExtractCallback) -> HRESULT,
    pub get_archive_property:             unsafe extern "system" fn(*mut IInArchive, PROPID, *mut PROPVARIANT) -> HRESULT,
    pub get_number_of_properties:         unsafe extern "system" fn(*mut IInArchive, *mut u32) -> HRESULT,
    pub get_property_info:                unsafe extern "system" fn(*mut IInArchive, u32, *mut BSTR, *mut VARTYPE, *mut PROPID) -> HRESULT,
    pub get_number_of_archive_properties: unsafe extern "system" fn(*mut IInArchive, *mut u32) -> HRESULT,
    pub get_archive_property_info:        unsafe extern "system" fn(*mut IInArchive, u32, *mut BSTR, *mut VARTYPE, *mut PROPID) -> HRESULT,
}

#[repr(C)]
pub struct IInArchive {
    pub vtbl: *const IInArchiveVtbl,
}

// ─── IInStream vtable ─────────────────────────────────────────────────────────

#[repr(C)]
pub struct IInStreamVtbl {
    // IUnknown
    pub query_interface: unsafe extern "system" fn(*mut IInStream, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref:         unsafe extern "system" fn(*mut IInStream) -> u32,
    pub release:         unsafe extern "system" fn(*mut IInStream) -> u32,
    // ISequentialInStream
    pub read:            unsafe extern "system" fn(*mut IInStream, *mut c_void, u32, *mut u32) -> HRESULT,
    // IInStream
    pub seek:            unsafe extern "system" fn(*mut IInStream, i64, u32, *mut u64) -> HRESULT,
}

#[repr(C)]
pub struct IInStream {
    pub vtbl: *const IInStreamVtbl,
}

// ─── IArchiveOpenCallback (opaque) ────────────────────────────────────────────

#[repr(C)]
pub struct IArchiveOpenCallback {
    _opaque: [u8; 0],
}

// ─── IArchiveExtractCallback vtable ──────────────────────────────────────────

#[repr(C)]
pub struct IArchiveExtractCallbackVtbl {
    // IUnknown
    pub query_interface: unsafe extern "system" fn(*mut IArchiveExtractCallback, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref:         unsafe extern "system" fn(*mut IArchiveExtractCallback) -> u32,
    pub release:         unsafe extern "system" fn(*mut IArchiveExtractCallback) -> u32,
    // IProgress
    pub set_total:       unsafe extern "system" fn(*mut IArchiveExtractCallback, u64) -> HRESULT,
    pub set_completed:   unsafe extern "system" fn(*mut IArchiveExtractCallback, *const u64) -> HRESULT,
    // IArchiveExtractCallback
    pub get_stream:           unsafe extern "system" fn(*mut IArchiveExtractCallback, u32, *mut *mut ISequentialOutStream, i32) -> HRESULT,
    pub prepare_operation:    unsafe extern "system" fn(*mut IArchiveExtractCallback, i32) -> HRESULT,
    pub set_operation_result: unsafe extern "system" fn(*mut IArchiveExtractCallback, i32) -> HRESULT,
}

#[repr(C)]
pub struct IArchiveExtractCallback {
    pub vtbl: *const IArchiveExtractCallbackVtbl,
}

// ─── ISequentialOutStream vtable ─────────────────────────────────────────────

#[repr(C)]
pub struct ISequentialOutStreamVtbl {
    pub query_interface: unsafe extern "system" fn(*mut ISequentialOutStream, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref:         unsafe extern "system" fn(*mut ISequentialOutStream) -> u32,
    pub release:         unsafe extern "system" fn(*mut ISequentialOutStream) -> u32,
    pub write:           unsafe extern "system" fn(*mut ISequentialOutStream, *const c_void, u32, *mut u32) -> HRESULT,
}

#[repr(C)]
pub struct ISequentialOutStream {
    pub vtbl: *const ISequentialOutStreamVtbl,
}

// ─── RustInStream ─────────────────────────────────────────────────────────────

#[repr(C)]
pub struct RustInStream {
    pub vtbl:      *const IInStreamVtbl,
    pub ref_count: AtomicU32,
    pub file:      std::fs::File,
}

// ─── SevenZaDllBackend ────────────────────────────────────────────────────────

/// Zero-sized backend that loads `7za.dll` per operation call.
pub struct SevenZaDllBackend;

// ─── RustInStream vtable functions ───────────────────────────────────────────

unsafe extern "system" fn rust_in_stream_query_interface(
    this: *mut IInStream,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let iid = &*iid;
    if *iid == IID_IUNKNOWN || *iid == IID_IIN_STREAM {
        rust_in_stream_add_ref(this);
        *out = this as *mut c_void;
        S_OK
    } else {
        *out = ptr::null_mut();
        E_NOINTERFACE
    }
}

unsafe extern "system" fn rust_in_stream_add_ref(this: *mut IInStream) -> u32 {
    let stream = &*(this as *mut RustInStream);
    stream.ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn rust_in_stream_release(this: *mut IInStream) -> u32 {
    let stream = &*(this as *mut RustInStream);
    let prev = stream.ref_count.fetch_sub(1, Ordering::Release);
    if prev == 1 {
        std::sync::atomic::fence(Ordering::Acquire);
        drop(Box::from_raw(this as *mut RustInStream));
        0
    } else {
        prev - 1
    }
}

unsafe extern "system" fn rust_in_stream_read(
    this: *mut IInStream,
    data: *mut c_void,
    size: u32,
    processed_size: *mut u32,
) -> HRESULT {
    let stream = &mut *(this as *mut RustInStream);
    let buf = std::slice::from_raw_parts_mut(data as *mut u8, size as usize);
    match stream.file.read(buf) {
        Ok(n) => {
            if !processed_size.is_null() {
                *processed_size = n as u32;
            }
            S_OK
        }
        Err(_) => {
            if !processed_size.is_null() {
                *processed_size = 0;
            }
            // Return a generic read error HRESULT
            -1i32
        }
    }
}

unsafe extern "system" fn rust_in_stream_seek(
    this: *mut IInStream,
    offset: i64,
    seek_origin: u32,
    new_position: *mut u64,
) -> HRESULT {
    let stream = &mut *(this as *mut RustInStream);
    let from = match seek_origin {
        0 => SeekFrom::Start(offset as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return -1i32,
    };
    match stream.file.seek(from) {
        Ok(pos) => {
            if !new_position.is_null() {
                *new_position = pos;
            }
            S_OK
        }
        Err(_) => -1i32,
    }
}

static RUST_IN_STREAM_VTBL: IInStreamVtbl = IInStreamVtbl {
    query_interface: rust_in_stream_query_interface,
    add_ref:         rust_in_stream_add_ref,
    release:         rust_in_stream_release,
    read:            rust_in_stream_read,
    seek:            rust_in_stream_seek,
};

impl RustInStream {
    /// Opens `path` and returns a heap-allocated `RustInStream` with ref_count = 1.
    pub fn new(path: &Path) -> Result<Box<Self>, LockError> {
        let file = std::fs::File::open(path)
            .map_err(|_| LockError::PathNotFound(path.to_string_lossy().into_owned()))?;
        Ok(Box::new(RustInStream {
            vtbl:      &RUST_IN_STREAM_VTBL,
            ref_count: AtomicU32::new(1),
            file,
        }))
    }

    /// Returns a raw pointer to this stream cast as `*mut IInStream`.
    /// The caller must ensure the `RustInStream` stays alive for the duration of use.
    pub fn as_ptr(&self) -> *mut IInStream {
        self as *const RustInStream as *mut IInStream
    }
}

// ─── RAII wrappers ────────────────────────────────────────────────────────────

/// RAII guard for an `IInArchive` COM pointer.
/// Calls `Close` then `Release` on drop.
pub struct InArchiveGuard {
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
/// Frees the BSTR if `vt == VT_BSTR` using `SysFreeString`.
pub struct PropVariantGuard(pub PROPVARIANT);

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
///
/// Primary: `GetModuleHandleW("zipease_core.dll")` + `GetModuleFileNameW`.
/// Fallback: `std::env::current_exe()` parent directory.
/// Returns `LockError::PluginRequired` if the resolved path does not exist.
pub fn resolve_dll_path() -> Result<PathBuf, LockError> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetModuleFileNameW};
    use windows::core::PCWSTR;

    let dir = unsafe {
        // Build a null-terminated UTF-16 module name
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
pub fn wide_to_string(ptr: *const u16) -> String {
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

// ─── CreateObject function type ───────────────────────────────────────────────

type CreateObjectFn = unsafe extern "system" fn(
    class_id:     *const GUID,
    interface_id: *const GUID,
    out_object:   *mut *mut c_void,
) -> HRESULT;

// ─── RustExtractCallback ──────────────────────────────────────────────────────

/// Rust-side implementation of `IArchiveExtractCallback`.
/// Holds the output directory, entry names, progress closure, and current index.
struct RustExtractCallback<F: Fn(usize, usize, &str)> {
    vtbl:        *const IArchiveExtractCallbackVtbl,
    ref_count:   AtomicU32,
    output_dir:  PathBuf,
    entries:     Vec<String>,
    progress_fn: F,
    current:     std::cell::Cell<usize>,
    /// Holds the current out-stream so it stays alive until SetOperationResult
    current_stream: std::cell::Cell<*mut RustOutStream>,
    /// Extraction error, if any
    error:       std::cell::Cell<Option<LockError>>,
}

/// Rust-side `ISequentialOutStream` backed by a `std::fs::File`.
#[repr(C)]
struct RustOutStream {
    vtbl:      *const ISequentialOutStreamVtbl,
    ref_count: AtomicU32,
    file:      std::fs::File,
}

// ─── RustOutStream vtable functions ──────────────────────────────────────────

unsafe extern "system" fn out_stream_query_interface(
    this: *mut ISequentialOutStream,
    _iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    out_stream_add_ref(this);
    *out = this as *mut c_void;
    S_OK
}

unsafe extern "system" fn out_stream_add_ref(this: *mut ISequentialOutStream) -> u32 {
    let s = &*(this as *mut RustOutStream);
    s.ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn out_stream_release(this: *mut ISequentialOutStream) -> u32 {
    let s = &*(this as *mut RustOutStream);
    let prev = s.ref_count.fetch_sub(1, Ordering::Release);
    if prev == 1 {
        std::sync::atomic::fence(Ordering::Acquire);
        drop(Box::from_raw(this as *mut RustOutStream));
        0
    } else {
        prev - 1
    }
}

unsafe extern "system" fn out_stream_write(
    this: *mut ISequentialOutStream,
    data: *const c_void,
    size: u32,
    processed_size: *mut u32,
) -> HRESULT {
    use std::io::Write;
    let s = &mut *(this as *mut RustOutStream);
    let buf = std::slice::from_raw_parts(data as *const u8, size as usize);
    match s.file.write(buf) {
        Ok(n) => {
            if !processed_size.is_null() {
                *processed_size = n as u32;
            }
            S_OK
        }
        Err(_) => {
            if !processed_size.is_null() {
                *processed_size = 0;
            }
            -1i32
        }
    }
}

static RUST_OUT_STREAM_VTBL: ISequentialOutStreamVtbl = ISequentialOutStreamVtbl {
    query_interface: out_stream_query_interface,
    add_ref:         out_stream_add_ref,
    release:         out_stream_release,
    write:           out_stream_write,
};

// ─── RustExtractCallback vtable functions ─────────────────────────────────────

unsafe extern "system" fn extract_cb_query_interface(
    this: *mut IArchiveExtractCallback,
    _iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    extract_cb_add_ref(this);
    *out = this as *mut c_void;
    S_OK
}

unsafe extern "system" fn extract_cb_add_ref(this: *mut IArchiveExtractCallback) -> u32 {
    // The callback is stack-allocated; ref counting is a no-op
    let _ = this;
    1
}

unsafe extern "system" fn extract_cb_release(this: *mut IArchiveExtractCallback) -> u32 {
    let _ = this;
    1
}

unsafe extern "system" fn extract_cb_set_total(
    _this: *mut IArchiveExtractCallback,
    _total: u64,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn extract_cb_set_completed(
    _this: *mut IArchiveExtractCallback,
    _completed: *const u64,
) -> HRESULT {
    S_OK
}

/// `GetStream` — called by 7za.dll to get an output stream for entry `index`.
unsafe extern "system" fn extract_cb_get_stream(
    this: *mut IArchiveExtractCallback,
    index: u32,
    out_stream: *mut *mut ISequentialOutStream,
    ask_extract_mode: i32,
) -> HRESULT {
    // ask_extract_mode: 0 = extract, 1 = test, 2 = skip
    if ask_extract_mode != 0 {
        *out_stream = ptr::null_mut();
        return S_OK;
    }

    let cb = &*(this as *mut RustExtractCallback<fn(usize, usize, &str)>);
    let idx = index as usize;

    let entry_name = if idx < cb.entries.len() {
        cb.entries[idx].clone()
    } else {
        *out_stream = ptr::null_mut();
        return S_OK;
    };

    // Skip directory entries
    if entry_name.ends_with('/') || entry_name.ends_with('\\') {
        *out_stream = ptr::null_mut();
        return S_OK;
    }

    let out_path = cb.output_dir.join(&entry_name);

    // Create parent directories
    if let Some(parent) = out_path.parent() {
        if let Err(_) = std::fs::create_dir_all(parent) {
            *out_stream = ptr::null_mut();
            return S_OK;
        }
    }

    match std::fs::File::create(&out_path) {
        Ok(file) => {
            let stream = Box::new(RustOutStream {
                vtbl:      &RUST_OUT_STREAM_VTBL,
                ref_count: AtomicU32::new(1),
                file,
            });
            let raw = Box::into_raw(stream);
            cb.current_stream.set(raw);
            *out_stream = raw as *mut ISequentialOutStream;
            S_OK
        }
        Err(_) => {
            *out_stream = ptr::null_mut();
            S_OK
        }
    }
}

unsafe extern "system" fn extract_cb_prepare_operation(
    _this: *mut IArchiveExtractCallback,
    _ask_extract_mode: i32,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn extract_cb_set_operation_result(
    this: *mut IArchiveExtractCallback,
    operation_result: i32,
) -> HRESULT {
    let cb = &*(this as *mut RustExtractCallback<fn(usize, usize, &str)>);

    // Release the current out-stream
    let stream_ptr = cb.current_stream.get();
    if !stream_ptr.is_null() {
        drop(Box::from_raw(stream_ptr));
        cb.current_stream.set(ptr::null_mut());
    }

    let current = cb.current.get();
    let total = cb.entries.len();
    let name = if current < cb.entries.len() { &cb.entries[current] } else { "" };

    // operation_result: 0 = OK, 1 = unsupported method, 2 = data error / password
    if operation_result == 2 {
        cb.error.set(Some(LockError::ExtractionFailed(
            "Archive is encrypted; password-protected RAR archives are not supported".into(),
        )));
    } else if operation_result != 0 {
        cb.error.set(Some(LockError::ExtractionFailed(format!(
            "Extraction error for {}: code {}",
            name, operation_result
        ))));
    }

    (cb.progress_fn)(current, total, name);
    cb.current.set(current + 1);
    S_OK
}

// We need a concrete vtable for the callback — use a static with fn pointer casts.
// Because the callback is generic, we use a type-erased approach: the vtable functions
// cast `this` to the concrete type. Since the vtable is the same layout regardless of F,
// we use a single static vtable with the non-generic free functions above.
static RUST_EXTRACT_CALLBACK_VTBL: IArchiveExtractCallbackVtbl = IArchiveExtractCallbackVtbl {
    query_interface:      extract_cb_query_interface,
    add_ref:              extract_cb_add_ref,
    release:              extract_cb_release,
    set_total:            extract_cb_set_total,
    set_completed:        extract_cb_set_completed,
    get_stream:           extract_cb_get_stream,
    prepare_operation:    extract_cb_prepare_operation,
    set_operation_result: extract_cb_set_operation_result,
};

// ─── ExtractionBackend impl ───────────────────────────────────────────────────

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
                &CLSID_RAR_HANDLER,
                &IID_IIN_ARCHIVE,
                &mut archive_ptr as *mut *mut IInArchive as *mut *mut c_void,
            )
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!(
                "CreateObject HRESULT {:#x}", hr
            )));
        }
        let _guard = InArchiveGuard { ptr: archive_ptr };

        let stream = RustInStream::new(archive_path)?;
        let stream_ptr = stream.as_ptr();
        // Leak the Box — the COM ref-count will manage its lifetime
        let _ = Box::into_raw(stream);

        let hr = unsafe {
            ((*(*archive_ptr).vtbl).open)(
                archive_ptr,
                stream_ptr,
                ptr::null(),
                ptr::null_mut(),
            )
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!(
                "IInArchive::Open HRESULT {:#x}", hr
            )));
        }

        let mut count: u32 = 0;
        unsafe {
            ((*(*archive_ptr).vtbl).get_number_of_items)(archive_ptr, &mut count);
        }

        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            // Check if directory
            let mut pv_dir = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(
                    archive_ptr, i, KPID_IS_DIR, &mut pv_dir.0,
                );
            }
            if pv_dir.0.vt == VT_BOOL {
                let val = unsafe { ptr::read_unaligned(pv_dir.0.data.as_ptr() as *const i16) };
                if val != 0 {
                    continue; // skip directories
                }
            }

            let mut pv = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(
                    archive_ptr, i, KPID_PATH, &mut pv.0,
                );
            }
            if pv.0.vt == VT_BSTR {
                let bstr = unsafe {
                    ptr::read_unaligned(pv.0.data.as_ptr() as *const *const u16)
                };
                entries.push(wide_to_string(bstr));
            }
        }

        // _guard drops here → Close + Release called
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
                &CLSID_RAR_HANDLER,
                &IID_IIN_ARCHIVE,
                &mut archive_ptr as *mut *mut IInArchive as *mut *mut c_void,
            )
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!(
                "CreateObject HRESULT {:#x}", hr
            )));
        }
        let _guard = InArchiveGuard { ptr: archive_ptr };

        let stream = RustInStream::new(archive_path)?;
        let stream_ptr = stream.as_ptr();
        let _ = Box::into_raw(stream);

        let hr = unsafe {
            ((*(*archive_ptr).vtbl).open)(
                archive_ptr,
                stream_ptr,
                ptr::null(),
                ptr::null_mut(),
            )
        };
        if hr != S_OK {
            return Err(LockError::ExtractionFailed(format!(
                "IInArchive::Open HRESULT {:#x}", hr
            )));
        }

        // Get entry list for progress reporting
        let mut count: u32 = 0;
        unsafe {
            ((*(*archive_ptr).vtbl).get_number_of_items)(archive_ptr, &mut count);
        }
        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut pv = PropVariantGuard(PROPVARIANT::zeroed());
            unsafe {
                ((*(*archive_ptr).vtbl).get_property)(
                    archive_ptr, i, KPID_PATH, &mut pv.0,
                );
            }
            let name = if pv.0.vt == VT_BSTR {
                let bstr = unsafe {
                    ptr::read_unaligned(pv.0.data.as_ptr() as *const *const u16)
                };
                wide_to_string(bstr)
            } else {
                String::new()
            };
            entries.push(name);
        }

        // Build the callback. Store the closure as a raw pointer to avoid 'static requirement.
        // The callback is stack-allocated and the closure lives for the duration of this call.
        struct ErasedCallback {
            vtbl:           *const IArchiveExtractCallbackVtbl,
            output_dir:     PathBuf,
            entries:        Vec<String>,
            /// Type-erased function pointer: fn(data, current, total, name)
            progress_call:  unsafe fn(*const (), usize, usize, &str),
            progress_data:  *const (),
            current:        std::cell::Cell<usize>,
            current_stream: std::cell::Cell<*mut RustOutStream>,
            error:          std::cell::Cell<Option<LockError>>,
        }
        unsafe extern "system" fn erased_get_stream(
            this: *mut IArchiveExtractCallback,
            index: u32,
            out_stream: *mut *mut ISequentialOutStream,
            ask_extract_mode: i32,
        ) -> HRESULT {
            if ask_extract_mode != 0 {
                *out_stream = ptr::null_mut();
                return S_OK;
            }
            let cb = &*(this as *mut ErasedCallback);
            let idx = index as usize;
            let entry_name = if idx < cb.entries.len() {
                cb.entries[idx].clone()
            } else {
                *out_stream = ptr::null_mut();
                return S_OK;
            };
            if entry_name.ends_with('/') || entry_name.ends_with('\\') {
                *out_stream = ptr::null_mut();
                return S_OK;
            }
            let out_path = cb.output_dir.join(&entry_name);
            if let Some(parent) = out_path.parent() {
                if std::fs::create_dir_all(parent).is_err() {
                    *out_stream = ptr::null_mut();
                    return S_OK;
                }
            }
            match std::fs::File::create(&out_path) {
                Ok(file) => {
                    let stream = Box::new(RustOutStream {
                        vtbl:      &RUST_OUT_STREAM_VTBL,
                        ref_count: AtomicU32::new(1),
                        file,
                    });
                    let raw = Box::into_raw(stream);
                    cb.current_stream.set(raw);
                    *out_stream = raw as *mut ISequentialOutStream;
                    S_OK
                }
                Err(_) => {
                    *out_stream = ptr::null_mut();
                    S_OK
                }
            }
        }

        unsafe extern "system" fn erased_set_operation_result(
            this: *mut IArchiveExtractCallback,
            operation_result: i32,
        ) -> HRESULT {
            let cb = &*(this as *mut ErasedCallback);
            let stream_ptr = cb.current_stream.get();
            if !stream_ptr.is_null() {
                drop(Box::from_raw(stream_ptr));
                cb.current_stream.set(ptr::null_mut());
            }
            let current = cb.current.get();
            let total = cb.entries.len();
            let name = if current < cb.entries.len() { &cb.entries[current] } else { "" };
            if operation_result == 2 {
                cb.error.set(Some(LockError::ExtractionFailed(
                    "Archive is encrypted; password-protected RAR archives are not supported".into(),
                )));
            } else if operation_result != 0 {
                cb.error.set(Some(LockError::ExtractionFailed(format!(
                    "Extraction error for {}: code {}", name, operation_result
                ))));
            }
            (cb.progress_call)(cb.progress_data, current, total, name);
            cb.current.set(current + 1);
            S_OK
        }

        static ERASED_VTBL: IArchiveExtractCallbackVtbl = IArchiveExtractCallbackVtbl {
            query_interface:      extract_cb_query_interface,
            add_ref:              extract_cb_add_ref,
            release:              extract_cb_release,
            set_total:            extract_cb_set_total,
            set_completed:        extract_cb_set_completed,
            get_stream:           erased_get_stream,
            prepare_operation:    extract_cb_prepare_operation,
            set_operation_result: erased_set_operation_result,
        };

        unsafe fn call_progress<F: Fn(usize, usize, &str)>(
            data: *const (),
            current: usize,
            total: usize,
            name: &str,
        ) {
            let f = &*(data as *const F);
            f(current, total, name);
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
            ((*(*archive_ptr).vtbl).extract)(
                archive_ptr,
                ptr::null(),   // NULL = all items
                0xFFFFFFFF,    // count = all
                0,             // test = 0 (extract)
                callback_ptr,
            )
        };

        if hr != S_OK && hr != S_FALSE {
            return Err(LockError::ExtractionFailed(format!(
                "IInArchive::Extract HRESULT {:#x}", hr
            )));
        }

        if let Some(err) = callback.error.take() {
            return Err(err);
        }

        Ok(())
    }

    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }
}
