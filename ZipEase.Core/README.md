# ZipEase.Core

ZipEase 的核心文件鎖定模塊，使用 Rust 實現並提供 C-ABI 接口。

## 核心功能

- **目錄鎖定**: 使用 Windows `CreateFileW` 配合 `FILE_SHARE_READ` 實現目錄鎖定，防止刪除和重命名。
- **FFI 接口**: 提供給 C# WPF 前端調用的 C 兼容接口。
- **線程安全**: 全局鎖定管理器與錯誤存儲均為線程安全設計。
- **Panic 隔離**: 所有 FFI 邊界均使用 `catch_unwind` 確保不會因 Rust Panic 導致宿主進程崩潰。

## 編譯

```bash
cargo build --release
```

編譯產物位於 `target/release/zipease_core.dll`。

## 測試

由於涉及全局狀態（錯誤存儲），建議單線程運行測試：

```bash
cargo test -- --test-threads=1
```

## FFI 接口定義

- `zip_ease_lock_directory(path: *const u16) -> isize`: 鎖定目錄，返回句柄或 -1。
- `zip_ease_unlock_directory(handle: isize) -> i32`: 解鎖目錄，返回 0 或錯誤代碼。
- `zip_ease_get_last_error() -> *const c_char`: 獲取最後一次錯誤的信息字符串。
- `zip_ease_free_error_string(ptr: *mut c_char)`: 釋放錯誤字符串內存。
