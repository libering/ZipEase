using System;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using SkiaSharp;
using Svg.Skia;

// Fully qualified: System.Windows.Media.ImageSource
// to avoid ambiguity with System.Windows.Forms (UseWindowsForms=true in csproj).

namespace ZipEase.UI.Core
{
    /// <summary>
    /// 從外部 icons/ 資料夾解析 SVG 圖示，提供 ImageSource 給 UI。
    /// 使用 Svg.Skia 渲染 SVG，結果快取在記憶體中。
    /// </summary>
    public sealed class IconResolver : IDisposable
    {
        private static IconResolver? _instance;

        /// <summary>
        /// Gets the singleton instance. Throws if <see cref="Initialize"/> has not been called.
        /// </summary>
        public static IconResolver Instance => _instance
            ?? throw new InvalidOperationException("IconResolver not initialized");

        /// <summary>icons/ 資料夾路徑</summary>
        public static readonly string IconsFolder = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "ZipEase", "icons");

        /// <summary>
        /// When set, overrides <see cref="IconsFolder"/> for testing purposes.
        /// Production code leaves this null.
        /// </summary>
        internal static string? IconsFolderOverride { get; set; }

        /// <summary>
        /// When set, overrides the DPI scale returned by <see cref="GetDpiScale"/> for testing purposes.
        /// Production code leaves this null.
        /// </summary>
        internal static double? DpiScaleOverride { get; set; }

        /// <summary>Returns <see cref="IconsFolderOverride"/> if set, otherwise <see cref="IconsFolder"/>.</summary>
        internal static string EffectiveIconsFolder => IconsFolderOverride ?? IconsFolder;

        /// <summary>快取：副檔名（小寫，不含點）→ ImageSource (null = no icon or render failed)</summary>
        private readonly ConcurrentDictionary<string, ImageSource?> _cache = new();

        /// <summary>Maximum SVG file size in bytes (1 MB). Files larger than this are skipped.</summary>
        private const long MaxSvgFileSizeBytes = 1 * 1024 * 1024;

        /// <summary>FileSystemWatcher monitoring the icons folder for *.svg changes.</summary>
        private FileSystemWatcher? _watcher;

        /// <summary>
        /// 初始化 IconResolver：建立資料夾、啟動 FileSystemWatcher。
        /// </summary>
        public static IconResolver Initialize()
        {
            _instance?.Dispose();
            _instance = new IconResolver();
            _instance.EnsureIconsFolder();
            _instance.StartWatcher();
            return _instance;
        }

        /// <summary>
        /// Starts a <see cref="FileSystemWatcher"/> on the icons folder, filtering for *.svg.
        /// On Created/Changed/Deleted events, the cache entry for the affected extension is
        /// invalidated so the next <see cref="Resolve"/> call re-renders the SVG.
        /// No debounce is needed since we only invalidate cache (cheap operation).
        /// </summary>
        internal void StartWatcher()
        {
            try
            {
                _watcher = new FileSystemWatcher(EffectiveIconsFolder, "*.svg")
                {
                    NotifyFilter = NotifyFilters.FileName | NotifyFilters.LastWrite | NotifyFilters.CreationTime,
                    EnableRaisingEvents = true
                };

                _watcher.Created += OnSvgFileChanged;
                _watcher.Changed += OnSvgFileChanged;
                _watcher.Deleted += OnSvgFileChanged;
                _watcher.Error += OnWatcherError;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[IconResolver] Warning: Could not start FileSystemWatcher: {ex.Message}");
            }
        }

        /// <summary>
        /// Handles Created, Changed, and Deleted events for SVG files.
        /// Extracts the extension from the filename (e.g., "zip.svg" → "zip")
        /// and invalidates the corresponding cache entry.
        /// </summary>
        private void OnSvgFileChanged(object sender, FileSystemEventArgs e)
        {
            try
            {
                var fileName = Path.GetFileNameWithoutExtension(e.Name);
                if (string.IsNullOrWhiteSpace(fileName))
                    return;

                InvalidateCache(fileName);
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[IconResolver] Warning: Error handling file event for '{e.Name}': {ex.Message}");
            }
        }

        /// <summary>
        /// Handles <see cref="FileSystemWatcher.Error"/> by invalidating all cached icons
        /// so they are re-rendered on next access.
        /// </summary>
        private void OnWatcherError(object sender, ErrorEventArgs e)
        {
            Debug.WriteLine($"[IconResolver] FileSystemWatcher error: {e.GetException().Message}. Invalidating all cached icons.");
            InvalidateAll();
        }

        /// <summary>
        /// 解析指定副檔名的圖示。
        /// 優先使用 icons/ 中的 SVG，找不到或渲染失敗時回傳 null（呼叫端使用內建圖示）。
        /// </summary>
        /// <param name="extension">副檔名（不含點），如 "zip"、"pdf"</param>
        /// <param name="size">目標邏輯尺寸（像素），用於 DPI 縮放。Default 24.</param>
        /// <returns>ImageSource or null</returns>
        public ImageSource? Resolve(string extension, double size = 24)
        {
            if (string.IsNullOrWhiteSpace(extension))
                return null;

            var key = extension.ToLowerInvariant();

            // Return cached result (including cached null for failed renders).
            if (_cache.TryGetValue(key, out var cached))
                return cached;

            // Render and cache.
            try
            {
                var result = RenderSvg(key, size);
                _cache.TryAdd(key, result);
                return result;
            }
            catch (IOException)
            {
                // File locked — don't cache, allow retry on next Resolve call.
                return null;
            }
        }

        /// <summary>清除指定副檔名的快取，下次 Resolve 時重新渲染</summary>
        public void InvalidateCache(string extension)
        {
            if (string.IsNullOrWhiteSpace(extension))
                return;

            var key = extension.ToLowerInvariant();
            _cache.TryRemove(key, out _);
        }

