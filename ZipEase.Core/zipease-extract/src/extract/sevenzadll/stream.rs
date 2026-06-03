//! Stream implementations for the 7za.dll backend.
//!
//! Contains: `RustInStream`, `RustOutStream`, their vtable function implementations,
//! and the static `RUST_IN_STREAM_VTBL` / `RUST_OUT_STREAM_VTBL` instances.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use zipease_shared::LockError;

use super::types::{
    GUID, HRESULT, IInStream, IInStreamVtbl, ISequentialOutStream, ISequentialOutStreamVtbl,
    E_NOINTERFACE, IID_IIN_STREAM, IID_IUNKNOWN, S_OK,
};

// ─── RustInStream ─────────────────────────────────────────────────────────────

/// Rust-side implementation of `IInStream` backed by a `std::fs::File`.
#[repr(C)]
pub(crate) struct RustInStream {
    pub vtbl:      *const IInStreamVtbl,
    pub ref_count: AtomicU32,
    pub file:      std::fs::File,
}

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

// ─── RustInStream static vtable ──────────────────────────────────────────────

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
    pub fn as_ptr(&self) -> *mut IInStream {
        self as *const RustInStream as *mut IInStream
    }
}

// ─── RustOutStream ────────────────────────────────────────────────────────────

/// Rust-side `ISequentialOutStream` backed by a `std::fs::File`.
#[repr(C)]
pub(crate) struct RustOutStream {
    pub vtbl:      *const ISequentialOutStreamVtbl,
    pub ref_count: AtomicU32,
    pub file:      std::fs::File,
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

// ─── RustOutStream static vtable ─────────────────────────────────────────────

pub(crate) static RUST_OUT_STREAM_VTBL: ISequentialOutStreamVtbl = ISequentialOutStreamVtbl {
    query_interface: out_stream_query_interface,
    add_ref:         out_stream_add_ref,
    release:         out_stream_release,
    write:           out_stream_write,
};
