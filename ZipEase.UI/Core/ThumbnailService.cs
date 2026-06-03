using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Media.Imaging;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Coordinates thumbnail generation for archive file list entries.
    /// Limits concurrent FFI operations to 4 via semaphore, skips oversized entries,
    /// and caches results. Contains zero business logic — delegates all decoding
    /// to the Rust backend via <see cref="ImagePreviewNative.GenerateThumbnailAsync"/>.
    /// </summary>
    internal sealed class ThumbnailService : IDisposable
    {
        // ─── Constants ────────────────────────────────────────────────────

        /// <summary>Maximum concurrent thumbnail generation operations.</summary>
        private const int MaxConcurrency = 4;

        /// <summary>Maximum compressed size in bytes (100 MB). Entries larger than this are skipped.</summary>
        private const long MaxCompressedSizeBytes = 100L * 1024 * 1024;

        /// <summary>Thumbnail target dimensions (64×64 pixels).</summary>
        private const uint ThumbnailMaxWidth = 64;
        private const uint ThumbnailMaxHeight = 64;

        /// <summary>
        /// Supported image extensions for preview (case-insensitive).
        /// Matches the Rust backend's SUPPORTED_EXTENSIONS list.
        /// </summary>
        private static readonly HashSet<string> SupportedExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            "png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "ico"
        };

        // ─── State ────────────────────────────────────────────────────────

        private readonly SemaphoreSlim _semaphore = new(MaxConcurrency, MaxConcurrency);
        private readonly ConcurrentDictionary<string, Task<WriteableBitmap?>> _inProgress = new();
        private readonly ConcurrentDictionary<string, WriteableBitmap?> _cache = new();
        private CancellationTokenSource _cts = new();
        private bool _disposed;

        // ─── Public Methods ───────────────────────────────────────────────

        /// <summary>
        /// Determines whether an archive entry is previewable based on its file name extension.
        /// Returns false for directories and names without a dot character.
        /// </summary>
        /// <param name="entryName">The archive entry file name (may include path separators).</param>
        /// <param name="isDirectory">Whether the entry is a directory.</param>
        /// <returns>True if the entry is a supported image format.</returns>
        public static bool IsPreviewable(string entryName, bool isDirectory = false)
        {
            if (isDirectory || string.IsNullOrEmpty(entryName))
                return false;

            // Extract the file name portion (after last path separator)
            string fileName = entryName;
            int lastSep = entryName.LastIndexOfAny(new[] { '/', '\\' });
            if (lastSep >= 0)
                fileName = entryName[(lastSep + 1)..];

            // Find last dot in file name
            int lastDot = fileName.LastIndexOf('.');
            if (lastDot < 0 || lastDot == fileName.Length - 1)
                return false;

            string extension = fileName[(lastDot + 1)..];
            return SupportedExtensions.Contains(extension);
        }

        /// <summary>
        /// Generates a thumbnail for the specified file path asynchronously.
        /// Returns a cached result if available, otherwise queues generation.
        /// Returns null on failure (no error propagated to caller).
        /// </summary>
        /// <param name="filePath">Absolute path to the extracted image file on disk.</param>
        /// <param name="compressedSizeBytes">Compressed size of the entry in the archive.</param>
        /// <returns>A 64×64 (max) WriteableBitmap thumbnail, or null if generation failed or was skipped.</returns>
        public async Task<WriteableBitmap?> GetThumbnailAsync(string filePath, long compressedSizeBytes = 0)
        {
            if (_disposed) return null;

            // Skip entries exceeding 100 MB compressed size
            if (compressedSizeBytes > MaxCompressedSizeBytes)
                return null;

            // Return cached result if available
            if (_cache.TryGetValue(filePath, out var cached))
                return cached;

            // Deduplicate: if already in progress, await the existing task
            var task = _inProgress.GetOrAdd(filePath, path => GenerateThumbnailCoreAsync(path));

            try
            {
                return await task;
            }
            catch (OperationCanceledException)
            {
                return null;
            }
        }

        /// <summary>
        /// Requests thumbnail generation for a batch of visible entries.
        /// Prioritizes entries in the order provided (visible-area first).
        /// Non-previewable or oversized entries are silently skipped.
        /// </summary>
        /// <param name="entries">Visible entries to generate thumbnails for, in priority order.</param>
        /// <param name="onThumbnailReady">
        /// Callback invoked on the UI thread when a thumbnail is ready.
        /// Parameters: entry file name, generated bitmap (or null on failure).
        /// </param>
        public void RequestThumbnailsForVisibleEntries(
            IEnumerable<ArchiveEntryViewModel> entries,
            Action<string, WriteableBitmap?> onThumbnailReady)
        {
            if (_disposed) return;

            var token = _cts.Token;

            foreach (var entry in entries)
            {
                if (token.IsCancellationRequested) break;
                if (entry.IsDirectory) continue;
                if (!IsPreviewable(entry.FileName)) continue;
                if (entry.SizeBytes > MaxCompressedSizeBytes) continue;

                // Already cached — invoke callback immediately
                if (_cache.TryGetValue(entry.FileName, out var cached))
                {
                    onThumbnailReady(entry.FileName, cached);
                    continue;
                }

                // Fire-and-forget background generation
                string fileName = entry.FileName;
                long sizeBytes = entry.SizeBytes;
                _ = Task.Run(async () =>
                {
                    try
                    {
                        var bitmap = await GetThumbnailAsync(fileName, sizeBytes);
                        if (!token.IsCancellationRequested)
                        {
                            System.Windows.Application.Current?.Dispatcher.BeginInvoke(() =>
                            {
                                onThumbnailReady(fileName, bitmap);
                            });
                        }
                    }
                    catch (Exception)
                    {
                        // Silently swallow — keep placeholder on failure (Req 5.5)
                    }
                }, token);
            }
        }

        /// <summary>
        /// Cancels all pending thumbnail operations and clears the cache.
        /// Called when the archive is closed or a new archive is opened.
        /// </summary>
        public void CancelAndClear()
        {
            _cts.Cancel();
            _cts.Dispose();
            _cts = new CancellationTokenSource();
            _inProgress.Clear();
            _cache.Clear();
        }

        /// <inheritdoc/>
        public void Dispose()
        {
            if (_disposed) return;
            _disposed = true;
            _cts.Cancel();
            _cts.Dispose();
            _semaphore.Dispose();
        }

        // ─── Private Helpers ──────────────────────────────────────────────

        /// <summary>
        /// Core thumbnail generation with semaphore-limited concurrency.
        /// Calls the Rust backend via FFI and caches the result.
        /// Returns null on any failure (no error shown to user).
        /// </summary>
        private async Task<WriteableBitmap?> GenerateThumbnailCoreAsync(string filePath)
        {
            var token = _cts.Token;

            try
            {
                // Wait for semaphore slot (limit to 4 concurrent operations)
                await _semaphore.WaitAsync(token);
            }
            catch (OperationCanceledException)
            {
                _inProgress.TryRemove(filePath, out _);
                return null;
            }

            try
            {
                token.ThrowIfCancellationRequested();

                // Call Rust backend for thumbnail generation
                var bitmap = await ImagePreviewNative.GenerateThumbnailAsync(
                    filePath, ThumbnailMaxWidth, ThumbnailMaxHeight);

                // Cache the result
                _cache[filePath] = bitmap;
                return bitmap;
            }
            catch (OperationCanceledException)
            {
                return null;
            }
            catch (Exception)
            {
                // On failure: cache null so we don't retry, keep placeholder (Req 5.5)
                _cache[filePath] = null;
                return null;
            }
            finally
            {
                _semaphore.Release();
                _inProgress.TryRemove(filePath, out _);
            }
        }
    }
}
