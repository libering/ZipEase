// FFI functions receive raw pointers from C# P/Invoke and must dereference them,
// but cannot be marked `unsafe` as that changes the extern "C" calling convention.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod cache;
pub mod decoder;
pub mod error;
pub mod ffi;
pub mod image_format;
pub mod magic_bytes;
pub mod natural_sort;
pub mod temp;
pub mod thumbnail;
