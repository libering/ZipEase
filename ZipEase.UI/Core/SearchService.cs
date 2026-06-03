using System;
using System.Runtime.InteropServices;
using System.Threading;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Managed wrapper around the Rust search FFI.
    /// Marshals pattern to UTF-16, manages cancel flag via GCHandle,
    /// and always frees native results in a finally block.
    /// </summary>
    internal static class SearchService
    {
        /// <summary>
        /// Searches archive entries using the Rust search engine.
        /// </summary>
        /// <param name="pattern">Search pattern (substring or glob with * / ?)</param>
        /// <param name="entriesPtr">Pointer to the native ArchiveEntryFFI array</param>
        /// <param name="entryCount">Number of entries in the array</param>
        /// <param name="cancellationToken">Token to cancel the search</param>
        /// <returns>Array of matching entry indices</returns>
        /// <exception cref="OperationCanceledException">Thrown when search is cancelled (FFI returns -2)</exception>
        /// <exception cref="InvalidOperationException">Thrown when FFI returns an error code</exception>
        public static int[] Search(
            string pattern,
            IntPtr entriesPtr,
            int entryCount,
            CancellationToken cancellationToken)
        {
            if (string.IsNullOrEmpty(pattern))
                return Array.Empty<int>();

            // Marshal pattern to UTF-16 (null-terminated)
            IntPtr patternPtr = Marshal.StringToHGlobalUni(pattern);

            // Allocate pinned cancel flag so Rust can read it safely
            int cancelFlag = 0;
            GCHandle cancelHandle = GCHandle.Alloc(cancelFlag, GCHandleType.Pinned);
            IntPtr cancelFlagPtr = cancelHandle.AddrOfPinnedObject();

            // Register cancellation callback to set the flag
            using var registration = cancellationToken.Register(() =>
            {
                Marshal.WriteInt32(cancelFlagPtr, 1);
            });

            IntPtr outIndicesPtr = IntPtr.Zero;
            int outCount = 0;

            try
            {
                int result = NativeMethods.SearchEntries(
                    patternPtr,
                    entriesPtr,
                    entryCount,
                    cancelFlagPtr,
                    out outIndicesPtr,
                    out outCount);

                if (result == -2)
                    throw new OperationCanceledException("Search was cancelled.");

                if (result < 0)
                    throw new InvalidOperationException($"Search FFI returned error code: {result}");

                // Read result indices into managed array
                if (outCount <= 0 || outIndicesPtr == IntPtr.Zero)
                    return Array.Empty<int>();

                int[] indices = new int[outCount];
                Marshal.Copy(outIndicesPtr, indices, 0, outCount);
                return indices;
            }
            finally
            {
                // Always free native search results
                if (outIndicesPtr != IntPtr.Zero)
                    NativeMethods.FreeSearchResults(outIndicesPtr, outCount);

                // Free marshalled pattern string
                Marshal.FreeHGlobal(patternPtr);

                // Release pinned cancel flag
                if (cancelHandle.IsAllocated)
                    cancelHandle.Free();
            }
        }
    }
}
