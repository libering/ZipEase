using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;

namespace ZipEase.UI.Core
{
    public class CompressionService
    {
        /// <summary>
        /// Compress a list of input paths into an archive.
        /// C# owns all input path pointers and frees them in finally blocks.
        /// Progress callback fires on the Rust thread — callers must use Dispatcher.BeginInvoke.
        /// </summary>
        public async Task CompressAsync(
            IReadOnlyList<string> inputPaths,
            string outputPath,
            int level,
            IProgress<(int Pct, string File)>? progress,
            CancellationToken ct = default,
            string? password = null)
        {
            if (inputPaths == null || inputPaths.Count == 0)
                throw new ArgumentException("At least one input path is required.", nameof(inputPaths));
            if (string.IsNullOrEmpty(outputPath))
                throw new ArgumentException("Output path cannot be null or empty.", nameof(outputPath));

            // Build the callback — must be kept alive for the duration of the native call
            NativeMethods.CompressProgressCallback? callback = null;
            if (progress != null)
            {
                callback = (pct, ptr) =>
                {
                    var file = ptr != IntPtr.Zero
                        ? Marshal.PtrToStringUni(ptr) ?? string.Empty
                        : string.Empty;
                    progress.Report((pct, file));
                };
            }

            // Pin the delegate to prevent GC collection during the native call
            GCHandle callbackHandle = callback != null
                ? GCHandle.Alloc(callback)
                : default;

            // Allocate UTF-16 pointers for each input path (C# owns these)
            var inputPtrs = new IntPtr[inputPaths.Count];
            try
            {
                for (int i = 0; i < inputPaths.Count; i++)
                    inputPtrs[i] = Marshal.StringToHGlobalUni(inputPaths[i]);

                int result = await Task.Run(() =>
                    NativeMethods.Compress(inputPtrs, inputPtrs.Length, outputPath, level, password, callback), ct);

                if (result != 0)
                {
                    IntPtr errPtr = NativeMethods.GetLastError();
                    string message = errPtr != IntPtr.Zero
                        ? Marshal.PtrToStringUTF8(errPtr) ?? "Unknown compression error"
                        : "Unknown compression error";
                    if (errPtr != IntPtr.Zero) NativeMethods.FreeErrorString(errPtr);
                    throw new CompressionException(message, result);
                }
            }
            finally
            {
                // C# frees all input path pointers
                foreach (var ptr in inputPtrs)
                    if (ptr != IntPtr.Zero) Marshal.FreeHGlobal(ptr);

                if (callbackHandle.IsAllocated)
                    callbackHandle.Free();
            }
        }
    }
}
