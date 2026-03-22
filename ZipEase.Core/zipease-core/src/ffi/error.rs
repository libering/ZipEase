use zipease_shared::get_last_error;
use std::ffi::CString;
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn zip_ease_get_last_error() -> *const c_char {
    std::panic::catch_unwind(|| {
        match get_last_error() {
            Some(error) => {
                let msg = error.message();
                match CString::new(msg) {
                    Ok(c_str) => c_str.into_raw(),
                    Err(_) => std::ptr::null(),
                }
            }
            None => std::ptr::null(),
        }
    })
    .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn zip_ease_free_error_string(ptr: *mut c_char) {
    if ptr.is_null() { return; }
    let _ = std::panic::catch_unwind(|| unsafe {
        let _ = CString::from_raw(ptr);
    });
}
