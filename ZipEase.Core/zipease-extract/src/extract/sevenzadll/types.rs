//! COM type definitions for the 7za.dll backend.
//!
//! Contains: primitive aliases (HRESULT, PROPID, VARTYPE, BSTR),
//! GUID struct and CLSID/IID constants, PROPVARIANT, VT_* and KPID_* constants,
//! and all vtable struct definitions for IInArchive, IInStream,
//! IArchiveExtractCallback, and ISequentialOutStream.

#![allow(non_snake_case, non_camel_case_types, dead_code, clippy::upper_case_acronyms)]

use std::ffi::c_void;

// ─── Primitive type aliases ───────────────────────────────────────────────────

pub(crate) type HRESULT = i32;
pub(crate) type PROPID   = u32;
pub(crate) type VARTYPE  = u16;
pub(crate) type BSTR     = *mut u16;

pub(crate) const S_OK:          HRESULT = 0;
pub(crate) const S_FALSE:       HRESULT = 1;
pub(crate) const E_NOINTERFACE: HRESULT = -2147467262i32;

// ─── PROPVARIANT variant-type constants ───────────────────────────────────────

pub(crate) const VT_EMPTY: u16 = 0;
pub(crate) const VT_BOOL:  u16 = 11;
pub(crate) const VT_BSTR:  u16 = 8;
pub(crate) const VT_UI8:   u16 = 21;

// ─── Property ID constants ────────────────────────────────────────────────────

pub(crate) const KPID_PATH:   u32 = 3;
pub(crate) const KPID_IS_DIR: u32 = 6;
pub(crate) const KPID_SIZE:   u32 = 7;

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

// 7z handler CLSID: {23170F69-40C1-278A-1000-000110070000}
pub const CLSID_7Z_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x07, 0x00, 0x00],
};

// XZ handler CLSID: {23170F69-40C1-278A-1000-0001100C0000}
pub const CLSID_XZ_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x0C, 0x00, 0x00],
};

// LZMA handler CLSID: {23170F69-40C1-278A-1000-0001100B0000}
pub const CLSID_LZMA_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x0B, 0x00, 0x00],
};

// WIM handler CLSID: {23170F69-40C1-278A-1000-0001100E0000}
pub const CLSID_WIM_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x0E, 0x00, 0x00],
};

// VHD handler CLSID: {23170F69-40C1-278A-1000-0001100F0000}
pub const CLSID_VHD_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x0F, 0x00, 0x00],
};

// ZIP handler CLSID: {23170F69-40C1-278A-1000-000110010000}
pub const CLSID_ZIP_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x01, 0x00, 0x00],
};

// IInArchive IID: {23170F69-40C1-278A-0000-000600600000}
pub(crate) const IID_IIN_ARCHIVE: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x00, 0x00, 0x00, 0x06, 0x00, 0x60, 0x00, 0x00],
};

// IInStream IID: {23170F69-40C1-278A-0000-000300030000}
pub(crate) const IID_IIN_STREAM: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x00],
};

// IUnknown IID: {00000000-0000-0000-C000-000000000046}
pub(crate) const IID_IUNKNOWN: GUID = GUID {
    data1: 0x00000000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

// ─── PROPVARIANT ──────────────────────────────────────────────────────────────

#[repr(C)]
pub(crate) struct PROPVARIANT {
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
pub(crate) struct IInArchiveVtbl {
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
pub(crate) struct IInArchive {
    pub vtbl: *const IInArchiveVtbl,
}

// ─── IInStream vtable ─────────────────────────────────────────────────────────

#[repr(C)]
pub(crate) struct IInStreamVtbl {
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
pub(crate) struct IInStream {
    pub vtbl: *const IInStreamVtbl,
}

// ─── IArchiveOpenCallback (opaque) ────────────────────────────────────────────

#[repr(C)]
pub(crate) struct IArchiveOpenCallback {
    pub(crate) _opaque: [u8; 0],
}

// ─── IArchiveExtractCallback vtable ──────────────────────────────────────────

#[repr(C)]
pub(crate) struct IArchiveExtractCallbackVtbl {
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
pub(crate) struct IArchiveExtractCallback {
    pub vtbl: *const IArchiveExtractCallbackVtbl,
}

// ─── ISequentialOutStream vtable ─────────────────────────────────────────────

#[repr(C)]
pub(crate) struct ISequentialOutStreamVtbl {
    pub query_interface: unsafe extern "system" fn(*mut ISequentialOutStream, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref:         unsafe extern "system" fn(*mut ISequentialOutStream) -> u32,
    pub release:         unsafe extern "system" fn(*mut ISequentialOutStream) -> u32,
    pub write:           unsafe extern "system" fn(*mut ISequentialOutStream, *const c_void, u32, *mut u32) -> HRESULT,
}

#[repr(C)]
pub(crate) struct ISequentialOutStream {
    pub vtbl: *const ISequentialOutStreamVtbl,
}
