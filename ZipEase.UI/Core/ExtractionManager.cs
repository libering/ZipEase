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
    }
}