        /// <summary>清除所有快取</summary>
        public void InvalidateAll()
        {
            _cache.Clear();
        }

        /// <summary>
        /// Renders an SVG file from the icons folder for the given extension key.
        /// Returns null if the file doesn't exist, is too large, or rendering fails.
        /// </summary>
        /// <param name="extensionKey">Lowercase extension without dot, e.g. "zip".</param>
        /// <param name="size">Logical size in pixels.</param>
        /// <returns>A frozen ImageSource, or null.</returns>
        private ImageSource? RenderSvg(string extensionKey, double size)
        {
            var svgPath = Path.Combine(EffectiveIconsFolder, $"{extensionKey}.svg");

            if (!File.Exists(svgPath))
                return null;

            try
            {
                // Skip files larger than 1 MB.
                var fileInfo = new FileInfo(svgPath);
                if (fileInfo.Length > MaxSvgFileSizeBytes)
                {
                    Debug.WriteLine($"[IconResolver] Skipping '{svgPath}': file size {fileInfo.Length} exceeds 1 MB limit.");
                    return null;
                }

                // Load SVG using Svg.Skia.
                using var svg = new SKSvg();
                svg.Load(svgPath);

                if (svg.Picture is null)
                {
                    Debug.WriteLine($"[IconResolver] Warning: SVG '{svgPath}' produced no picture. Skipping.");
                    return null;
                }

                // Calculate DPI-aware pixel dimensions.
                double dpiScale = GetDpiScale();
                int pixelSize = (int)Math.Ceiling(size * dpiScale);
                if (pixelSize <= 0)
                    pixelSize = (int)Math.Ceiling(size);

                // Create bitmap and render.
                var imageInfo = new SKImageInfo(pixelSize, pixelSize, SKColorType.Bgra8888, SKAlphaType.Premul);
                using var surface = SKSurface.Create(imageInfo);
                if (surface is null)
                {
                    Debug.WriteLine($"[IconResolver] Warning: Could not create SKSurface for '{svgPath}'.");
                    return null;
                }

                var canvas = surface.Canvas;
                canvas.Clear(SKColors.Transparent);

                // Scale the SVG picture to fit the target pixel size.
                var pictureBounds = svg.Picture.CullRect;
                if (pictureBounds.Width > 0 && pictureBounds.Height > 0)
                {
                    float scaleX = pixelSize / pictureBounds.Width;
                    float scaleY = pixelSize / pictureBounds.Height;
                    float scale = Math.Min(scaleX, scaleY);

                    float offsetX = (pixelSize - pictureBounds.Width * scale) / 2f;
                    float offsetY = (pixelSize - pictureBounds.Height * scale) / 2f;

                    canvas.Translate(offsetX, offsetY);
                    canvas.Scale(scale);
                }

                canvas.DrawPicture(svg.Picture);
                canvas.Flush();

                // Snapshot to get pixel data.
                using var snapshot = surface.Snapshot();
                if (snapshot is null)
                    return null;

                using var pixelData = snapshot.Encode(SKEncodedImageFormat.Png, 100);
                if (pixelData is null)
                    return null;

                // Convert PNG bytes to WPF BitmapSource using BitmapFrame (thread-safe).
                var bytes = pixelData.ToArray();
                var stream = new MemoryStream(bytes);
                var decoder = BitmapDecoder.Create(stream, BitmapCreateOptions.PreservePixelFormat, BitmapCacheOption.OnLoad);
                if (decoder.Frames.Count == 0)
                    return null;

                var frame = decoder.Frames[0];
                if (frame.CanFreeze)
                    frame.Freeze();

                return frame;
            }
            catch (IOException ex)
            {
                Debug.WriteLine($"[IconResolver] Warning: Could not read '{svgPath}': {ex.Message}");
                throw;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[IconResolver] Warning: Failed to render '{svgPath}': {ex.Message}");
                return null;
            }
        }

        /// <summary>
        /// Gets the current DPI scale factor from the main window.
        /// Falls back to 1.0 if the main window is not available.
        /// </summary>
        internal static double GetDpiScale()
        {
            // Allow tests to override the DPI scale without needing a WPF Application.
            if (DpiScaleOverride.HasValue)
                return DpiScaleOverride.Value;

            try
            {
                var mainWindow = System.Windows.Application.Current?.MainWindow;
                if (mainWindow is not null)
                {
                    var dpiInfo = VisualTreeHelper.GetDpi(mainWindow);
                    return dpiInfo.DpiScaleX;
                }
            }
            catch
            {
                // Fallback — Application.Current or MainWindow may not be available
                // (e.g., during startup or in tests).
            }

            return 1.0;
        }

        /// <summary>
        /// Ensures the icons folder exists. Creates it if it doesn't.
        /// </summary>
        private void EnsureIconsFolder()
        {
            try
            {
                var folder = EffectiveIconsFolder;
                if (!Directory.Exists(folder))
                {
                    Directory.CreateDirectory(folder);
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[IconResolver] Warning: Could not create icons folder: {ex.Message}");
            }
        }

        /// <summary>
        /// Disposes the IconResolver: stops the FileSystemWatcher and clears the cache.
        /// </summary>
        public void Dispose()
        {
            if (_watcher is not null)
            {
                _watcher.EnableRaisingEvents = false;
                _watcher.Created -= OnSvgFileChanged;
                _watcher.Changed -= OnSvgFileChanged;
                _watcher.Deleted -= OnSvgFileChanged;
                _watcher.Error -= OnWatcherError;
                _watcher.Dispose();
                _watcher = null;
            }

            _cache.Clear();

            if (_instance == this)
            {
                _instance = null;
            }
        }
    }
}
