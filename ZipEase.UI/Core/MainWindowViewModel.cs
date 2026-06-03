using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
#pragma warning disable CS4014 // fire-and-forget tasks are intentional
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using WpfApplication = System.Windows.Application;
using OpenFileDialog = Microsoft.Win32.OpenFileDialog;

namespace ZipEase.UI.Core
{
    public partial class MainWindowViewModel : ObservableObject
    {
        private readonly ArchivePreviewService _previewService;
        private readonly RepairService _repairService = new();

        [ObservableProperty] private UIState _currentState = UIState.Idle;
        [ObservableProperty] private string _loadedArchivePath = string.Empty;
        [ObservableProperty] private int _extractionProgress;
        [ObservableProperty] private string _currentExtractionFile = string.Empty;
        [ObservableProperty] private string _statusMessage = string.Empty;
        [ObservableProperty] private bool _isStatusVisible;
        [ObservableProperty] private bool _isStatusError;
        [ObservableProperty] private string _currentPath = string.Empty;
        [ObservableProperty] private bool _isTrashButtonEnabled;
        [ObservableProperty] private bool _isRepairAvailable;
        [ObservableProperty] private string _repairedFilePath = string.Empty;
        [ObservableProperty] private bool _isRepairSuccessVisible;

        // Batch extraction state
        [ObservableProperty] private int _batchArchiveIndex;
        [ObservableProperty] private int _batchArchiveCount;
        [ObservableProperty] private string _batchCurrentArchiveName = string.Empty;

        // Batch cancellation
        private CancellationTokenSource? _batchCts;

        public bool ForceExtract
        {
            get => AppSettings.Instance.ForceExtract;
            set
            {
                AppSettings.Instance.ForceExtract = value;
                AppSettings.Instance.Save();
                OnPropertyChanged();
            }
        }

        // Subscribe to AppSettings changes so the extract toolbar CheckBox stays in sync
        // when the user toggles ForceExtract in the Settings page.
        partial void OnIsTrashButtonEnabledChanged(bool value) { } // keep partial chain alive

        // Navigation state
        private readonly Stack<string> _navigationStack = new();
        private List<ArchiveEntry> _allEntries = new();

        // Native entries pointer — kept alive for Rust-backed search
        private IntPtr _nativeEntriesPtr = IntPtr.Zero;
        private int _nativeEntryCount;

        // Password state (never exposed as public property)
        private string? _pendingPassword;
        private int _passwordAttempts;

        public ObservableCollection<ArchiveEntryViewModel> ArchiveEntries { get; } = new();

        // Search / filter
        [ObservableProperty] private string _searchText = string.Empty;
        private CancellationTokenSource? _searchCts;
        private DispatcherTimer? _debounceTimer;
        private bool _isDeepSearchActive;

        partial void OnSearchTextChanged(string value)
        {
            _debounceTimer?.Stop();

            // If cleared, restore immediately without debounce
            if (string.IsNullOrEmpty(value))
            {
                _searchCts?.Cancel();
                _isDeepSearchActive = false;
                RefreshEntriesForCurrentPath();
                return;
            }

            _debounceTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(300) };
            _debounceTimer.Tick += (s, e) =>
            {
                _debounceTimer!.Stop();
                _ = ExecuteSearchAsync(value);
            };
            _debounceTimer.Start();
        }

        private async Task ExecuteSearchAsync(string pattern)
        {
            // Cancel previous search
            _searchCts?.Cancel();
            _searchCts = new CancellationTokenSource();
            var token = _searchCts.Token;

            if (string.IsNullOrEmpty(pattern))
            {
                _isDeepSearchActive = false;
                RefreshEntriesForCurrentPath();
                return;
            }

            try
            {
                int[] indices;

                if (_nativeEntriesPtr != IntPtr.Zero && _nativeEntryCount > 0)
                {
                    // Use Rust-backed search via FFI
                    indices = await Task.Run(() =>
                        SearchService.Search(pattern, _nativeEntriesPtr, _nativeEntryCount, token),
                        token);
                }
                else
                {
                    // Fallback: simple managed search (when native pointer not available)
                    indices = await Task.Run(() =>
                    {
                        var results = new List<int>();
                        for (int i = 0; i < _allEntries.Count; i++)
                        {
                            if (token.IsCancellationRequested) break;
                            if (_allEntries[i].FileName.Contains(pattern, StringComparison.OrdinalIgnoreCase))
                                results.Add(i);
                        }
                        return results.ToArray();
                    }, token);
                }

                if (token.IsCancellationRequested) return;

                // Update filtered view on UI thread (deep search: show all levels with full paths)
                _isDeepSearchActive = true;
                ArchiveEntries.Clear();
                foreach (var idx in indices)
                {
                    if (idx >= 0 && idx < _allEntries.Count)
                    {
                        ArchiveEntries.Add(new ArchiveEntryViewModel(_allEntries[idx]));
                    }
                }
                OnPropertyChanged(nameof(FileCount));
                OnPropertyChanged(nameof(FolderCount));
                OnPropertyChanged(nameof(ArchiveSummary));
            }
            catch (OperationCanceledException) { /* Normal cancellation */ }
            catch (Exception ex)
            {
#if DEBUG_CONSOLE
                DebugConsole.Log($"[Search] Error: {ex.Message}");
#endif
                // On error, keep last valid results visible
            }
        }

        // Computed properties
        public bool IsIdleVisible => CurrentState == UIState.Idle || CurrentState == UIState.DragOver;
        public bool IsPreviewVisible => CurrentState == UIState.Previewing || CurrentState == UIState.Extracting;
        public bool IsProgressVisible => CurrentState == UIState.Extracting || CurrentState == UIState.BatchExtracting;
        public bool IsExtractButtonEnabled => CurrentState == UIState.Previewing;
        public bool IsDragOverActive => CurrentState == UIState.DragOver;
        public bool IsBackButtonVisible => !string.IsNullOrEmpty(CurrentPath);
        public bool IsCancelBatchVisible => CurrentState == UIState.BatchExtracting;
        public int FileCount => ArchiveEntries.Count(e => !e.IsDirectory);
        public int FolderCount => ArchiveEntries.Count(e => e.IsDirectory);
        public string ArchiveSummary => FolderCount > 0
            ? $"{FolderCount} 個資料夾，{FileCount} 個檔案"
            : $"{FileCount} 個檔案";

