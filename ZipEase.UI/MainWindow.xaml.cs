using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using Wpf.Ui.Controls;
using ZipEase.UI.Core;
using Brush = System.Windows.Media.Brush;
using Brushes = System.Windows.Media.Brushes;
using DragEventArgs = System.Windows.DragEventArgs;
using DragDropEffects = System.Windows.DragDropEffects;
using DataFormats = System.Windows.DataFormats;
using MouseEventArgs = System.Windows.Input.MouseEventArgs;
using WpfDataGrid = System.Windows.Controls.DataGrid;

namespace ZipEase.UI;

public partial class MainWindow : FluentWindow
{
    private MainWindowViewModel ViewModel => (MainWindowViewModel)DataContext;

    // ── Image Preview ─────────────────────────────────────────────────────────
    private readonly PreviewViewModel _previewViewModel = new();
    private readonly ThumbnailService _thumbnailService = new();

    public MainWindow()
    {
        InitializeComponent();
        DataContext = new MainWindowViewModel(new ArchivePreviewService());
        CompressPanel.DataContext = new CompressViewModel();
        SetActiveNav(NavPage.Extract);

        // Wire image preview panel
        ImagePreviewPanel.DataContext = _previewViewModel;
        _previewViewModel.Closed += OnPreviewPanelClosed;
        _previewViewModel.Navigation.NavigationRequested += OnPreviewNavigationRequested;

        // Wire archive close detection (for cache/temp cleanup)
        ((MainWindowViewModel)DataContext).PropertyChanged += OnViewModelPropertyChanged;

        // Natural sort for file name column
        ArchiveDataGrid.Loaded += (_, _) =>
        {
            var view = System.Windows.Data.CollectionViewSource.GetDefaultView(ArchiveDataGrid.ItemsSource);
            if (view is System.Windows.Data.ListCollectionView lcv)
                lcv.CustomSort = NaturalFileNameComparer.Instance;
        };

        // Wire selection changed to dynamicly update extraction button content
        ArchiveDataGrid.SelectionChanged += OnDataGridSelectionChanged;


        // Restore backdrop from settings
        BackdropSwitcher.Apply(AppSettings.Instance.BackdropType, this);

        // Handle command-line startup modes (files passed from Shell Extension)
        HandleStartupArgs();

        // Startup cleanup: remove stale temp files from previous sessions (Req 10.4)
        System.Threading.Tasks.Task.Run(() =>
        {
            try { ImagePreviewNative.StartupCleanup(); }
            catch { /* Best-effort — do not crash on startup cleanup failure */ }
        });

        // Wire application exit to cleanup all temps (Req 10.3)
        System.Windows.Application.Current.Exit += OnApplicationExit;
    }

    /// <summary>
    /// Processes command-line parse results set by App.OnStartup.
    /// On Extract mode: loads archive files into the extract view.
    /// On Compress mode: switches to compress view with files pre-loaded.
    /// </summary>
    private void HandleStartupArgs()
    {
        var parseResult = App.StartupParseResult;
        if (parseResult is null)
            return;

        switch (parseResult.Mode)
        {
            case CommandLineParser.Mode.Extract:
                // Load archive files via the existing batch extract command
                ViewModel.BatchExtractCommand.Execute(parseResult.ValidPaths);
                break;

            case CommandLineParser.Mode.Compress:
                // Switch to compress view and pre-load files
                SetActiveNav(NavPage.Compress);
                var compressVm = (CompressViewModel)CompressPanel.DataContext;
                compressVm.AddDroppedFilesCommand.Execute(parseResult.ValidPaths);
                break;
        }
    }

    // ── Nav ───────────────────────────────────────────────────────────────────

    private enum NavPage { Extract, Compress, Settings }

