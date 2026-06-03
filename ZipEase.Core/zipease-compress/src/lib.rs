// FFI functions receive raw pointers from C# P/Invoke and must dereference them,
// but cannot be marked `unsafe` as that changes the extern "C" calling convention.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod compress;
pub mod ffi;

pub use compress::compress_with_progress;
