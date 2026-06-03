using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Windows.Markup;

// Fully qualified: System.Windows.Application / System.Windows.ResourceDictionary
// to avoid ambiguity with System.Windows.Forms (UseWindowsForms=true in csproj).

namespace ZipEase.UI.Core
{
    /// <summary>
    /// 管理自訂 XAML ResourceDictionary 的載入、卸載與 hot-reload。
    /// 生命週期與 Application 一致，由 App.OnStartup() 初始化。
    /// </summary>
    public sealed class ThemeLoader : IDisposable
    {
        private static ThemeLoader? _instance;

        /// <summary>
        /// Gets the singleton instance. Throws if <see cref="Initialize"/> has not been called.
        /// </summary>
        public static ThemeLoader Instance => _instance
            ?? throw new InvalidOperationException("ThemeLoader not initialized");

        /// <summary>themes/ 資料夾路徑</summary>
        public static readonly string ThemesFolder = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "ZipEase", "themes");

        /// <summary>
        /// When set, overrides <see cref="ThemesFolder"/> for testing purposes.
        /// Production code leaves this null.
        /// </summary>
        internal static string? ThemesFolderOverride { get; set; }

        /// <summary>Returns <see cref="ThemesFolderOverride"/> if set, otherwise <see cref="ThemesFolder"/>.</summary>
        internal static string EffectiveThemesFolder => ThemesFolderOverride ?? ThemesFolder;

        /// <summary>已載入的自訂主題字典（key = 檔案完整路徑，case-insensitive）</summary>
        private readonly Dictionary<string, System.Windows.ResourceDictionary> _loaded =
            new(StringComparer.OrdinalIgnoreCase);

        /// <summary>The dispatcher captured during initialization, used for hot-reload callbacks.</summary>
        private readonly System.Windows.Threading.Dispatcher? _dispatcher;

        /// <summary>FileSystemWatcher monitoring the themes folder for .xaml changes.</summary>
        private FileSystemWatcher? _watcher;

        /// <summary>Debounce timer (300ms) to coalesce rapid file-system events.</summary>
        private System.Timers.Timer? _debounceTimer;

        /// <summary>Thread-safe queue buffering file-system events until the debounce timer fires.</summary>
        private readonly ConcurrentQueue<FileSystemEventArgs> _pendingEvents = new();

        /// <summary>Retry delay (ms) when a file is locked by another process.</summary>
        private const int IoRetryDelayMs = 500;

        /// <summary>Debounce interval (ms) for coalescing FileSystemWatcher events.</summary>
        private const int DebounceIntervalMs = 300;

        /// <summary>目前已載入的主題檔案數量</summary>
        public int LoadedCount => _loaded.Count;

        /// <summary>
        /// Creates a new ThemeLoader instance, capturing the current thread's Dispatcher.
        /// </summary>
        public ThemeLoader()
        {
            _dispatcher = System.Windows.Threading.Dispatcher.CurrentDispatcher;
        }

        /// <summary>
        /// 初始化 ThemeLoader：建立資料夾、掃描載入、啟動 FileSystemWatcher。
        /// </summary>
        public static ThemeLoader Initialize()
        {
            _instance?.Dispose();
            _instance = new ThemeLoader();
            _instance.EnsureThemesFolder();
            _instance.ScanAndLoad();
            _instance.StartWatcher();
            return _instance;
        }

        /// <summary>
        /// Scans the <see cref="ThemesFolder"/> for .xaml files and loads each one.
        /// Invalid files are skipped with a warning.
        /// </summary>
        internal void ScanAndLoad()
        {
            var files = ScanFolder(EffectiveThemesFolder);
            foreach (var file in files)
            {
                LoadTheme(file);
            }
        }