    private void SetActiveNav(NavPage page)
    {
        var accent      = (Brush)FindResource("SystemAccentColorPrimaryBrush");
        var accentText  = (Brush)FindResource("TextOnAccentFillColorPrimaryBrush");
        var none        = Brushes.Transparent;
        var defaultText = (Brush)FindResource("TextFillColorPrimaryBrush");

        void Style(Border border, System.Windows.Controls.TextBlock text, Wpf.Ui.Controls.SymbolIcon icon, bool active)
        {
            border.Background   = active ? accent       : none;
            text.Foreground     = active ? accentText   : defaultText;
            icon.Foreground     = active ? accentText   : defaultText;
        }

        Style(ExtractNavItem,  ExtractNavText,  ExtractNavIcon,  page == NavPage.Extract);
        Style(CompressNavItem, CompressNavText, CompressNavIcon, page == NavPage.Compress);
        Style(SettingsNavItem, SettingsNavText, SettingsNavIcon, page == NavPage.Settings);

        ExtractPanel.Visibility  = page == NavPage.Extract   ? Visibility.Visible : Visibility.Collapsed;
        CompressPanel.Visibility = page == NavPage.Compress  ? Visibility.Visible : Visibility.Collapsed;
        SettingsPanel.Visibility = page == NavPage.Settings  ? Visibility.Visible : Visibility.Collapsed;
    }

    private void OnExtractTabClick(object sender, MouseButtonEventArgs e)  => SetActiveNav(NavPage.Extract);
    private void OnCompressTabClick(object sender, MouseButtonEventArgs e) => SetActiveNav(NavPage.Compress);
    private void OnSettingsTabClick(object sender, MouseButtonEventArgs e) => SetActiveNav(NavPage.Settings);

    // ── Drag & Drop ───────────────────────────────────────────────────────────

    private void OnDragEnter(object sender, DragEventArgs e)
    {
        if (CompressPanel.Visibility == Visibility.Visible ||
            SettingsPanel.Visibility == Visibility.Visible)
        { e.Effects = DragDropEffects.None; return; }

        if (ViewModel.CurrentState != UIState.Idle) { e.Effects = DragDropEffects.None; return; }

        if (e.Data.GetDataPresent(DataFormats.FileDrop))
        {
            var files = (string[])e.Data.GetData(DataFormats.FileDrop);
            var svc = new ArchivePreviewService();
            if (files?.Any(f => svc.IsSupportedArchive(f)) == true)
            {
                e.Effects = DragDropEffects.Copy;
                ViewModel.TransitionToDragOver();
                e.Handled = true;
                return;
            }
        }
        e.Effects = DragDropEffects.None;
        e.Handled = true;
    }

    private void OnDragOver(object sender, DragEventArgs e) => e.Handled = true;

    private void OnDragLeave(object sender, DragEventArgs e)
    {
        if (ViewModel.CurrentState == UIState.DragOver)
            ViewModel.TransitionToIdle();
    }

    private void OnDrop(object sender, DragEventArgs e)
    {
        if (CompressPanel.Visibility == Visibility.Visible ||
            SettingsPanel.Visibility == Visibility.Visible) return;
        if (!e.Data.GetDataPresent(DataFormats.FileDrop)) return;
        var files = (string[])e.Data.GetData(DataFormats.FileDrop);
        if (files?.Length > 0)
            ViewModel.BatchExtractCommand.Execute(files);
    }

    private void OnDropZoneClick(object sender, MouseButtonEventArgs e)
        => ViewModel.BrowseFileCommand.Execute(null);

    // ── Extract Selected ──────────────────────────────────────────────────────

