using System;
using System.Runtime.InteropServices;

namespace ZipEase.UI.Core
{
    internal static class NativeMethods
    {
        private const string DllName = "zipease_core.dll";

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_lock_directory")]
        public static extern IntPtr LockDirectory([MarshalAs(UnmanagedType.LPWStr)] string path);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_unlock_directory")]
        public static extern int UnlockDirectory(IntPtr handle);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_get_last_error")]
        public static extern IntPtr GetLastError();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_free_error_string")]
        public static extern void FreeErrorString(IntPtr ptr);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_extract")]
        public static extern int Extract(
            [MarshalAs(UnmanagedType.LPWStr)] string archivePath,
            [MarshalAs(UnmanagedType.LPWStr)] string outputDir
        );

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_extract_with_progress")]
        public static extern int ExtractWithProgress(
            [MarshalAs(UnmanagedType.LPWStr)] string archivePath,
            [MarshalAs(UnmanagedType.LPWStr)] string outputDir,
            IntPtr progressCallback
        );

        [StructLayout(LayoutKind.Sequential)]
        public struct ArchiveEntryFFI
        {
            public IntPtr FileNamePtr;  // UTF-16 null-terminated string pointer
            public long FileSize;
            public int IsDirectory;     // 1 = directory, 0 = file
        }

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_list_archive_contents")]
        public static extern int ListArchiveContents(
            [MarshalAs(UnmanagedType.LPWStr)] string archivePath,
            out IntPtr outEntriesPtr,
            out int outCount
        );

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_free_archive_entries")]
        public static extern void FreeArchiveEntries(IntPtr entriesPtr, int count);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_list_archive_contents_with_password")]
        public static extern int ListArchiveContentsWithPassword(
            [MarshalAs(UnmanagedType.LPWStr)] string archivePath,
            [MarshalAs(UnmanagedType.LPWStr)] string? password,
            out IntPtr outEntriesPtr,
            out int outCount
        );

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_extract_with_password")]
        public static extern int ExtractWithPassword(
            [MarshalAs(UnmanagedType.LPWStr)] string archivePath,
            [MarshalAs(UnmanagedType.LPWStr)] string outputDir,
            [MarshalAs(UnmanagedType.LPWStr)] string? password,
            IntPtr progressCallback
        );

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        public delegate void CompressProgressCallback(int percentage, IntPtr currentFilePtr);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_compress")]
        public static extern int Compress(
            IntPtr[] inputPathPtrs,
            int inputCount,
            [MarshalAs(UnmanagedType.LPWStr)] string outputPath,
            int level,
            CompressProgressCallback? progressCallback
        );

        /// <summary>
        /// Extracts a ZIP archive ignoring CRC errors (force/recovery mode).
        /// Caller provides optional progress callback; pass IntPtr.Zero to omit.
        /// Returns 0 on success, negative error code on failure.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_extract_force")]
        public static extern int ExtractForce(
            [MarshalAs(UnmanagedType.LPWStr)] string archivePath,
            [MarshalAs(UnmanagedType.LPWStr)] string outputDir,
            IntPtr progressCallback
        );

        /// <summary>
        /// Extracts a single entry by zero-based index from a ZIP archive.
        /// On success (returns 0), *outNamePtr is set to a Rust-allocated UTF-16 string
        /// that MUST be freed with <see cref="FreeString"/>.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_extract_entry")]
        public static extern int ExtractEntry(
            [MarshalAs(UnmanagedType.LPWStr)] string archivePath,
            uint entryIndex,
            [MarshalAs(UnmanagedType.LPWStr)] string outputDir,
            out IntPtr outNamePtr
        );

        /// <summary>
        /// Frees a UTF-16 string allocated by Rust FFI (e.g. returned by ExtractEntry).
        /// MUST be called in a finally block to prevent memory leaks.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_free_string")]
        public static extern void FreeString(IntPtr ptr);

        /// <summary>
        /// Moves a file to the Windows Recycle Bin.
        /// Returns 0 on success, -1 on panic, -2 on any other error.
        /// No memory is allocated by Rust for this call — no free function needed.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_trash_file")]
        public static extern int ZipEaseTrashFile(
            [MarshalAs(UnmanagedType.LPWStr)] string path);

        /// <summary>
        /// Dispatches a success toast notification (fire-and-forget).
        /// All errors are discarded silently in Rust — this call always returns.
        /// No memory is allocated by Rust; no free function needed.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl,
                   EntryPoint = "zip_ease_notify_success")]
        public static extern void ZipEaseNotifySuccess(
            [MarshalAs(UnmanagedType.LPWStr)] string archiveName,
            [MarshalAs(UnmanagedType.LPWStr)] string outputFolder,
            int fileCount);

        /// <summary>
        /// Dispatches a failure toast notification (fire-and-forget).
        /// All errors are discarded silently in Rust — this call always returns.
        /// No memory is allocated by Rust; no free function needed.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl,
                   EntryPoint = "zip_ease_notify_failure")]
        public static extern void ZipEaseNotifyFailure(
            [MarshalAs(UnmanagedType.LPWStr)] string archiveName,
            [MarshalAs(UnmanagedType.LPWStr)] string errorMsg);

        /// <summary>
        /// Returns a comma-separated list of process names that currently hold a lock on
        /// the specified file path, or <see cref="System.IntPtr.Zero"/> if no lock holders
        /// are found or the query fails.
        /// The returned pointer MUST be freed with <see cref="FreeString"/> in a finally block.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl,
                   EntryPoint = "zip_ease_who_locks")]
        public static extern System.IntPtr ZipEaseWhoLocks(
            [MarshalAs(UnmanagedType.LPWStr)] string path);
    }
}
