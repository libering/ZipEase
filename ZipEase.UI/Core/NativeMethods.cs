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
    }
}
