using System;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// P/Invoke declarations and async helpers for the image preview FFI layer.
    /// Rust allocates pixel buffers — C# MUST call <see cref="FreeImageBuffer"/> in a finally block.
    /// </summary>
    internal static class ImagePreviewNative
    {
        private const string DllName = "zipease_core.dll";

        // ─── FFI Structs ──────────────────────────────────────────────────

        /// <summary>
        /// FFI-safe image result struct matching Rust's <c>ImageResultFFI</c>.
        /// Pixel buffer is Rust-allocated and must be freed via <see cref="FreeImageBuffer"/>.
        /// </summary>
        [StructLayout(LayoutKind.Sequential)]
        public struct ImageResultFFI
        {
            /// <summary>Pointer to RGBA pixel buffer (Rust-allocated).</summary>
            public IntPtr Pixels;

            /// <summary>Length of the pixel buffer in bytes.</summary>
            public UIntPtr PixelsLen;

            /// <summary>Image width in pixels.</summary>
            public uint Width;

            /// <summary>Image height in pixels.</summary>
            public uint Height;
        }

        // ─── P/Invoke Declarations ───────────────────────────────────────

        /// <summary>
        /// Decodes an image file to an RGBA pixel buffer.
        /// Returns 0 on success, negative error code on failure.
        /// On success, <paramref name="outResult"/> is populated with pixel data that
        /// MUST be freed with <see cref="FreeImageBuffer"/>.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_decode_image")]
        public static extern int DecodeImage(
            IntPtr filePathPtr,
            int filePathLen,
            ref ImageResultFFI outResult);

        /// <summary>
        /// Generates a thumbnail for an image file, fitting within max dimensions.
        /// Returns 0 on success, negative error code on failure.
        /// On success, <paramref name="outResult"/> is populated with pixel data that
        /// MUST be freed with <see cref="FreeImageBuffer"/>.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_generate_thumbnail")]
        public static extern int GenerateThumbnail(
            IntPtr filePathPtr,
            int filePathLen,
            uint maxWidth,
            uint maxHeight,
            ref ImageResultFFI outResult);

        /// <summary>
        /// Frees a pixel buffer previously allocated by Rust (from decode or thumbnail).
        /// MUST be called exactly once per successful decode/thumbnail call in a finally block.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "free_image_buffer")]
        public static extern void FreeImageBuffer(IntPtr ptr, UIntPtr len);

        /// <summary>
        /// Validates whether an image file's magic bytes match its claimed extension.
        /// Returns 0 if valid, negative error code if mismatch or error.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_validate_image_entry")]
        public static extern int ValidateImageEntry(
            IntPtr filePathPtr,
            int filePathLen,
            IntPtr extensionPtr,
            int extensionLen);

        // ─── Temp File Lifecycle ──────────────────────────────────────────

        /// <summary>
        /// Performs startup cleanup of stale temp files from previous sessions.
        /// Called on application startup (Requirement 10.4).
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_preview_startup_cleanup")]
        public static extern void StartupCleanup();

        /// <summary>
        /// Cleans up all preview temp files.
        /// Called on application exit (Requirement 10.3).
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_preview_cleanup_all_temps")]
        public static extern void CleanupAllTemps();

        /// <summary>
        /// Cleans up all preview temp files for a specific archive.
        /// Called when an archive is closed (Requirement 10.2).
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "zip_ease_preview_cleanup_archive")]
        public static extern void CleanupArchive(IntPtr archiveIdPtr, int archiveIdLen);

        // ─── Async Helpers ────────────────────────────────────────────────

        /// <summary>
        /// Decodes an image file asynchronously and returns a <see cref="WriteableBitmap"/>.
        /// The FFI call runs on a background thread; the bitmap is created on the UI thread.
        /// Rust-allocated memory is always freed in a finally block.
        /// </summary>
        /// <param name="filePath">Absolute path to the image file on disk.</param>
        /// <returns>A frozen <see cref="WriteableBitmap"/> containing the decoded RGBA image.</returns>
        /// <exception cref="ImagePreviewException">Thrown when decoding fails.</exception>
        public static async Task<WriteableBitmap> DecodeImageAsync(string filePath)
        {
            var (pixels, pixelsLen, width, height) = await Task.Run(() =>
            {
                var result = new ImageResultFFI();
                IntPtr pathPtr = IntPtr.Zero;

                try
                {
                    pathPtr = Marshal.StringToHGlobalUni(filePath);
                    int pathLen = filePath.Length;

                    int returnCode = DecodeImage(pathPtr, pathLen, ref result);

                    if (returnCode < 0)
                    {
                        string errorMessage = GetLastErrorMessage();
                        throw new ImagePreviewException(errorMessage, returnCode);
                    }

                    // Copy pixel data before returning so we can free Rust memory here
                    int bufferLen = (int)(uint)result.PixelsLen;
                    byte[] pixelData = new byte[bufferLen];
                    Marshal.Copy(result.Pixels, pixelData, 0, bufferLen);

                    return (pixelData, bufferLen, result.Width, result.Height);
                }
                finally
                {
                    // Always free Rust-allocated pixel buffer
                    if (result.Pixels != IntPtr.Zero)
                    {
                        FreeImageBuffer(result.Pixels, result.PixelsLen);
                    }

                    if (pathPtr != IntPtr.Zero)
                    {
                        Marshal.FreeHGlobal(pathPtr);
                    }
                }
            });

            // Create WriteableBitmap on UI thread
            return await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
            {
                var bitmap = new WriteableBitmap(
                    (int)width,
                    (int)height,
                    96, 96,
                    PixelFormats.Bgra32,
                    null);

                // RGBA → BGRA channel swap (Rust outputs RGBA, WPF expects BGRA)
                SwapRgbaToBgra(pixels);

                bitmap.Lock();
                try
                {
                    Marshal.Copy(pixels, 0, bitmap.BackBuffer, pixelsLen);
                    bitmap.AddDirtyRect(new Int32Rect(0, 0, (int)width, (int)height));
                }
                finally
                {
                    bitmap.Unlock();
                }

                bitmap.Freeze();
                return bitmap;
            }, DispatcherPriority.Normal);
        }

        /// <summary>
        /// Generates a thumbnail asynchronously and returns a <see cref="WriteableBitmap"/>.
        /// The FFI call runs on a background thread; the bitmap is created on the UI thread.
        /// Rust-allocated memory is always freed in a finally block.
        /// </summary>
        /// <param name="filePath">Absolute path to the image file on disk.</param>
        /// <param name="maxWidth">Maximum thumbnail width in pixels.</param>
        /// <param name="maxHeight">Maximum thumbnail height in pixels.</param>
        /// <returns>A frozen <see cref="WriteableBitmap"/> containing the thumbnail.</returns>
        /// <exception cref="ImagePreviewException">Thrown when thumbnail generation fails.</exception>
        public static async Task<WriteableBitmap> GenerateThumbnailAsync(string filePath, uint maxWidth = 64, uint maxHeight = 64)
        {
            var (pixels, pixelsLen, width, height) = await Task.Run(() =>
            {
                var result = new ImageResultFFI();
                IntPtr pathPtr = IntPtr.Zero;

                try
                {
                    pathPtr = Marshal.StringToHGlobalUni(filePath);
                    int pathLen = filePath.Length;

                    int returnCode = GenerateThumbnail(pathPtr, pathLen, maxWidth, maxHeight, ref result);

                    if (returnCode < 0)
                    {
                        string errorMessage = GetLastErrorMessage();
                        throw new ImagePreviewException(errorMessage, returnCode);
                    }

                    // Copy pixel data before returning so we can free Rust memory here
                    int bufferLen = (int)(uint)result.PixelsLen;
                    byte[] pixelData = new byte[bufferLen];
                    Marshal.Copy(result.Pixels, pixelData, 0, bufferLen);

                    return (pixelData, bufferLen, result.Width, result.Height);
                }
                finally
                {
                    // Always free Rust-allocated pixel buffer
                    if (result.Pixels != IntPtr.Zero)
                    {
                        FreeImageBuffer(result.Pixels, result.PixelsLen);
                    }

                    if (pathPtr != IntPtr.Zero)
                    {
                        Marshal.FreeHGlobal(pathPtr);
                    }
                }
            });

            // Create WriteableBitmap on UI thread
            return await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
            {
                var bitmap = new WriteableBitmap(
                    (int)width,
                    (int)height,
                    96, 96,
                    PixelFormats.Bgra32,
                    null);

                // RGBA → BGRA channel swap (Rust outputs RGBA, WPF expects BGRA)
                SwapRgbaToBgra(pixels);

                bitmap.Lock();
                try
                {
                    Marshal.Copy(pixels, 0, bitmap.BackBuffer, pixelsLen);
                    bitmap.AddDirtyRect(new Int32Rect(0, 0, (int)width, (int)height));
                }
                finally
                {
                    bitmap.Unlock();
                }

                bitmap.Freeze();
                return bitmap;
            }, DispatcherPriority.Normal);
        }

        // ─── Private Helpers ──────────────────────────────────────────────

        /// <summary>
        /// Retrieves the last error message from Rust's thread-local storage.
        /// </summary>
        private static string GetLastErrorMessage()
        {
            IntPtr ptr = NativeMethods.GetLastError();
            if (ptr == IntPtr.Zero) return "Unknown error";

            string? message = Marshal.PtrToStringUTF8(ptr);
            NativeMethods.FreeErrorString(ptr);
            return message ?? "Unknown error";
        }

        /// <summary>
        /// Swaps RGBA pixel data to BGRA in-place for WPF compatibility.
        /// WPF's <see cref="PixelFormats.Bgra32"/> expects B-G-R-A byte order,
        /// while Rust's image crate outputs R-G-B-A.
        /// </summary>
        private static void SwapRgbaToBgra(byte[] pixels)
        {
            for (int i = 0; i < pixels.Length - 3; i += 4)
            {
                // Swap R and B channels (indices 0 and 2)
                (pixels[i], pixels[i + 2]) = (pixels[i + 2], pixels[i]);
            }
        }
    }

    /// <summary>
    /// Exception thrown when an image preview FFI operation fails.
    /// Contains the user-friendly error message from Rust (no internal codes or stack traces).
    /// </summary>
    internal class ImagePreviewException : Exception
    {
        /// <summary>The negative FFI error code returned by Rust.</summary>
        public int ErrorCode { get; }

        public ImagePreviewException(string message, int errorCode)
            : base(message)
        {
            ErrorCode = errorCode;
        }
    }
}
