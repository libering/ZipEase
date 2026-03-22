using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Delegate for progress updates during extraction.
    /// </summary>
    /// <param name="percentage">Progress percentage (0-100).</param>
    /// <param name="currentFile">Name of the file currently being extracted.</param>
    public delegate void ProgressCallback(int percentage, string currentFile);

    /// <summary>
    /// Manages archive extraction operations with FFI to Rust core.
    /// </summary>
    public static class ExtractionManager
    {
        /// <summary>
        /// Unmanaged function pointer delegate matching Rust callback signature.
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate void NativeProgressCallback(int percentage, IntPtr fileNamePtr);

        /// <summary>
        /// Extracts an archive asynchronously with optional password and progress reporting.
        /// </summary>
        public static Task<int> ExtractAsync(
            string archivePath,
            string outputDir,
            string? password = null,
            ProgressCallback? progressCallback = null)
        {
            return Task.Run(() => Extract(archivePath, outputDir, password, progressCallback));
        }

        /// <summary>
        /// Extracts an archive synchronously with optional password and progress reporting.
        /// </summary>
        private static int Extract(
            string archivePath,
            string outputDir,
            string? password,
            ProgressCallback? progressCallback)
        {
            if (string.IsNullOrEmpty(archivePath))
                throw new ArgumentException("Archive path cannot be null or empty", nameof(archivePath));

            if (string.IsNullOrEmpty(outputDir))
                throw new ArgumentException("Output directory cannot be null or empty", nameof(outputDir));

            if (!File.Exists(archivePath))
                throw new FileNotFoundException("Archive file not found", archivePath);

            // Create native callback wrapper
            NativeProgressCallback? nativeCallback = null;
            GCHandle callbackHandle = default;

            if (progressCallback != null)
            {
                nativeCallback = (percentage, fileNamePtr) =>
                {
                    try
                    {
                        if (percentage < 0 || percentage > 100)
                            return;

                        string fileName = fileNamePtr != IntPtr.Zero
                            ? Marshal.PtrToStringUni(fileNamePtr) ?? "Unknown"
                            : "Unknown";

                        progressCallback.Invoke(percentage, fileName);
                    }
                    catch
                    {
                        // Swallow exceptions in callback to prevent crossing FFI boundary
                    }
                };

                callbackHandle = GCHandle.Alloc(nativeCallback);
            }

            try
            {
                IntPtr callbackPtr = nativeCallback != null
                    ? Marshal.GetFunctionPointerForDelegate(nativeCallback)
                    : IntPtr.Zero;

                int result = password != null
                    ? NativeMethods.ExtractWithPassword(archivePath, outputDir, password, callbackPtr)
                    : NativeMethods.ExtractWithProgress(archivePath, outputDir, callbackPtr);

                if (result < 0)
                {
                    string errorMessage = GetLastErrorMessage();
                    throw new ExtractionException(errorMessage, result);
                }

                return result;
            }
            finally
            {
                if (callbackHandle.IsAllocated)
                    callbackHandle.Free();
            }
        }

        private static string GetLastErrorMessage()
        {
            IntPtr ptr = NativeMethods.GetLastError();
            if (ptr == IntPtr.Zero) return "Unknown error";

            string? message = Marshal.PtrToStringUni(ptr);
            NativeMethods.FreeErrorString(ptr);
            return message ?? "Unknown error";
        }

        /// <summary>
        /// Extracts a ZIP archive ignoring CRC errors (force/recovery mode).
        /// Best-effort: skips unreadable entries rather than failing.
        /// Caller is responsible for ensuring outputDir exists or is creatable.
        /// </summary>
        public static Task<int> ExtractForceAsync(
            string archivePath,
            string outputDir,
            ProgressCallback? progressCallback = null)
        {
            return Task.Run(() =>
            {
                if (string.IsNullOrEmpty(archivePath))
                    throw new ArgumentException("Archive path cannot be null or empty", nameof(archivePath));
                if (string.IsNullOrEmpty(outputDir))
                    throw new ArgumentException("Output directory cannot be null or empty", nameof(outputDir));
                if (!File.Exists(archivePath))
                    throw new FileNotFoundException("Archive file not found", archivePath);

                NativeProgressCallback? nativeCallback = null;
                GCHandle callbackHandle = default;

                if (progressCallback != null)
                {
                    nativeCallback = (percentage, fileNamePtr) =>
                    {
                        try
                        {
                            if (percentage < 0 || percentage > 100) return;
                            string fileName = fileNamePtr != IntPtr.Zero
                                ? Marshal.PtrToStringUni(fileNamePtr) ?? "Unknown"
                                : "Unknown";
                            progressCallback.Invoke(percentage, fileName);
                        }
                        catch { /* swallow — must not cross FFI boundary */ }
                    };
                    callbackHandle = GCHandle.Alloc(nativeCallback);
                }

                try
                {
                    IntPtr callbackPtr = nativeCallback != null
                        ? Marshal.GetFunctionPointerForDelegate(nativeCallback)
                        : IntPtr.Zero;

                    int result = NativeMethods.ExtractForce(archivePath, outputDir, callbackPtr);
                    if (result < 0)
                    {
                        string errorMessage = GetLastErrorMessage();
                        throw new ExtractionException(errorMessage, result);
                    }
                    return result;
                }
                finally
                {
                    if (callbackHandle.IsAllocated)
                        callbackHandle.Free();
                }
            });
        }

        /// <summary>
        /// Moves the specified file to the Windows Recycle Bin on a background thread.
        /// Returns 0 on success, non-zero on failure.
        /// All file-system logic is in Rust — this method is a pure FFI bridge.
        /// </summary>
        public static async Task<int> TrashFileAsync(string path)
            => await Task.Run(() => NativeMethods.ZipEaseTrashFile(path));

        /// <summary>
        /// Queries which processes hold a lock on the given file path, on a background thread.
        /// Returns IntPtr.Zero if no lock holders are found or if the query fails for any reason.
        /// The returned pointer MUST be freed with NativeMethods.FreeString in a finally block,
        /// regardless of whether the pointer is null or non-null.
        /// </summary>
        public static async Task<IntPtr> WhoLocksAsync(string path)
            => await Task.Run(() => NativeMethods.ZipEaseWhoLocks(path));

        /// <summary>
        /// Dispatches a success toast notification on a background thread (fire-and-forget).
        /// All errors are discarded silently in Rust — this task always completes.
        /// </summary>
        public static async System.Threading.Tasks.Task NotifySuccessAsync(
            string archiveName, string outputFolder, int fileCount)
            => await Task.Run(() => NativeMethods.ZipEaseNotifySuccess(archiveName, outputFolder, fileCount));

        /// <summary>
        /// Dispatches a failure toast notification on a background thread (fire-and-forget).
        /// All errors are discarded silently in Rust — this task always completes.
        /// </summary>
        public static async System.Threading.Tasks.Task NotifyFailureAsync(
            string archiveName, string errorMsg)
            => await Task.Run(() => NativeMethods.ZipEaseNotifyFailure(archiveName, errorMsg));

        /// <summary>
        /// Extracts a single entry by zero-based index from a ZIP archive to outputDir.
        /// Returns the extracted filename on success.
        /// Memory contract: Rust allocates the name string; this method frees it via
        /// NativeMethods.FreeString in a finally block.
        /// </summary>
        public static Task<string> ExtractEntryAsync(
            string archivePath,
            uint entryIndex,
            string outputDir)
        {
            return Task.Run(() =>
            {
                if (string.IsNullOrEmpty(archivePath))
                    throw new ArgumentException("Archive path cannot be null or empty", nameof(archivePath));
                if (string.IsNullOrEmpty(outputDir))
                    throw new ArgumentException("Output directory cannot be null or empty", nameof(outputDir));
                if (!File.Exists(archivePath))
                    throw new FileNotFoundException("Archive file not found", archivePath);

                IntPtr outNamePtr = IntPtr.Zero;
                try
                {
                    int result = NativeMethods.ExtractEntry(archivePath, entryIndex, outputDir, out outNamePtr);
                    if (result < 0)
                    {
                        string errorMessage = GetLastErrorMessage();
                        throw new ExtractionException(errorMessage, result);
                    }
                    return outNamePtr != IntPtr.Zero
                        ? Marshal.PtrToStringUni(outNamePtr) ?? string.Empty
                        : string.Empty;
                }
                finally
                {
                    // Always free Rust-allocated string, even on exception
                    if (outNamePtr != IntPtr.Zero)
                        NativeMethods.FreeString(outNamePtr);
                }
            });
        }
    }
}