        public MainWindowViewModel(ArchivePreviewService previewService)
        {
            _previewService = previewService;
            // Keep ForceExtract binding in sync when changed from Settings page
            AppSettings.ForceExtractChanged += () =>
                WpfApplication.Current.Dispatcher.BeginInvoke(() => OnPropertyChanged(nameof(ForceExtract)));
        }

        // State transitions
        public void TransitionToIdle()
        {
            CurrentState = UIState.Idle;
            LoadedArchivePath = string.Empty;
            ExtractionProgress = 0;
            CurrentExtractionFile = string.Empty;
            ArchiveEntries.Clear();
            _allEntries.Clear();
            _navigationStack.Clear();
            CurrentPath = string.Empty;
            _pendingPassword = null;
            _passwordAttempts = 0;
            IsRepairAvailable = false;
            IsRepairSuccessVisible = false;
            RepairedFilePath = string.Empty;

            // Clean up search state
            _searchCts?.Cancel();
            _searchCts = null;
            _debounceTimer?.Stop();
            _debounceTimer = null;
            _isDeepSearchActive = false;
            SearchText = string.Empty;

            // Free native entries pointer
            FreeNativeEntries();

            NotifyVisibilityChanged();
            IsTrashButtonEnabled = false;
            TrashSourceCommand.NotifyCanExecuteChanged();
        }

        public void TransitionToDragOver()
        {
            CurrentState = UIState.DragOver;
            NotifyVisibilityChanged();
        }

        public void TransitionToPreviewing(string archivePath)
        {
            CurrentState = UIState.Previewing;
            LoadedArchivePath = archivePath;
            NotifyVisibilityChanged();
        }

        public void TransitionToExtracting()
        {
            CurrentState = UIState.Extracting;
            ExtractionProgress = 0;
            CurrentExtractionFile = string.Empty;
            NotifyVisibilityChanged();
        }

        public void TransitionToBatchExtracting()
        {
            CurrentState = UIState.BatchExtracting;
            BatchArchiveIndex = 0;
            BatchArchiveCount = 0;
            BatchCurrentArchiveName = string.Empty;
            ExtractionProgress = 0;
            CurrentExtractionFile = string.Empty;
            NotifyVisibilityChanged();
        }

        public void TransitionFromBatchExtracting()
        {
            BatchArchiveIndex = 0;
            BatchArchiveCount = 0;
            BatchCurrentArchiveName = string.Empty;
            ExtractionProgress = 0;
            CurrentExtractionFile = string.Empty;
            CurrentState = UIState.Idle;
            NotifyVisibilityChanged();
        }

        public void TransitionBackToPreviewing()
        {
            CurrentState = UIState.Previewing;
            ExtractionProgress = 0;
            CurrentExtractionFile = string.Empty;
            NotifyVisibilityChanged();
            IsTrashButtonEnabled = true;
            TrashSourceCommand.NotifyCanExecuteChanged();
        }

        public void ShowError(string message)
        {
            StatusMessage = message;
            IsStatusError = true;
            IsStatusVisible = true;
#if DEBUG_CONSOLE
            ZipEase.UI.Core.DebugConsole.Log($"[ShowError] {message}");
#endif
        }

        public void ShowSuccess(string message)
        {
            StatusMessage = message;
            IsStatusError = false;
            IsStatusVisible = true;
#if DEBUG_CONSOLE
            ZipEase.UI.Core.DebugConsole.Log($"[ShowSuccess] {message}");
#endif
        }

        private void NotifyVisibilityChanged()
        {
            OnPropertyChanged(nameof(IsIdleVisible));
            OnPropertyChanged(nameof(IsPreviewVisible));
            OnPropertyChanged(nameof(IsProgressVisible));
            OnPropertyChanged(nameof(IsExtractButtonEnabled));
            OnPropertyChanged(nameof(IsDragOverActive));
            OnPropertyChanged(nameof(IsBackButtonVisible));
            OnPropertyChanged(nameof(IsCancelBatchVisible));
            OnPropertyChanged(nameof(FileCount));
            OnPropertyChanged(nameof(FolderCount));
            OnPropertyChanged(nameof(ArchiveSummary));
            OnPropertyChanged(nameof(IsTrashButtonEnabled));
        }

        // Navigation helpers
        private void FreeNativeEntries()
        {
            if (_nativeEntriesPtr != IntPtr.Zero)
            {
                NativeMethods.FreeArchiveEntries(_nativeEntriesPtr, _nativeEntryCount);
                _nativeEntriesPtr = IntPtr.Zero;
                _nativeEntryCount = 0;
            }
        }

        private void RefreshEntriesForCurrentPath()
        {
            ArchiveEntries.Clear();

            // If deep search is active, don't filter by path — ExecuteSearchAsync handles display
            if (_isDeepSearchActive) return;

            foreach (var entry in _allEntries)
            {
                if (GetImmediateParent(entry.FileName) != CurrentPath) continue;
                ArchiveEntries.Add(new ArchiveEntryViewModel(entry));
            }
            OnPropertyChanged(nameof(FileCount));
            OnPropertyChanged(nameof(FolderCount));
            OnPropertyChanged(nameof(ArchiveSummary));
            OnPropertyChanged(nameof(IsBackButtonVisible));
        }

        public static string GetImmediateParent(string entryName)
        {
            var trimmed = entryName.TrimEnd('/');
            var lastSlash = trimmed.LastIndexOf('/');
            return lastSlash < 0 ? string.Empty : trimmed[..(lastSlash + 1)];
        }

        // Commands
        [RelayCommand(CanExecute = nameof(CanNavigateBack))]
        private void NavigateBack()
        {
            if (_navigationStack.Count == 0) return;
            CurrentPath = _navigationStack.Pop();
            RefreshEntriesForCurrentPath();
            NavigateBackCommand.NotifyCanExecuteChanged();
        }

        private bool CanNavigateBack() => _navigationStack.Count > 0;

        [RelayCommand]
        private void NavigateInto(ArchiveEntryViewModel? entry)
        {
            if (entry == null || !entry.IsDirectory) return;
            _navigationStack.Push(CurrentPath);
            CurrentPath = entry.FileName.TrimEnd('/') + "/";
            RefreshEntriesForCurrentPath();
            NavigateBackCommand.NotifyCanExecuteChanged();
        }

