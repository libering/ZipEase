//! Extraction callback implementations for the 7za.dll backend.
//!
//! Contains: `RustExtractCallback` (generic over F), shared callback vtable functions,
//! `ErasedCallback`, `ErasedCallback2`, `SingleCb`, their per-callback vtable functions,
//! and the static vtable instances (`RUST_EXTRACT_CALLBACK_VTBL`, `ERASED_VTBL`,
//! `EC2_VTBL`, `SC_VTBL`).

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::AtomicU32;

use zipease_shared::LockError;

use super::stream::{RustOutStream, RUST_OUT_STREAM_VTBL};
use super::types::{
    GUID, HRESULT, IArchiveExtractCallback, IArchiveExtractCallbackVtbl,
    ISequentialOutStream, S_OK,
};

// ─── RustExtractCallback ──────────────────────────────────────────────────────

/// Rust-side implementation of `IArchiveExtractCallback` (generic, used in the
/// original `SevenZaDllBackend` list/extract path).
pub(crate) struct RustExtractCallback<F: Fn(usize, usize, &str)> {
    pub vtbl:           *const IArchiveExtractCallbackVtbl,
    pub ref_count:      AtomicU32,
    pub output_dir:     PathBuf,
    pub entries:        Vec<String>,
    pub progress_fn:    F,
    pub current:        std::cell::Cell<usize>,
    pub current_stream: std::cell::Cell<*mut RustOutStream>,
    pub error:          std::cell::Cell<Option<LockError>>,
}

// ─── Shared extract callback vtable functions ─────────────────────────────────

pub(crate) unsafe extern "system" fn extract_cb_query_interface(
    this: *mut IArchiveExtractCallback,
    _iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    extract_cb_add_ref(this);
    *out = this as *mut c_void;
    S_OK
}

pub(crate) unsafe extern "system" fn extract_cb_add_ref(
    this: *mut IArchiveExtractCallback,
) -> u32 {
    let _ = this;
    1
}

pub(crate) unsafe extern "system" fn extract_cb_release(
    this: *mut IArchiveExtractCallback,
) -> u32 {
    let _ = this;
    1
}

pub(crate) unsafe extern "system" fn extract_cb_set_total(
    _this: *mut IArchiveExtractCallback,
    _total: u64,
) -> HRESULT {
    S_OK
}

pub(crate) unsafe extern "system" fn extract_cb_set_completed(
    _this: *mut IArchiveExtractCallback,
    _completed: *const u64,
) -> HRESULT {
    S_OK
}

pub(crate) unsafe extern "system" fn extract_cb_prepare_operation(
    _this: *mut IArchiveExtractCallback,
    _ask_extract_mode: i32,
) -> HRESULT {
    S_OK
}

// ─── RustExtractCallback-specific vtable functions ────────────────────────────

unsafe extern "system" fn extract_cb_get_stream(
    this: *mut IArchiveExtractCallback,
    index: u32,
    out_stream: *mut *mut ISequentialOutStream,
    ask_extract_mode: i32,
) -> HRESULT {
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

unsafe extern "system" fn extract_cb_set_operation_result(
    this: *mut IArchiveExtractCallback,
    operation_result: i32,
) -> HRESULT {
    let cb = &*(this as *mut RustExtractCallback<fn(usize, usize, &str)>);

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
            "Extraction error for {}: code {}",
            name, operation_result
        ))));
    }

    (cb.progress_fn)(current, total, name);
    cb.current.set(current + 1);
    S_OK
}

pub(crate) static RUST_EXTRACT_CALLBACK_VTBL: IArchiveExtractCallbackVtbl =
    IArchiveExtractCallbackVtbl {
        query_interface:      extract_cb_query_interface,
        add_ref:              extract_cb_add_ref,
        release:              extract_cb_release,
        set_total:            extract_cb_set_total,
        set_completed:        extract_cb_set_completed,
        get_stream:           extract_cb_get_stream,
        prepare_operation:    extract_cb_prepare_operation,
        set_operation_result: extract_cb_set_operation_result,
    };

// ─── ErasedCallback (used in SevenZaDllBackend::extract_with_progress) ────────