        /// <summary>
        /// Returns only .xaml files (case-insensitive) from the given folder.
        /// Returns an empty array if the folder does not exist.
        /// </summary>
        /// <param name="folderPath">The folder to scan.</param>
        /// <returns>Full paths of .xaml files found.</returns>
        public static string[] ScanFolder(string folderPath)
        {
            if (!Directory.Exists(folderPath))
                return Array.Empty<string>();

            return Directory.GetFiles(folderPath)
                .Where(f => Path.GetExtension(f).Equals(".xaml", StringComparison.OrdinalIgnoreCase))
                .ToArray();
        }

        /// <summary>
        /// Starts a <see cref="FileSystemWatcher"/> on the themes folder, filtering for *.xaml.
        /// File-system events are buffered in a <see cref="ConcurrentQueue{T}"/> and processed
        /// after a 300ms debounce window on the UI thread via <c>Dispatcher.Invoke</c>.
        /// </summary>
        internal void StartWatcher()
        {
            try
            {
                _watcher = new FileSystemWatcher(EffectiveThemesFolder, "*.xaml")
                {
                    NotifyFilter = NotifyFilters.FileName | NotifyFilters.LastWrite | NotifyFilters.CreationTime,
                    EnableRaisingEvents = true
                };

                _watcher.Created += OnFileChanged;
                _watcher.Changed += OnFileChanged;
                _watcher.Deleted += OnFileChanged;
                _watcher.Error += OnWatcherError;

                _debounceTimer = new System.Timers.Timer(DebounceIntervalMs)
                {
                    AutoReset = false
                };
                _debounceTimer.Elapsed += OnDebounceTimerElapsed;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[ThemeLoader] Warning: Could not start FileSystemWatcher: {ex.Message}");
            }
        }

        /// <summary>
        /// Enqueues a file-system event and resets the debounce timer.
        /// </summary>
        private void OnFileChanged(object sender, FileSystemEventArgs e)
        {
            _pendingEvents.Enqueue(e);
            _debounceTimer?.Stop();
            _debounceTimer?.Start();
        }

