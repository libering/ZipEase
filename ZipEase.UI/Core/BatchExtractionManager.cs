using System;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Managed wrapper for batch extraction FFI calls.
    /// Pure FFI bridge — no business logic. GCHandle lifecycle is managed here
    /// to prevent memory leaks and ensure the callback delegate stays alive
    /// for the duration of the native call.
    /// </summary>
    public static class BatchExtractionManager
    {
        /// <summary>
        /// Managed progress handler delegate for batch extraction.
        /// Callers receive marshalled C# types (string, not IntPtr).
        /// </summary>
        /// <param name="archiveIndex">Zero-based index of the current archive.</param>
        /// <param name="archiveCount">Total number of archives in the batch.</param>
        /// <param name="filePercent">Extraction progress of the current archive (0-100).</param>
        /// <param name="currentFileName">Name of the file currently being extracted.</param>
        public delegate void BatchProgressHandler(
            uint archiveIndex,
            uint archiveCount,
            int filePercent,
            string currentFileName);

        /// <summary>
        /// Executes batch extraction on a background thread, reporting progress via callback.
        /// Maps CancellationToken to a pinned int (cancel flag) for FFI interop.
        /// All GCHandles are freed in a finally block to prevent memory leaks.
        /// </summary>
        /// <param name="archivePaths">Array of archive file paths to extract.</param>
        /// <param name="outputDir">Target output directory for all archives.</param>
        /// <param name="progressHandler">Optional managed progress callback.</param>
        /// <param name="cancellationToken">Token to signal cancellation.</param>
        /// <returns>Number of successfully extracted archives (≥ 0).</returns>
        /// <exception cref="ArgumentException">Thrown when archivePaths or outputDir is null/empty.</exception>
        /// <exception cref="ExtractionException">Thrown when the native call returns a negative error code.</exception>
        public static Task<int> ExtractBatchAsync(
            string[] archivePaths,
            string outputDir,
            BatchProgressHandler? progressHandler = null,
            CancellationToken cancellationToken = default)
        {
            if (archivePaths == null || archivePaths.Length == 0)
                throw new ArgumentException("Archive paths cannot be null or empty", nameof(archivePaths));

            if (string.IsNullOrEmpty(outputDir))
                throw new ArgumentException("Output directory cannot be null or empty", nameof(outputDir));

            return Task.Run(() => ExtractBatch(archivePaths, outputDir, progressHandler, cancellationToken));
        }

        private static int ExtractBatch(
            string[] archivePaths,
            string outputDir,
            BatchProgressHandler? progressHandler,
            CancellationToken cancellationToken)
        {
            // --- Pin the cancel flag (int) so Rust can read it via pointer ---
            int cancelFlag = 0;
            GCHandle cancelHandle = GCHandle.Alloc(cancelFlag, GCHandleType.Pinned);

            // --- Register CancellationToken callback to set the flag ---
            CancellationTokenRegistration ctReg = default;
            if (cancellationToken.CanBeCanceled)
            {
                ctReg = cancellationToken.Register(() =>
                {
                    Marshal.WriteInt32(cancelHandle.AddrOfPinnedObject(), 1);
                });
            }

            // --- Build native callback wrapper ---
            NativeMethods.BatchProgressCallback? nativeCallback = null;
            GCHandle callbackHandle = default;

            if (progressHandler != null)
            {
                nativeCallback = (archiveIndex, archiveCount, filePercent, currentFileNamePtr) =>
                {
                    try
                    {
                        string fileName = currentFileNamePtr != IntPtr.Zero
                            ? Marshal.PtrToStringUni(currentFileNamePtr) ?? string.Empty
                            : string.Empty;

                        progressHandler.Invoke(archiveIndex, archiveCount, filePercent, fileName);
                    }
                    catch
                    {
                        // Swallow exceptions — must not cross FFI boundary
                    }
                };

                callbackHandle = GCHandle.Alloc(nativeCallback);
            }

            // --- Marshal archive paths to UTF-16 IntPtr array ---
            IntPtr[] pathPtrs = new IntPtr[archivePaths.Length];
            try
            {
                for (int i = 0; i < archivePaths.Length; i++)
                {
                    pathPtrs[i] = Marshal.StringToHGlobalUni(archivePaths[i]);
                }

                IntPtr callbackPtr = nativeCallback != null
                    ? Marshal.GetFunctionPointerForDelegate(nativeCallback)
                    : IntPtr.Zero;

                IntPtr cancelFlagPtr = cancelHandle.AddrOfPinnedObject();

                int result = NativeMethods.BatchExtract(
                    pathPtrs,
                    archivePaths.Length,
                    outputDir,
                    callbackPtr,
                    cancelFlagPtr);

                if (result < 0)
                {
                    string errorMessage = GetLastErrorMessage();
                    throw new ExtractionException(errorMessage, result);
                }

                return result;
            }
            finally
            {
                // --- Free all marshalled path strings ---
                for (int i = 0; i < pathPtrs.Length; i++)
                {
                    if (pathPtrs[i] != IntPtr.Zero)
                        Marshal.FreeHGlobal(pathPtrs[i]);
                }

                // --- Dispose CancellationToken registration ---
                ctReg.Dispose();

                // --- Free GCHandles ---
                if (callbackHandle.IsAllocated)
                    callbackHandle.Free();

                if (cancelHandle.IsAllocated)
                    cancelHandle.Free();
            }
        }

        private static string GetLastErrorMessage()
        {
            IntPtr ptr = NativeMethods.GetLastError();
            if (ptr == IntPtr.Zero) return "Unknown error";

            string? message = Marshal.PtrToStringUTF8(ptr);
            NativeMethods.FreeErrorString(ptr);
            return message ?? "Unknown error";
        }
    }
}