    private void OnDataGridSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (ExtractButton == null) return;
        if (ArchiveDataGrid.SelectedItems.Count > 0)
        {
            ExtractButton.Content = LocalizationManager.Get("Extract_ExtractSelected");
            ExtractButton.Icon = new Wpf.Ui.Controls.SymbolIcon { Symbol = Wpf.Ui.Controls.SymbolRegular.ArrowDownload24 };
        }
        else
        {
            ExtractButton.Content = LocalizationManager.Get("Extract_ExtractAll");
            ExtractButton.Icon = new Wpf.Ui.Controls.SymbolIcon { Symbol = Wpf.Ui.Controls.SymbolRegular.FolderArrowUp24 };
        }
    }


    private void OnExtractClick(object sender, RoutedEventArgs e)
    {
        if (ArchiveDataGrid.SelectedItems.Count > 0)
        {
            ViewModel.ExtractSelectedCommand.Execute(ArchiveDataGrid.SelectedItems);
        }
        else
        {
            ViewModel.ExtractCommand.Execute(null);
        }
    }


    // ── Compress Drag & Drop ──────────────────────────────────────────────────

    private void OnCompressDragEnter(object sender, DragEventArgs e)
    {
        if (!e.Data.GetDataPresent(DataFormats.FileDrop)) { e.Effects = DragDropEffects.None; return; }
        e.Effects = DragDropEffects.Copy;
        e.Handled = true;
    }

    private void OnCompressDragOver(object sender, DragEventArgs e)
    {
        e.Effects = e.Data.GetDataPresent(DataFormats.FileDrop) ? DragDropEffects.Copy : DragDropEffects.None;
        e.Handled = true;
    }

    private void OnCompressDrop(object sender, DragEventArgs e)
    {
        if (!e.Data.GetDataPresent(DataFormats.FileDrop)) return;
        var files = (string[])e.Data.GetData(DataFormats.FileDrop);
        if (files == null) return;
        var vm = (CompressViewModel)CompressPanel.DataContext;
        vm.AddDroppedFilesCommand.Execute(files);
    }

    // ── DataGrid ──────────────────────────────────────────────────────────────

    private void OnDataGridDoubleClick(object sender, MouseButtonEventArgs e)
    {
        if (sender is WpfDataGrid grid && grid.SelectedItem is ArchiveEntryViewModel entry)
        {
            if (entry.IsDirectory)
            {
                ViewModel.NavigateIntoCommand.Execute(entry);
            }
            else if (ThumbnailService.IsPreviewable(entry.FileName))
            {
                // Open in-app image preview panel (Req 2.1)
                ShowImagePreview(entry);
            }
            else
            {
                ViewModel.PreviewEntryCommand.Execute(entry);
            }
        }
    }

    private System.Windows.Point _dragStartPoint;

    private void OnDataGridPreviewMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        => _dragStartPoint = e.GetPosition(null);

    private void OnDataGridPreviewMouseMove(object sender, MouseEventArgs e)
    {
        if (e.LeftButton != MouseButtonState.Pressed) return;

        var diff = e.GetPosition(null) - _dragStartPoint;
        if (System.Math.Abs(diff.X) < SystemParameters.MinimumHorizontalDragDistance &&
            System.Math.Abs(diff.Y) < SystemParameters.MinimumVerticalDragDistance) return;

        if (ArchiveDataGrid.SelectedItem is not ArchiveEntryViewModel entry || entry.IsDirectory) return;

        if (ViewModel.ExtractSingleEntryCommand.CanExecute(entry))
            ViewModel.ExtractSingleEntryCommand.Execute(entry);
    }

    // ── Search ────────────────────────────────────────────────────────────────

    private void OnSearchBoxKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (e.Key == Key.Escape)
        {
            ViewModel.SearchText = string.Empty;
            SearchBox.MoveFocus(new System.Windows.Input.TraversalRequest(
                System.Windows.Input.FocusNavigationDirection.Next));
            e.Handled = true;
        }
    }

    // ── Image Preview Integration ─────────────────────────────────────────────

    /// <summary>
    /// Detects when the archive is closed (state transitions to Idle) to trigger cleanup.
    /// </summary>
    private string _lastLoadedArchivePath = string.Empty;

    private void OnViewModelPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(MainWindowViewModel.LoadedArchivePath))
        {
            // Track the archive path so we can clean up when it's cleared
            if (!string.IsNullOrEmpty(ViewModel.LoadedArchivePath))
                _lastLoadedArchivePath = ViewModel.LoadedArchivePath;
        }

        if (e.PropertyName == nameof(MainWindowViewModel.CurrentState) &&
            ViewModel.CurrentState == UIState.Idle &&
            !string.IsNullOrEmpty(_lastLoadedArchivePath))
        {
            OnArchiveClosed();
            _lastLoadedArchivePath = string.Empty;
        }
    }

    /// <summary>
    /// Handles navigation requests from the PreviewPanel (next/previous image).
    /// Extracts the target entry to temp if needed, then the ViewModel's internal
    /// handler will load it via LoadImageAsync.
    /// </summary>
    private async void OnPreviewNavigationRequested(object? sender, string tempFilePath)
    {
        if (string.IsNullOrEmpty(ViewModel.LoadedArchivePath)) return;
        if (System.IO.File.Exists(tempFilePath)) return; // Already extracted

        string tempDir = GetStablePreviewDir(ViewModel.LoadedArchivePath);
        string fileName = System.IO.Path.GetFileName(tempFilePath);

        // Find the archive entry that corresponds to this temp file
        var entry = ViewModel.ArchiveEntries.FirstOrDefault(
            e => !e.IsDirectory && System.IO.Path.GetFileName(e.FileName) == fileName);
        if (entry == null) return;

        try
        {
            string extractedPath = await ExtractEntryToTemp(entry, tempDir);
            if (!string.IsNullOrEmpty(extractedPath))
            {
                // Re-trigger load now that the file exists
                await _previewViewModel.LoadImageAsync(extractedPath);
            }
        }
        catch (Exception ex)
        {
            ViewModel.ShowError($"無法預覽圖片: {ex.Message}");
        }
    }

    /// <summary>
    /// Shows the image preview panel and loads the selected entry.
    /// Extracts the file to a temp directory, then calls LoadImageAsync on the ViewModel.
    /// Also initializes navigation with all previewable entries in the same directory.
    /// </summary>
    private async void ShowImagePreview(ArchiveEntryViewModel entry)
    {
        if (string.IsNullOrEmpty(ViewModel.LoadedArchivePath)) return;

        ImagePreviewPanel.Visibility = Visibility.Visible;

        // Build navigation list: all previewable entries in the same directory
        string entryDir = GetEntryDirectory(entry.FileName);
        var previewableEntries = ViewModel.ArchiveEntries
            .Where(e => !e.IsDirectory && ThumbnailService.IsPreviewable(e.FileName))
            .Where(e => GetEntryDirectory(e.FileName) == entryDir)
            .Select(e => e.FileName)
            .OrderBy(n => n, StringComparer.OrdinalIgnoreCase)
            .ToList();

        // Build expected temp file paths for navigation
        string tempDir = GetStablePreviewDir(ViewModel.LoadedArchivePath);
        System.IO.Directory.CreateDirectory(tempDir);

        var tempPaths = previewableEntries
            .Select(name => System.IO.Path.Combine(tempDir, System.IO.Path.GetFileName(name)))
            .ToList();

        string currentTempPath = System.IO.Path.Combine(tempDir, System.IO.Path.GetFileName(entry.FileName));
        _previewViewModel.InitializeNavigation(tempPaths, currentTempPath);

        try
        {
            string extractedPath = await ExtractEntryToTemp(entry, tempDir);
            if (!string.IsNullOrEmpty(extractedPath))
            {
                await _previewViewModel.LoadImageAsync(extractedPath);
            }
        }
        catch (Exception ex)
        {
            ViewModel.ShowError($"無法預覽圖片: {ex.Message}");
            ImagePreviewPanel.Visibility = Visibility.Collapsed;
        }

        // Give focus to the preview panel for keyboard navigation
        ImagePreviewPanel.Focus();
    }

    /// <summary>
    /// Handles the PreviewPanel close event: hides the panel and returns focus to the file list.
    /// </summary>
    private void OnPreviewPanelClosed(object? sender, EventArgs e)
    {
        ImagePreviewPanel.Visibility = Visibility.Collapsed;
        ArchiveDataGrid.Focus();
    }

    /// <summary>
    /// Handles application exit: cleans up all preview temp files (Req 10.3).
    /// </summary>
    private void OnApplicationExit(object sender, ExitEventArgs e)
    {
        try
        {
            _thumbnailService.Dispose();
            ImagePreviewNative.CleanupAllTemps();
        }
        catch { /* Best-effort cleanup on exit */ }
    }

    /// <summary>
    /// Called when the archive is closed (Reset). Clears thumbnail cache and preview temps.
    /// Wired to ViewModel state changes.
    /// </summary>
    private void OnArchiveClosed()
    {
        _thumbnailService.CancelAndClear();

        // Cleanup archive-specific temps via Rust FFI (Req 10.2)
        if (!string.IsNullOrEmpty(_lastLoadedArchivePath))
        {
            string archiveId = GetArchiveId(_lastLoadedArchivePath);
            IntPtr ptr = Marshal.StringToHGlobalUni(archiveId);
            try
            {
                ImagePreviewNative.CleanupArchive(ptr, archiveId.Length);
            }
            finally
            {
                Marshal.FreeHGlobal(ptr);
            }
        }

        // Hide preview panel if open
        ImagePreviewPanel.Visibility = Visibility.Collapsed;
    }

    /// <summary>
    /// Extracts an archive entry to the temp directory and returns the extracted file path.
    /// </summary>
    private async Task<string> ExtractEntryToTemp(ArchiveEntryViewModel entry, string tempDir)
    {
        string fileNameOnly = System.IO.Path.GetFileName(entry.FileName);
        string flatPath = System.IO.Path.Combine(tempDir, fileNameOnly);

        // If already extracted, return cached path
        if (System.IO.File.Exists(flatPath) && new System.IO.FileInfo(flatPath).Length > 0)
            return flatPath;

        bool isZipBased = ZipBasedExtensions.Contains(
            System.IO.Path.GetExtension(ViewModel.LoadedArchivePath));

        string extractedName;
        if (isZipBased)
        {
            int index = FindEntryIndex(entry.FileName);
            if (index < 0) return string.Empty;
            extractedName = await ExtractionManager.ExtractEntryAsync(
                ViewModel.LoadedArchivePath, (uint)index, tempDir);
        }
        else
        {
            extractedName = await ExtractionManager.ExtractEntryByNameAsync(
                ViewModel.LoadedArchivePath, entry.FileName, tempDir);
        }

        // If FFI placed the file in a subdirectory, move it to the flat root
        string ffiPath = System.IO.Path.Combine(tempDir, extractedName);
        if (!System.IO.File.Exists(flatPath) && System.IO.File.Exists(ffiPath))
        {
            var attrs = System.IO.File.GetAttributes(ffiPath);
            if ((attrs & System.IO.FileAttributes.ReadOnly) != 0)
                System.IO.File.SetAttributes(ffiPath, attrs & ~System.IO.FileAttributes.ReadOnly);
            System.IO.File.Move(ffiPath, flatPath);
        }

        return flatPath;
    }

    /// <summary>
    /// Gets the parent directory of an archive entry name.
    /// </summary>
    private static string GetEntryDirectory(string entryName)
    {
        var trimmed = entryName.Replace('\\', '/').TrimEnd('/');
        var lastSlash = trimmed.LastIndexOf('/');
        return lastSlash < 0 ? string.Empty : trimmed[..(lastSlash + 1)];
    }

    /// <summary>
    /// Generates a stable preview directory path for an archive (deterministic by path hash).
    /// </summary>
    private static string GetStablePreviewDir(string archivePath)
    {
        string archiveId = GetArchiveId(archivePath);
        return System.IO.Path.Combine(System.IO.Path.GetTempPath(), "ZipEase_preview_" + archiveId);
    }

    /// <summary>
    /// Generates a short hash-based ID for an archive path.
    /// </summary>
    private static string GetArchiveId(string archivePath)
    {
        using var sha = System.Security.Cryptography.SHA256.Create();
        var bytes = System.Text.Encoding.UTF8.GetBytes(archivePath);
        var hash = sha.ComputeHash(bytes);
        return Convert.ToHexString(hash)[..16].ToLowerInvariant();
    }

    /// <summary>
    /// Finds the index of an entry in the all-entries list by file name.
    /// Uses reflection to access the private _allEntries field in the ViewModel.
    /// Returns -1 if not found.
    /// </summary>
    private int FindEntryIndex(string fileName)
    {
        // Search through the visible entries to find the matching one
        for (int i = 0; i < ViewModel.ArchiveEntries.Count; i++)
        {
            if (ViewModel.ArchiveEntries[i].FileName == fileName)
                return i;
        }
        return -1;
    }

    /// <summary>
    /// Set of ZIP-based archive extensions (used to determine extraction method).
    /// </summary>
    private static readonly HashSet<string> ZipBasedExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".zip", ".apk", ".jar", ".docx", ".xlsx", ".pptx", ".odt", ".ods", ".odp", ".epub"
    };
}