        [RelayCommand]
        private void BrowseFile()
        {
            var dialog = new OpenFileDialog
            {
                Title = "Select Archive File",
                Filter = "Archive Files|*.zip;*.rar;*.7z;*.tar;*.gz;*.cab;*.iso;*.apk|All Files|*.*",
                Multiselect = true
            };

            if (dialog.ShowDialog() != true) return;

            if (dialog.FileNames.Length == 1)
                LoadArchive(dialog.FileNames[0]);
            else
                BatchExtractCommand.Execute(dialog.FileNames);
        }

        [RelayCommand]
        private void CancelBatch()
        {
            _batchCts?.Cancel();
        }

        [RelayCommand]
        private async Task Extract()
        {
            if (string.IsNullOrEmpty(LoadedArchivePath)) return;

            var dialog = new System.Windows.Forms.FolderBrowserDialog
            {
                Description = "選擇輸出資料夾",
                UseDescriptionForTitle = true,
                SelectedPath = AppSettings.Instance.LastOutputDir
            };

            if (dialog.ShowDialog() != System.Windows.Forms.DialogResult.OK) return;

            string baseOutputDir = dialog.SelectedPath;

            // Ask user: extract directly or wrap in a folder
            string wrapFolderName = System.IO.Path.GetFileNameWithoutExtension(LoadedArchivePath);
            var choice = System.Windows.MessageBox.Show(
                $"要如何解壓縮？\n\n「是」— 直接解壓到所選資料夾\n「否」— 建立「{wrapFolderName}」資料夾並解壓到其中",
                "解壓縮方式",
                System.Windows.MessageBoxButton.YesNoCancel,
                System.Windows.MessageBoxImage.Question,
                System.Windows.MessageBoxResult.No);

            if (choice == System.Windows.MessageBoxResult.Cancel) return;

            string outputDir = choice == System.Windows.MessageBoxResult.Yes
                ? baseOutputDir
                : System.IO.Path.Combine(baseOutputDir, wrapFolderName);

            if (choice == System.Windows.MessageBoxResult.No)
                System.IO.Directory.CreateDirectory(outputDir);

            AppSettings.Instance.LastOutputDir = baseOutputDir;
            AppSettings.Instance.Save();
            TransitionToExtracting();
            IsStatusVisible = false;

            try
            {
                int fileCount = ForceExtract
                    ? await ExtractionManager.ExtractForceAsync(
                        LoadedArchivePath,
                        outputDir,
                        (percentage, currentFile) =>
                        {
                            WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                            {
                                ExtractionProgress = percentage;
                                CurrentExtractionFile = currentFile;
                            });
                        })
                    : await ExtractionManager.ExtractAsync(
                        LoadedArchivePath,
                        outputDir,
                        _pendingPassword,
                        (percentage, currentFile) =>
                        {
                            WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                            {
                                ExtractionProgress = percentage;
                                CurrentExtractionFile = currentFile;
                            });
                        });

                _pendingPassword = null;
                string archiveName = System.IO.Path.GetFileName(LoadedArchivePath);
                TransitionBackToPreviewing();
                ShowSuccess(LocalizationManager.F("Status_ExtractSuccess", fileCount, outputDir));

                if (AppSettings.Instance.ToastNotifications)
                    _ = ExtractionManager.NotifySuccessAsync(archiveName, outputDir, fileCount);