/// Type-erased extraction callback that stores the progress closure as a raw
/// function pointer + data pointer pair.
pub(crate) struct ErasedCallback {
    pub vtbl:           *const IArchiveExtractCallbackVtbl,
    pub output_dir:     PathBuf,
    pub entries:        Vec<String>,
    pub progress_call:  unsafe fn(*const (), usize, usize, &str),
    pub progress_data:  *const (),
    pub current:        std::cell::Cell<usize>,
    pub current_stream: std::cell::Cell<*mut RustOutStream>,
    pub error:          std::cell::Cell<Option<LockError>>,
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

pub(crate) static ERASED_VTBL: IArchiveExtractCallbackVtbl = IArchiveExtractCallbackVtbl {
    query_interface:      extract_cb_query_interface,
    add_ref:              extract_cb_add_ref,
    release:              extract_cb_release,
    set_total:            extract_cb_set_total,
    set_completed:        extract_cb_set_completed,
    get_stream:           erased_get_stream,
    prepare_operation:    extract_cb_prepare_operation,
    set_operation_result: erased_set_operation_result,
};

/// Helper to invoke a typed progress closure through a raw pointer.
pub(crate) unsafe fn call_progress<F: Fn(usize, usize, &str)>(
    data: *const (),
    current: usize,
    total: usize,
    name: &str,
) {
    let f = &*(data as *const F);
    f(current, total, name);
}

// ─── ErasedCallback2 (used in SevenZaDllBackendWithClsid::extract_with_progress)

/// Type-erased extraction callback for `SevenZaDllBackendWithClsid`.
pub(crate) struct ErasedCallback2 {
    pub vtbl:           *const IArchiveExtractCallbackVtbl,
    pub output_dir:     PathBuf,
    pub entries:        Vec<String>,
    pub progress_call:  unsafe fn(*const (), usize, usize, &str),
    pub progress_data:  *const (),
    pub current:        std::cell::Cell<usize>,
    pub current_stream: std::cell::Cell<*mut RustOutStream>,
    pub error:          std::cell::Cell<Option<LockError>>,
}

unsafe extern "system" fn ec2_get_stream(
    this: *mut IArchiveExtractCallback,
    index: u32,
    out_stream: *mut *mut ISequentialOutStream,
    ask_extract_mode: i32,
) -> HRESULT {
    if ask_extract_mode != 0 {
        *out_stream = ptr::null_mut();
        return S_OK;
    }
    let cb = &*(this as *mut ErasedCallback2);
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
        let _ = std::fs::create_dir_all(parent);
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

unsafe extern "system" fn ec2_set_operation_result(
    this: *mut IArchiveExtractCallback,
    operation_result: i32,
) -> HRESULT {
    let cb = &*(this as *mut ErasedCallback2);
    let stream_ptr = cb.current_stream.get();
    if !stream_ptr.is_null() {
        drop(Box::from_raw(stream_ptr));
        cb.current_stream.set(ptr::null_mut());
    }
    let current = cb.current.get();
    let total = cb.entries.len();
    let name = if current < cb.entries.len() { &cb.entries[current] } else { "" };
    if operation_result != 0 && operation_result != 2 {
        cb.error.set(Some(LockError::ExtractionFailed(format!(
            "Extraction error for {}: code {}", name, operation_result
        ))));
    }
    (cb.progress_call)(cb.progress_data, current, total, name);
    cb.current.set(current + 1);
    S_OK
}

pub(crate) static EC2_VTBL: IArchiveExtractCallbackVtbl = IArchiveExtractCallbackVtbl {
    query_interface:      extract_cb_query_interface,
    add_ref:              extract_cb_add_ref,
    release:              extract_cb_release,
    set_total:            extract_cb_set_total,
    set_completed:        extract_cb_set_completed,
    get_stream:           ec2_get_stream,
    prepare_operation:    extract_cb_prepare_operation,
    set_operation_result: ec2_set_operation_result,
};

/// Helper to invoke a typed progress closure through a raw pointer (for WithClsid variant).
pub(crate) unsafe fn call_progress2<F: Fn(usize, usize, &str)>(
    data: *const (),
    current: usize,
    total: usize,
    name: &str,
) {
    let f = &*(data as *const F);
    f(current, total, name);
}

// ─── SingleCb (used in extract_single_entry) ─────────────────────────────────

/// Minimal callback for extracting a single entry by index.
pub(crate) struct SingleCb {
    pub vtbl:           *const IArchiveExtractCallbackVtbl,
    pub output_dir:     std::path::PathBuf,
    pub entry_name:     String,
    pub current_stream: std::cell::Cell<*mut RustOutStream>,
    pub error:          std::cell::Cell<Option<LockError>>,
}

unsafe extern "system" fn sc_get_stream(
    this: *mut IArchiveExtractCallback,
    _index: u32,
    out_stream: *mut *mut ISequentialOutStream,
    ask_extract_mode: i32,
) -> HRESULT {
    if ask_extract_mode != 0 {
        *out_stream = ptr::null_mut();
        return S_OK;
    }
    let cb = &*(this as *mut SingleCb);
    let file_name = std::path::Path::new(&cb.entry_name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| cb.entry_name.clone());
    let out_path = cb.output_dir.join(&file_name);
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::File::create(&out_path) {
        Ok(file) => {
            let s = Box::new(RustOutStream {
                vtbl:      &RUST_OUT_STREAM_VTBL,
                ref_count: AtomicU32::new(1),
                file,
            });
            let raw = Box::into_raw(s);
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

unsafe extern "system" fn sc_set_op_result(
    this: *mut IArchiveExtractCallback,
    op: i32,
) -> HRESULT {
    let cb = &*(this as *mut SingleCb);
    let sp = cb.current_stream.get();
    if !sp.is_null() {
        drop(Box::from_raw(sp));
        cb.current_stream.set(ptr::null_mut());
    }
    if op != 0 {
        cb.error.set(Some(LockError::ExtractionFailed(format!("code {}", op))));
    }
    S_OK
}

pub(crate) static SC_VTBL: IArchiveExtractCallbackVtbl = IArchiveExtractCallbackVtbl {
    query_interface:      extract_cb_query_interface,
    add_ref:              extract_cb_add_ref,
    release:              extract_cb_release,
    set_total:            extract_cb_set_total,
    set_completed:        extract_cb_set_completed,
    get_stream:           sc_get_stream,
    prepare_operation:    extract_cb_prepare_operation,
    set_operation_result: sc_set_op_result,
};