        /// <summary>
        /// Handles <see cref="FileSystemWatcher.Error"/> by performing a full rescan
        /// of the themes folder on the UI thread.
        /// </summary>
        private void OnWatcherError(object sender, ErrorEventArgs e)
        {
            Debug.WriteLine($"[ThemeLoader] FileSystemWatcher error: {e.GetException().Message}. Performing full rescan.");

            // Drain any stale events.
            while (_pendingEvents.TryDequeue(out _)) { }

            try
            {
                if (_dispatcher != null)
                {
                    _dispatcher.Invoke(() =>
                    {
                        UnloadAll();
                        ScanAndLoad();
                    });
                }
                else
                {
                    UnloadAll();
                    ScanAndLoad();
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[ThemeLoader] Warning: Full rescan after watcher error failed: {ex.Message}");
            }
        }

        /// <summary>
        /// Drains the pending event queue after the debounce window and processes
        /// each event on the UI thread. Deduplicates by keeping only the last event
        /// per file path.
        /// </summary>
        private void OnDebounceTimerElapsed(object? sender, System.Timers.ElapsedEventArgs e)
        {
            // Drain the queue and deduplicate: keep the last event per file path.
            var eventsByPath = new Dictionary<string, FileSystemEventArgs>(StringComparer.OrdinalIgnoreCase);
            while (_pendingEvents.TryDequeue(out var fsEvent))
            {
                eventsByPath[fsEvent.FullPath] = fsEvent;
            }

            if (eventsByPath.Count == 0)
                return;

            try
            {
                if (_dispatcher != null)
                {
                    _dispatcher.Invoke(() =>
                    {
                        foreach (var kvp in eventsByPath)
                        {
                            ProcessFileEvent(kvp.Value);
                        }
                    });
                }
                else
                {
                    // No dispatcher captured — process directly (shouldn't happen in production).
                    foreach (var kvp in eventsByPath)
                    {
                        ProcessFileEvent(kvp.Value);
                    }
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[ThemeLoader] Warning: Error processing file events on UI thread: {ex.Message}");
            }
        }

        /// <summary>
        /// Processes a single file-system event on the UI thread.
        /// Created/Changed → reload (with IOException retry).
        /// Deleted → unload.
        /// </summary>
        private void ProcessFileEvent(FileSystemEventArgs e)
        {
            switch (e.ChangeType)
            {
                case WatcherChangeTypes.Created:
                case WatcherChangeTypes.Changed:
                    if (!ReloadThemeWithRetry(e.FullPath))
                    {
                        Debug.WriteLine($"[ThemeLoader] Hot-reload: Theme file '{Path.GetFileName(e.FullPath)}' has invalid XAML. Previous version retained.");
                    }
                    break;

                case WatcherChangeTypes.Deleted:
                    UnloadTheme(e.FullPath);
                    break;
            }
        }

        /// <summary>
        /// Attempts to reload a theme file. If the file cannot be read (e.g., locked by
        /// another process), waits 500ms and retries once.
        /// </summary>
        /// <param name="filePath">Full path to the .xaml file.</param>
        /// <returns>true if reloaded successfully, false otherwise.</returns>
        private bool ReloadThemeWithRetry(string filePath)
        {
            // First attempt: check if the file is accessible before reloading.
            if (!IsFileAccessible(filePath))
            {
                Debug.WriteLine($"[ThemeLoader] File locked, retrying in {IoRetryDelayMs}ms: '{filePath}'");
                Thread.Sleep(IoRetryDelayMs);
            }

            if (ReloadTheme(filePath))
                return true;

            // ReloadTheme returned false — could be IOException (caught internally) or invalid XAML.
            // Try once more after a delay in case it was a transient lock.
            if (IsFileAccessible(filePath))
            {
                // File is accessible but XAML is invalid — no point retrying.
                return false;
            }

            Debug.WriteLine($"[ThemeLoader] File still locked, retrying in {IoRetryDelayMs}ms: '{filePath}'");
            Thread.Sleep(IoRetryDelayMs);
            return ReloadTheme(filePath);
        }

        /// <summary>
        /// Checks whether a file can be opened for reading.
        /// Returns false if the file is locked or inaccessible.
        /// </summary>
        private static bool IsFileAccessible(string filePath)
        {
            try
            {
                using var stream = File.Open(filePath, FileMode.Open, FileAccess.Read, FileShare.Read);
                return true;
            }
            catch (IOException)
            {
                return false;
            }
            catch
            {
                return false;
            }
        }

        /// <summary>
        /// 載入單一 .xaml 檔案為 ResourceDictionary 並加入 MergedDictionaries。
        /// </summary>
        /// <param name="filePath">Full path to the .xaml file.</param>
        /// <returns>true if loaded successfully, false if invalid XAML or error.</returns>
        public bool LoadTheme(string filePath)
        {
            try
            {
                System.Windows.ResourceDictionary rd;
                using (var stream = File.OpenRead(filePath))
                {
                    var obj = XamlReader.Load(stream);
                    if (obj is not System.Windows.ResourceDictionary dict)
                    {
                        Debug.WriteLine($"[ThemeLoader] Warning: '{filePath}' did not parse as a ResourceDictionary. Skipping.");
                        return false;
                    }
                    rd = dict;
                }

                // If already loaded, unload the old version first.
                if (_loaded.ContainsKey(filePath))
                {
                    UnloadTheme(filePath);
                }

                System.Windows.Application.Current.Resources.MergedDictionaries.Add(rd);
                _loaded[filePath] = rd;
                return true;
            }
            catch (XamlParseException ex)
            {
                Debug.WriteLine($"[ThemeLoader] Warning: Invalid XAML in '{filePath}': {ex.Message}");
                return false;
            }
            catch (IOException ex)
            {
                Debug.WriteLine($"[ThemeLoader] Warning: Could not read '{filePath}': {ex.Message}");
                return false;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[ThemeLoader] Warning: Unexpected error loading '{filePath}': {ex.Message}");
                return false;
            }
        }

        /// <summary>
        /// 從 MergedDictionaries 移除指定主題。
        /// No-op if the file was not previously loaded.
        /// </summary>
        /// <param name="filePath">Full path to the .xaml file.</param>
        public void UnloadTheme(string filePath)
        {
            if (_loaded.TryGetValue(filePath, out var rd))
            {
                try
                {
                    System.Windows.Application.Current.Resources.MergedDictionaries.Remove(rd);
                }
                catch (ArgumentOutOfRangeException)
                {
                    // The ResourceDictionary was already removed from MergedDictionaries
                    // (e.g., by another ThemeLoader instance in the same AppDomain).
                    Debug.WriteLine($"[ThemeLoader] Warning: ResourceDictionary already removed for '{filePath}'.");
                }
                _loaded.Remove(filePath);
            }
        }

        /// <summary>
        /// 重新載入指定主題（unload + load）。
        /// If the new version fails to load, the previous version is kept.
        /// </summary>
        /// <param name="filePath">Full path to the .xaml file.</param>
        /// <returns>true if reloaded successfully, false if the new version is invalid (previous kept).</returns>
        public bool ReloadTheme(string filePath)
        {
            // Save the previous version in case the new one fails.
            System.Windows.ResourceDictionary? previous = null;
            if (_loaded.TryGetValue(filePath, out var existing))
            {
                previous = existing;
            }

            // Unload the current version.
            UnloadTheme(filePath);

            // Attempt to load the new version.
            if (LoadTheme(filePath))
            {
                return true;
            }

            // Load failed — restore the previous version if we had one.
            if (previous is not null)
            {
                System.Windows.Application.Current.Resources.MergedDictionaries.Add(previous);
                _loaded[filePath] = previous;
                Debug.WriteLine($"[ThemeLoader] Reload failed for '{filePath}'. Previous version restored.");
            }

            return false;
        }

        /// <summary>
        /// 卸載所有自訂主題，恢復預設。
        /// </summary>
        public void UnloadAll()
        {
            foreach (var rd in _loaded.Values)
            {
                try
                {
                    System.Windows.Application.Current.Resources.MergedDictionaries.Remove(rd);
                }
                catch (ArgumentOutOfRangeException)
                {
                    // Already removed — safe to ignore.
                }
            }
            _loaded.Clear();
        }

        /// <summary>
        /// Ensures the themes folder exists. Creates it if it doesn't.
        /// </summary>
        private void EnsureThemesFolder()
        {
            try
            {
                var folder = EffectiveThemesFolder;
                if (!Directory.Exists(folder))
                {
                    Directory.CreateDirectory(folder);
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[ThemeLoader] Warning: Could not create themes folder: {ex.Message}");
            }
        }

        /// <summary>
        /// Disposes the ThemeLoader: stops the FileSystemWatcher, disposes the debounce timer,
        /// and unloads all custom themes.
        /// </summary>
        public void Dispose()
        {
            if (_watcher is not null)
            {
                _watcher.EnableRaisingEvents = false;
                _watcher.Created -= OnFileChanged;
                _watcher.Changed -= OnFileChanged;
                _watcher.Deleted -= OnFileChanged;
                _watcher.Error -= OnWatcherError;
                _watcher.Dispose();
                _watcher = null;
            }

            if (_debounceTimer is not null)
            {
                _debounceTimer.Stop();
                _debounceTimer.Elapsed -= OnDebounceTimerElapsed;
                _debounceTimer.Dispose();
                _debounceTimer = null;
            }

            // Drain any remaining events.
            while (_pendingEvents.TryDequeue(out _)) { }

            UnloadAll();

            if (_instance == this)
            {
                _instance = null;
            }
        }
    }
}