                if (AppSettings.Instance.AutoTrashAfterExtract)
                {
                    int trashResult = await ExtractionManager.TrashFileAsync(LoadedArchivePath);
                    if (trashResult != 0)
                        ShowError(LocalizationManager.Get("Status_AutoTrashFailed"));
                }
            }
            catch (ExtractionException ex) when (ex.ErrorCode == unchecked((int)0x2004))
            {
                TransitionBackToPreviewing();
                await HandlePasswordRequiredForExtraction();
            }
            catch (ExtractionException ex)
            {
                TransitionBackToPreviewing();
                ShowError(LocalizationManager.F("Status_ExtractFailed", ex.Message));
                IsRepairAvailable = true;
                _ = ExtractionManager.NotifyFailureAsync(
                        System.IO.Path.GetFileName(LoadedArchivePath),
                        ex.Message);

                bool isAccessDenied = ex.Message.IndexOf("access", StringComparison.OrdinalIgnoreCase) >= 0
                                   || ex.Message.IndexOf("denied", StringComparison.OrdinalIgnoreCase) >= 0
                                   || ex.Message.IndexOf("sharing", StringComparison.OrdinalIgnoreCase) >= 0;

                if (isAccessDenied && LoadedArchivePath is not null && AppSettings.Instance.LockDetection)
                {
                    _ = Task.Run(async () =>
                    {
                        System.IntPtr ptr = System.IntPtr.Zero;
                        try
                        {
                            ptr = await ExtractionManager.WhoLocksAsync(LoadedArchivePath);
                            if (ptr != System.IntPtr.Zero)
                            {
                                string names = System.Runtime.InteropServices.Marshal.PtrToStringUni(ptr)!;
                                WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                                    ShowError(LocalizationManager.F("Status_FileLocked", names)));
                            }
                        }
                        finally
                        {
                            NativeMethods.FreeString(ptr);
                        }
                    });
                }
            }
            catch (Exception ex)
            {
                TransitionBackToPreviewing();
                ShowError(LocalizationManager.F("Status_UnexpectedError", ex.Message));
                _ = ExtractionManager.NotifyFailureAsync(
                        System.IO.Path.GetFileName(LoadedArchivePath),
                        ex.Message);
            }
        }

        private async Task HandlePasswordRequiredForExtraction()
        {
            _passwordAttempts++;
            if (_passwordAttempts > 3)
            {
                _pendingPassword = null;
                _passwordAttempts = 0;
                ShowError(LocalizationManager.Get("Status_WrongPassword"));
                return;
            }

            string? errorMsg = _passwordAttempts > 1 ? LocalizationManager.Get("PasswordDialog_WrongPassword") : null;
            var pwDialog = new PasswordDialog(errorMsg);
            bool confirmed = pwDialog.ShowDialog() == true;

            if (!confirmed)
            {
                _pendingPassword = null;
                _passwordAttempts = 0;
                return;
            }

            _pendingPassword = pwDialog.Password;
            await Extract();
        }

        [RelayCommand]
        private void Reset()
        {
            TransitionToIdle();
            IsStatusVisible = false;
        }

        [RelayCommand]
        private void Drop(string filePath)
        {
            if (!_previewService.IsSupportedArchive(filePath))
            {
                ShowError(LocalizationManager.Get("Status_UnsupportedFormat"));
                return;
            }
            LoadArchive(filePath);
        }

        /// <summary>
        /// Batch-extract multiple archives to the same output directory.
        /// Each archive is extracted sequentially; overall progress is shown.
        /// </summary>
        [RelayCommand]
        private async Task BatchExtract(string[] archivePaths)
        {
            var supported = archivePaths
                .Where(p => _previewService.IsSupportedArchive(p))
                .ToList();

            if (supported.Count == 0)
            {
                ShowError(LocalizationManager.Get("Status_NoSupportedArchives"));
                return;
            }

            // If only one file, fall through to normal single-file preview
            if (supported.Count == 1)
            {
                LoadArchive(supported[0]);
                return;
            }

            var dialog = new System.Windows.Forms.FolderBrowserDialog
            {
                Description = "選擇輸出資料夾",
                UseDescriptionForTitle = true,
                SelectedPath = AppSettings.Instance.LastOutputDir
            };
            if (dialog.ShowDialog() != System.Windows.Forms.DialogResult.OK) return;

            string outputDir = dialog.SelectedPath;
            AppSettings.Instance.LastOutputDir = outputDir;
            AppSettings.Instance.Save();

            TransitionToBatchExtracting();
            BatchArchiveCount = supported.Count;
            IsStatusVisible = false;

            // Create a new CancellationTokenSource for this batch operation
            _batchCts?.Dispose();
            _batchCts = new CancellationTokenSource();
            var token = _batchCts.Token;

            int totalExtracted = 0;
            int failed = 0;
            bool wasCancelled = false;

            for (int i = 0; i < supported.Count; i++)
            {
                // Check cancellation before starting next archive
                if (token.IsCancellationRequested)
                {
                    wasCancelled = true;
                    break;
                }

                var archivePath = supported[i];
                var archiveName = System.IO.Path.GetFileName(archivePath);

                // Update batch progress UI via Dispatcher.BeginInvoke (Rust callbacks run on background threads)
                WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                {
                    BatchArchiveIndex = i + 1;
                    BatchArchiveCount = supported.Count;
                    BatchCurrentArchiveName = archiveName;
                    CurrentExtractionFile = $"解壓中 {i + 1}/{supported.Count}: {archiveName}";
                    ExtractionProgress = (int)((double)i / supported.Count * 100);
                });

                try
                {
                    int count = await ExtractionManager.ExtractAsync(
                        archivePath, outputDir,
                        progressCallback: (pct, file) =>
                        {
                            WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                            {
                                // Per-archive progress blended with overall batch progress
                                int overall = (int)(((double)i + pct / 100.0) / supported.Count * 100);
                                ExtractionProgress = overall;
                                CurrentExtractionFile = $"解壓中 {i + 1}/{supported.Count}: {archiveName}";
                            });
                        });
                    totalExtracted += count;

                    if (AppSettings.Instance.AutoTrashAfterExtract)
                        await ExtractionManager.TrashFileAsync(archivePath);
                }
                catch (Exception ex)
                {
                    failed++;
                    // Continue with remaining archives — error isolation
                    WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                        ShowError($"{archiveName}: {ex.Message}"));
                }
            }

            // Clean up CancellationTokenSource
            _batchCts?.Dispose();
            _batchCts = null;

            TransitionFromBatchExtracting();

            string summary;
            if (wasCancelled)
                summary = $"批次解壓縮已取消：已完成 {totalExtracted} 個檔案（共 {supported.Count} 個壓縮檔）";
            else if (failed == 0)
                summary = $"批次解壓縮完成：{supported.Count} 個壓縮檔，共 {totalExtracted} 個檔案";
            else
                summary = $"批次解壓縮完成（{failed} 個失敗）：{totalExtracted} 個檔案";
            ShowSuccess(summary);

            // Requirement 8.4: Respect Notifications setting
            if (AppSettings.Instance.ToastNotifications)
            {
                if (failed == 0 && !wasCancelled)
                {
                    // Requirement 8.1: All success — "已解壓 N 個檔案到 [目錄名稱]"
                    // Requirement 8.3: Toast includes "開啟資料夾" button (built into NotifySuccessAsync)
                    string dirName = System.IO.Path.GetFileName(outputDir.TrimEnd('\\', '/'));
                    _ = ExtractionManager.NotifySuccessAsync(
                        $"已解壓 {supported.Count} 個壓縮檔到 {dirName}",
                        outputDir,
                        totalExtracted);
                }
                else if (failed > 0)
                {
                    // Requirement 8.2: Partial success — "N 個成功、M 個失敗"
                    // Requirement 8.3: Toast includes "開啟資料夾" button (built into NotifySuccessAsync)
                    int succeeded = supported.Count - failed;
                    _ = ExtractionManager.NotifySuccessAsync(
                        $"{succeeded} 個成功、{failed} 個失敗",
                        outputDir,
                        totalExtracted);
                }
            }
        }

        /// <summary>
        /// Extracts a single entry to a temp folder and initiates a WPF drag-drop operation.
        /// The temp file is placed in Path.GetTempPath() and cleaned up after the drag completes.
        /// </summary>
        [RelayCommand]
        private async Task ExtractSingleEntry(ArchiveEntryViewModel? entry)
        {
            if (entry == null || entry.IsDirectory) return;
            if (string.IsNullOrEmpty(LoadedArchivePath)) return;

            string tempDir = Path.Combine(Path.GetTempPath(), "ZipEase_drag_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(tempDir);

            try
            {
                bool isZipBased = ZipBasedExtensions.Contains(
                    Path.GetExtension(LoadedArchivePath));

                string extractedName;
                if (isZipBased)
                {
                    int index = _allEntries.FindIndex(e => e.FileName == entry.FileName);
                    if (index < 0) return;
                    extractedName = await ExtractionManager.ExtractEntryAsync(
                        LoadedArchivePath, (uint)index, tempDir);
                }
                else
                {
                    // 傳遞完整路徑字串，Rust 側用名稱匹配，不依賴 index
                    extractedName = await ExtractionManager.ExtractEntryByNameAsync(
                        LoadedArchivePath, entry.FileName, tempDir);
                }

                string extractedPath = Path.Combine(tempDir, extractedName);
                if (!File.Exists(extractedPath)) return;

                // Build a DataObject with the file path for shell drag-drop
                var dataObject = new System.Windows.DataObject(System.Windows.DataFormats.FileDrop, new[] { extractedPath });

                // DoDragDrop must run on the UI thread
                WpfApplication.Current.Dispatcher.Invoke(() =>
                {
                    System.Windows.DragDrop.DoDragDrop(
                        WpfApplication.Current.MainWindow,
                        dataObject,
                        System.Windows.DragDropEffects.Copy | System.Windows.DragDropEffects.Move);
                });
            }
            catch (ExtractionException ex)
            {
                ShowError($"無法提取檔案: {ex.Message}");
            }
            catch (Exception ex)
            {
                ShowError($"拖出失敗: {ex.Message}");
            }
            finally
            {
                // Best-effort cleanup of temp dir after drag completes
                try { Directory.Delete(tempDir, recursive: true); } catch { }
            }
        }

        /// <summary>
        /// Extracts a single entry to a temp folder and opens it with the system default application.
        private static readonly HashSet<string> ExecutableExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            ".exe", ".bat", ".cmd", ".ps1", ".vbs", ".js", ".msi", ".com", ".scr"
        };

        private static readonly HashSet<string> ZipBasedExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            ".zip", ".apk", ".ipa", ".jar", ".war", ".ear"
        };

        private static readonly HashSet<string> ImageExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            ".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".avif", ".tiff", ".tif"
        };

        // Tracks which archives have already had background pre-extraction triggered
        private readonly System.Collections.Concurrent.ConcurrentDictionary<string, bool> _preExtractStarted = new();

        /// <summary>
        /// Checks if a file extension has a registered shell association in the Windows registry.
        /// Returns false for extensions with no associated program (e.g. .dex, .arsc).
        /// </summary>
        private static bool HasShellAssociation(string extension)
        {
            if (string.IsNullOrEmpty(extension)) return false;
            try
            {
                // Check HKCR\<ext> for a ProgID, then HKCR\<ProgID>\shell\open\command
                using var extKey = Microsoft.Win32.Registry.ClassesRoot.OpenSubKey(extension);
                if (extKey == null) return false;
                var progId = extKey.GetValue(null) as string;
                if (string.IsNullOrEmpty(progId)) return false;
                using var openCmd = Microsoft.Win32.Registry.ClassesRoot.OpenSubKey(
                    $@"{progId}\shell\open\command");
                return openCmd != null;
            }
            catch { return false; }
        }

        private static readonly HashSet<string> TextLikeExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            ".txt", ".log", ".md", ".markdown", ".csv", ".tsv",
            ".ini", ".cfg", ".conf", ".config",
            ".xml", ".json", ".yaml", ".yml", ".toml",
            ".html", ".htm", ".css", ".js", ".ts",
            ".py", ".rb", ".sh", ".bat", ".cmd", ".ps1",
            ".c", ".cpp", ".h", ".cs", ".java", ".rs", ".go",
            ".properties", ".gradle", ".pom",
        };

        /// <summary>
        /// Tries to open a file using a fallback editor chain when the shell has no association.
        /// Only applies to text-like formats. Returns true if an editor was successfully launched.
        /// Editor chain: VS Code → Notepad++ → WordPad → notepad.exe
        /// </summary>
        private static bool TryOpenWithFallbackEditor(string filePath)
        {
            var ext = System.IO.Path.GetExtension(filePath);
            if (!TextLikeExtensions.Contains(ext)) return false;

            // Ordered fallback chain — try each editor in turn
            var editors = new[]
            {
                // VS Code (most common dev editor)
                ("code", $"\"{filePath}\""),
                // Notepad++
                (FindInProgramFiles("Notepad++", "notepad++.exe"), $"\"{filePath}\""),
                // WordPad (built-in, handles more formats than Notepad)
                (System.IO.Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.System),
                    "write.exe"), $"\"{filePath}\""),
                // Notepad (may be removed on some systems)
                ("notepad.exe", $"\"{filePath}\""),
            };

            foreach (var (exe, args) in editors)
            {
                if (string.IsNullOrEmpty(exe)) continue;
                try
                {
                    System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                    {
                        FileName = exe,
                        Arguments = args,
                        UseShellExecute = true
                    });
                    return true;
                }
                catch { }
            }
            return false;
        }

        private static string? FindInProgramFiles(string folder, string exe)
        {
            foreach (var root in new[]
            {
                Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
                Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86),
            })
            {
                var path = System.IO.Path.Combine(root, folder, exe);
                if (System.IO.File.Exists(path)) return path;
            }
            return null;
        }

        /// <summary>
        /// Returns a stable temp directory for previewing files from a given archive.
        /// All entries from the same archive share the same directory so that image viewers
        /// can navigate between files with arrow keys / "next" buttons.
        /// The directory name is derived from the archive path hash — deterministic and collision-free.
        /// </summary>
        private static string GetStablePreviewDir(string archivePath)
        {
            // Use a short hash of the full archive path as the directory name.
            // This is stable across sessions and unique per archive.
            int hash = archivePath.GetHashCode();
            string dirName = $"ZipEase_preview_{(uint)hash:x8}";
            return System.IO.Path.Combine(System.IO.Path.GetTempPath(), dirName);
        }

        /// <summary>
        /// Background pre-extraction: silently extracts all image files from the archive
        /// into the stable preview dir so the image viewer can navigate with arrow keys.
        /// For ZIP-based formats: concurrent per-entry extraction (max 3 parallel).
        /// For other formats (RAR/7z/TAR): single extract_direct call (most efficient).
        /// Fire-and-forget — never throws, never blocks UI.
        /// </summary>
        private void StartBackgroundImagePreExtract(string archivePath, string tempDir, bool isZipBased)
        {
            // Only trigger once per archive per session
            if (!_preExtractStarted.TryAdd(archivePath, true)) return;

            _ = Task.Run(async () =>
            {
                try
                {
                    if (isZipBased)
                    {
                        // ZIP: extract each image entry individually with concurrency limit
                        var imageEntries = _allEntries
                            .Where(e => !e.IsDirectory && ImageExtensions.Contains(
                                System.IO.Path.GetExtension(e.FileName)))
                            .ToList();

                        using var sem = new System.Threading.SemaphoreSlim(3);
                        var tasks = imageEntries.Select(async e =>
                        {
                            await sem.WaitAsync();
                            try
                            {
                                string fileNameOnly = System.IO.Path.GetFileName(e.FileName);
                                string destPath = System.IO.Path.Combine(tempDir, fileNameOnly);
                                if (System.IO.File.Exists(destPath)) return;

                                int index = _allEntries.FindIndex(x => x.FileName == e.FileName);
                                if (index < 0) return;

                                string extractedName = await ExtractionManager.ExtractEntryAsync(
                                    archivePath, (uint)index, tempDir);

                                // Flatten: move from subdir to root if needed
                                string ffiPath = System.IO.Path.Combine(tempDir, extractedName);
                                if (!System.IO.File.Exists(destPath) && System.IO.File.Exists(ffiPath)
                                    && ffiPath != destPath)
                                {
                                    System.IO.File.Move(ffiPath, destPath);
                                }

                                // Set read-only
                                if (System.IO.File.Exists(destPath))
                                {
                                    var attrs = System.IO.File.GetAttributes(destPath);
                                    if ((attrs & System.IO.FileAttributes.ReadOnly) == 0)
                                        System.IO.File.SetAttributes(destPath,
                                            attrs | System.IO.FileAttributes.ReadOnly);
                                }
                            }
                            catch { /* best-effort, ignore individual failures */ }
                            finally { sem.Release(); }
                        });
                        await Task.WhenAll(tasks);
                    }
                    else
                    {
                        // RAR/7z/TAR: extract image entries one by one using name-based extraction
                        // Do NOT use ExtractAsync (full archive) — that would dump all files including
                        // executables into the stable preview dir.
                        var imageEntries = _allEntries
                            .Where(e => !e.IsDirectory && ImageExtensions.Contains(
                                System.IO.Path.GetExtension(e.FileName)))
                            .ToList();

                        using var sem = new System.Threading.SemaphoreSlim(2);
                        var tasks = imageEntries.Select(async e =>
                        {
                            await sem.WaitAsync();
                            try
                            {
                                string fileNameOnly = System.IO.Path.GetFileName(e.FileName);
                                string destPath = System.IO.Path.Combine(tempDir, fileNameOnly);
                                if (System.IO.File.Exists(destPath) &&
                                    new System.IO.FileInfo(destPath).Length > 0) return;

                                string extractedName = await ExtractionManager.ExtractEntryByNameAsync(
                                    archivePath, e.FileName, tempDir);

                                // Flatten if needed
                                string ffiPath = System.IO.Path.Combine(tempDir, extractedName);
                                if (!System.IO.File.Exists(destPath) && System.IO.File.Exists(ffiPath)
                                    && ffiPath != destPath)
                                {
                                    System.IO.File.Move(ffiPath, destPath);
                                }

                                if (System.IO.File.Exists(destPath))
                                {
                                    var attrs = System.IO.File.GetAttributes(destPath);
                                    if ((attrs & System.IO.FileAttributes.ReadOnly) == 0)
                                        System.IO.File.SetAttributes(destPath,
                                            attrs | System.IO.FileAttributes.ReadOnly);
                                }
                            }
                            catch { }
                            finally { sem.Release(); }
                        });
                        await Task.WhenAll(tasks);
                    }
                }
                catch { /* top-level safety net */ }
            });
        }

        /// <summary>
        /// Extracts a single entry to a stable temp folder (shared per archive) and opens it
        /// with the system default application.
        /// Using a stable directory means image viewers can navigate between files in the same archive.
        /// Immutability: the temp file is set read-only so the user cannot accidentally modify it.
        /// Isolation: executable files require explicit user confirmation before launch.
        /// Temp dir is intentionally NOT deleted — the opened app still needs the file.
        /// </summary>
        [RelayCommand]
        private async Task PreviewEntry(ArchiveEntryViewModel? entry)
        {
            if (entry == null || entry.IsDirectory) return;
            if (string.IsNullOrEmpty(LoadedArchivePath)) return;

            // Warn before launching executables from an untrusted archive
            var ext = System.IO.Path.GetExtension(entry.FileName);
            if (ExecutableExtensions.Contains(ext))
            {
                var result = System.Windows.MessageBox.Show(
                    $"「{System.IO.Path.GetFileName(entry.FileName)}」是可執行程式。\n\n直接執行壓縮包內的程式可能有安全風險。確定要繼續嗎？",
                    "安全警告",
                    System.Windows.MessageBoxButton.YesNo,
                    System.Windows.MessageBoxImage.Warning,
                    System.Windows.MessageBoxResult.No);

                if (result != System.Windows.MessageBoxResult.Yes) return;
            }

            // Stable dir: same archive → same dir → image viewer can navigate between files
            string tempDir = GetStablePreviewDir(LoadedArchivePath);
            System.IO.Directory.CreateDirectory(tempDir);

            try
            {
                bool isZipBased = ZipBasedExtensions.Contains(
                    System.IO.Path.GetExtension(LoadedArchivePath));

                // Extract the entry — FFI returns the relative path within tempDir.
                // For ZIP: may be "subdir/file.jpg"; for non-ZIP: always just "file.jpg".
                // We always flatten to tempDir root so the image viewer can navigate between
                // all files in the same archive with arrow keys.
                string fileNameOnly = System.IO.Path.GetFileName(entry.FileName);
                string flatPath = System.IO.Path.Combine(tempDir, fileNameOnly);

                if (!System.IO.File.Exists(flatPath) ||
                    new System.IO.FileInfo(flatPath).Length == 0)
                {
                    string extractedName;
                    if (isZipBased)
                    {
                        int index = _allEntries.FindIndex(e => e.FileName == entry.FileName);
                        if (index < 0) return;
                        extractedName = await ExtractionManager.ExtractEntryAsync(
                            LoadedArchivePath, (uint)index, tempDir);
                    }
                    else
                    {
                        extractedName = await ExtractionManager.ExtractEntryByNameAsync(
                            LoadedArchivePath, entry.FileName, tempDir);
                    }

                    // If FFI placed the file in a subdirectory, move it to the flat root
                    string ffiPath = System.IO.Path.Combine(tempDir, extractedName);
                    if (!System.IO.File.Exists(flatPath) && System.IO.File.Exists(ffiPath))
                    {
                        // Remove read-only before moving (will re-apply after)
                        var attrs = System.IO.File.GetAttributes(ffiPath);
                        if ((attrs & System.IO.FileAttributes.ReadOnly) != 0)
                            System.IO.File.SetAttributes(ffiPath, attrs & ~System.IO.FileAttributes.ReadOnly);
                        System.IO.File.Move(ffiPath, flatPath);
                    }
                }

                string extractedPath = flatPath;
                if (!System.IO.File.Exists(extractedPath)) return;

                // Set file read-only for non-executables only — immutability for documents
                if (!ExecutableExtensions.Contains(ext))
                {
                    var attrs = System.IO.File.GetAttributes(extractedPath);
                    if ((attrs & System.IO.FileAttributes.ReadOnly) == 0)
                        System.IO.File.SetAttributes(extractedPath, attrs | System.IO.FileAttributes.ReadOnly);
                }

                // ShellExecuteEx via Process.Start
                try
                {
                    System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                    {
                        FileName = extractedPath,
                        UseShellExecute = true
                    });
                }
                catch (Exception ex)
                {
#if DEBUG_CONSOLE
                    DebugConsole.Log($"[Process.Start] FAILED for {extractedPath}: {ex.GetType().Name}: {ex.Message}");
#endif
                    // Shell has no association — check if it's a text-like format and try editor fallback chain
                    bool opened = TryOpenWithFallbackEditor(extractedPath);
                    if (!opened)
                    {
                        // Check if the extension has ANY registered association in the registry
                        bool hasAssociation = HasShellAssociation(System.IO.Path.GetExtension(extractedPath));
                        if (hasAssociation)
                        {
                            // Has association but still failed — likely a system error
                            ShowError($"無法開啟「{System.IO.Path.GetFileName(entry.FileName)}」：{ex.Message}");
                        }
                        else
                        {
                            // Truly no associated program
                            ShowError($"「{System.IO.Path.GetFileName(entry.FileName)}」沒有可開啟的程式。已開啟所在資料夾。");
                        }
                        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                        {
                            FileName = "explorer.exe",
                            Arguments = $"/select,\"{extractedPath}\"",
                            UseShellExecute = true
                        });
                    }
                }

                // Kick off background pre-extraction of all images in this archive
                // so the image viewer can navigate with arrow keys immediately
                StartBackgroundImagePreExtract(LoadedArchivePath, tempDir, isZipBased);
            }
            catch (ExtractionException ex)
            {
                ShowError($"無法預覽檔案: {ex.Message}");
            }
            catch (Exception ex)
            {
                ShowError($"預覽失敗: {ex.Message}");
            }
        }

        /// <summary>
        /// Extracts the entries passed in (from DataGrid.SelectedItems in code-behind).
        /// Uses the same output-dir memory as full extraction.
        /// </summary>
        [RelayCommand]
        private async Task ExtractSelected(System.Collections.IList? selectedItems)
        {
            var selected = selectedItems?
                .OfType<ArchiveEntryViewModel>()
                .Where(e => !e.IsDirectory)
                .ToList() ?? new System.Collections.Generic.List<ArchiveEntryViewModel>();
            if (selected.Count == 0 || string.IsNullOrEmpty(LoadedArchivePath)) return;

            var dialog = new System.Windows.Forms.FolderBrowserDialog
            {
                Description = "選擇輸出資料夾",
                UseDescriptionForTitle = true,
                SelectedPath = AppSettings.Instance.LastOutputDir
            };
            if (dialog.ShowDialog() != System.Windows.Forms.DialogResult.OK) return;

            string outputDir = dialog.SelectedPath;
            AppSettings.Instance.LastOutputDir = outputDir;
            AppSettings.Instance.Save();

            TransitionToExtracting();
            IsStatusVisible = false;
            int extracted = 0;

            try
            {
                foreach (var entry in selected)
                {
                    int index = _allEntries.FindIndex(e => e.FileName == entry.FileName);
                    if (index < 0) continue;

                    await ExtractionManager.ExtractEntryAsync(
                        LoadedArchivePath, (uint)index, outputDir);
                    extracted++;

                    WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                    {
                        ExtractionProgress = (int)((double)extracted / selected.Count * 100);
                        CurrentExtractionFile = entry.FileName;
                    });
                }

                TransitionBackToPreviewing();
                ShowSuccess(LocalizationManager.F("Status_ExtractSelectedSuccess", extracted, outputDir));
            }
            catch (ExtractionException ex)
            {
                TransitionBackToPreviewing();
                ShowError($"解壓縮失敗：{ex.Message}");
            }
            catch (Exception ex)
            {
                TransitionBackToPreviewing();
                ShowError($"未預期的錯誤：{ex.Message}");
            }
        }
        [RelayCommand(CanExecute = nameof(CanTrashSource))]
        private async Task TrashSource()
        {
            if (string.IsNullOrEmpty(LoadedArchivePath)) return;

            // Optimistically disable to prevent double-click
            IsTrashButtonEnabled = false;
            TrashSourceCommand.NotifyCanExecuteChanged();

            int result = await ExtractionManager.TrashFileAsync(LoadedArchivePath);

            WpfApplication.Current.Dispatcher.BeginInvoke(() =>
            {
                if (result == 0)
                {
                    ShowSuccess(LocalizationManager.Get("Status_Trashed"));
                }
                else
                {
                    IsTrashButtonEnabled = true;
                    TrashSourceCommand.NotifyCanExecuteChanged();
                    ShowError(LocalizationManager.Get("Status_TrashFailed"));
                }
            });
        }

        private bool CanTrashSource() => IsTrashButtonEnabled;

        [RelayCommand]
        private async Task RepairArchive()
        {
            if (string.IsNullOrEmpty(LoadedArchivePath)) return;

            IsRepairAvailable = false;
            IsStatusVisible = false;
            IsRepairSuccessVisible = false;
            TransitionToExtracting();
            CurrentExtractionFile = "正在掃描損壞狀況...";

            // Phase 1: Diagnose
            var report = await _repairService.DiagnoseAsync(LoadedArchivePath);
            if (report == null || !report.Repairable)
            {
                TransitionBackToPreviewing();
                ShowError("這個檔案損壞太嚴重，無法修復。你可以試試「強行提取」來搶救部分內容。");
                return;
            }

            // Phase 2: Repair with progress
            CurrentExtractionFile = "正在修復...";
            var progress = new Progress<RepairProgress>(p =>
            {
                WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                {
                    if (p.TotalSteps > 0)
                        ExtractionProgress = (int)((double)p.CurrentStep / p.TotalSteps * 100);
                    CurrentExtractionFile = $"修復中：{p.CurrentEntryName}";
                });
            });

            var result = await _repairService.RepairAsync(LoadedArchivePath, null, progress);
            TransitionBackToPreviewing();

            if (result == null)
            {
                ShowError("這個檔案損壞太嚴重，無法修復。");
                return;
            }

            if (result.Success)
            {
                RepairedFilePath = result.RepairedPath ?? "";
                IsRepairSuccessVisible = true;
                ShowSuccess("修復完成！所有檔案已成功恢復。");
            }
            else if (!string.IsNullOrEmpty(result.RepairedPath))
            {
                // Partial success
                int recovered = result.RecoveredEntries.Count;
                int failed = result.FailedEntries.Count;
                RepairedFilePath = result.RepairedPath;
                IsRepairSuccessVisible = true;
                ShowSuccess($"{recovered} 個檔案已修復，{failed} 個檔案無法恢復");
            }
            else
            {
                ShowError("這個檔案損壞太嚴重，無法修復。");
            }
        }

        [RelayCommand]
        private void OpenRepairedFile()
        {
            if (string.IsNullOrEmpty(RepairedFilePath)) return;
            // Load the repaired archive in ZipEase
            LoadArchive(RepairedFilePath);
            IsRepairSuccessVisible = false;
        }

        private void LoadArchive(string archivePath)
        {
            IsStatusVisible = false;
            _navigationStack.Clear();
            CurrentPath = string.Empty;
            _passwordAttempts = 0;
            // Show extracting state as loading indicator while FFI runs
            TransitionToExtracting();
            _ = LoadArchiveAsync(archivePath);
        }

        private async Task LoadArchiveAsync(string archivePath)
        {
            CurrentExtractionFile = "載入中...";
            ExtractionProgress = 0;

            ListResult result;
            List<ArchiveEntry> entries;
            string? errorMsg;
            IntPtr nativePtr;
            int nativeCount;

            try
            {
                // Run FFI on background thread — never block UI thread
                (result, entries, errorMsg, nativePtr, nativeCount) = await Task.Run(() =>
                    _previewService.ListArchiveContentsKeepNative(archivePath, _pendingPassword));
            }
            catch (Exception ex)
            {
                CurrentExtractionFile = string.Empty;
                ShowError(LocalizationManager.F("Status_LoadFailed", ex.Message));
                TransitionToIdle();
                return;
            }

            CurrentExtractionFile = string.Empty;

            if (result == ListResult.PasswordRequired)
            {
                _passwordAttempts++;
                if (_passwordAttempts > 3)
                {
                    _pendingPassword = null;
                    _passwordAttempts = 0;
                    ShowError(LocalizationManager.Get("Status_WrongPassword"));
                    TransitionToIdle();
                    return;
                }

                string? pwErrorMsg = _passwordAttempts > 1 ? LocalizationManager.Get("PasswordDialog_WrongPassword") : null;
                var pwDialog = new PasswordDialog(pwErrorMsg);
                bool confirmed = pwDialog.ShowDialog() == true;

                if (!confirmed)
                {
                    _pendingPassword = null;
                    _passwordAttempts = 0;
                    TransitionToIdle();
                    return;
                }

                _pendingPassword = pwDialog.Password;
                await LoadArchiveAsync(archivePath);
                return;
            }

            if (result == ListResult.Error)
            {
                ShowError(LocalizationManager.F("Status_LoadFailed", errorMsg ?? "Unknown error"));
                TransitionToIdle();
                return;
            }

            if (result == ListResult.ZipBomb)
            {
                ShowError(errorMsg ?? "壓縮炸彈偵測");
                TransitionToIdle();
                return;
            }

            _allEntries = entries;
            // Store native pointer for Rust-backed search
            FreeNativeEntries(); // Free any previous pointer
            _nativeEntriesPtr = nativePtr;
            _nativeEntryCount = nativeCount;
            TransitionToPreviewing(archivePath);
            RefreshEntriesForCurrentPath();
            _passwordAttempts = 0;

#if DEBUG_CONSOLE
            DebugConsole.Log($"[LoadArchive] {System.IO.Path.GetFileName(archivePath)}: {_allEntries.Count} entries total, {ArchiveEntries.Count} shown at root");
            foreach (var e in _allEntries.Take(10))
                DebugConsole.Log($"  entry: isDir={e.IsDirectory} name={e.FileName}");
#endif

            // Auto-navigate: if root has exactly one directory and no files, go into it
            // This handles archives like "archive.zip/MyFolder/file1.jpg, file2.jpg"
            var rootFiles = ArchiveEntries.Where(e => !e.IsDirectory).ToList();
            var rootDirs  = ArchiveEntries.Where(e => e.IsDirectory).ToList();
            if (rootFiles.Count == 0 && rootDirs.Count == 1)
            {
                _navigationStack.Push(CurrentPath);
                CurrentPath = rootDirs[0].FileName.TrimEnd('/') + "/";
                RefreshEntriesForCurrentPath();
                NavigateBackCommand.NotifyCanExecuteChanged();
            }
            // If root is completely empty but _allEntries has data, the paths may have
            // a common prefix — show all entries flat as fallback
            else if (ArchiveEntries.Count == 0 && _allEntries.Count > 0)
            {
                // Flat fallback: show all non-directory entries regardless of path
                ArchiveEntries.Clear();
                foreach (var entry in _allEntries)
                {
                    if (entry.IsDirectory) continue;
                    ArchiveEntries.Add(new ArchiveEntryViewModel(entry));
                }
                OnPropertyChanged(nameof(FileCount));
                OnPropertyChanged(nameof(FolderCount));
                OnPropertyChanged(nameof(ArchiveSummary));
            }
        }
    }
}



