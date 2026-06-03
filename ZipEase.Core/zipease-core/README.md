# zipease-core

ZipEase 的核心 Rust crate，編譯為 Windows `cdylib`（DLL），供 C# WPF 前端透過 P/Invoke 調用。

## 項目結構

```
zipease-core/
├── src/
│   ├── lib.rs          # DLL 入口、FFI 符號重新導出
│   ├── ffi/            # FFI 接口層（lock, error）
│   │   ├── mod.rs
│   │   ├── lock.rs     # zip_ease_lock_directory / zip_ease_unlock_directory
│   │   └── error.rs    # zip_ease_get_last_error / zip_ease_free_error_string
│   ├── lock/           # 鎖定業務邏輯
│   │   ├── mod.rs
│   │   ├── handle.rs   # LockHandle 新類型封裝
│   │   └── manager.rs  # LockManager 全局單例
│   └── platform/       # 平台抽象層
│       ├── mod.rs
│       └── windows.rs  # WindowsDirectoryLock（CreateFileW）
├── tests/              # 測試套件
│   ├── error_tests.rs
│   ├── ffi_integration_tests.rs
│   ├── handle_tests.rs
│   ├── lock_manager_tests.rs
│   └── platform_tests.rs
└── Cargo.toml
```

## 編譯

```bash
cd ZipEase.Core
cargo build -p zipease-core --release
```

產出 DLL 位於 `target/release/zipease_core.dll`。

## 測試

```bash
cargo test -p zipease-core
```

包含單元測試、屬性測試（proptest, 100 次迭代）和 FFI 集成測試。

## 從 C# 調用

```csharp
public static class NativeMethods
{
    private const string DllName = "zipease_core.dll";

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern long zip_ease_lock_directory(IntPtr pathPtr);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int zip_ease_unlock_directory(long handle);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr zip_ease_get_last_error();

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void zip_ease_free_error_string(IntPtr ptr);
}
```

### 使用範例

```csharp
// 鎖定目錄
var pathBytes = Encoding.Unicode.GetBytes(directoryPath + "\0");
var pathPtr = Marshal.AllocHGlobal(pathBytes.Length);
Marshal.Copy(pathBytes, 0, pathPtr, pathBytes.Length);

long handle = NativeMethods.zip_ease_lock_directory(pathPtr);
Marshal.FreeHGlobal(pathPtr);

if (handle == -1)
{
    var errPtr = NativeMethods.zip_ease_get_last_error();
    var msg = Marshal.PtrToStringUTF8(errPtr);
    NativeMethods.zip_ease_free_error_string(errPtr);
    throw new Exception($"Lock failed: {msg}");
}

// ... 執行操作 ...

// 解鎖
NativeMethods.zip_ease_unlock_directory(handle);
```

## 設計原則

- **Functional Paranoia**: 所有 `extern "C"` 函數使用 `catch_unwind` 防止 panic 跨 FFI 邊界
- **記憶體安全**: Rust 分配的記憶體必須由對應的 `free_*` 函數釋放
- **線程安全**: `LockManager` 使用 `Arc<Mutex<>>` 保護共享狀態
- **錯誤處理**: 失敗時設置 thread-local 錯誤，C# 端透過 `zip_ease_get_last_error()` 查詢
