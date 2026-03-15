using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Windows;
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

        [ObservableProperty] private UIState _currentState = UIState.Idle;
        [ObservableProperty] private string _loadedArchivePath = string.Empty;
        [ObservableProperty] private int _extractionProgress;
        [ObservableProperty] private string _currentExtractionFile = string.Empty;
        [ObservableProperty] private string _statusMessage = string.Empty;
        [ObservableProperty] private bool _isStatusVisible;
        [ObservableProperty] private bool _isStatusError;
        [ObservableProperty] private string _currentPath = string.Empty;

        // Navigation state
        private readonly Stack<string> _navigationStack = new();
        private List<ArchiveEntry> _allEntries = new();

        // Password state (never exposed as public property)
        private string? _pendingPassword;
        private int _passwordAttempts;

        public ObservableCollection<ArchiveEntryViewModel> ArchiveEntries { get; } = new();

        // Computed properties
        public bool IsIdleVisible => CurrentState == UIState.Idle || CurrentState == UIState.DragOver;
        public bool IsPreviewVisible => CurrentState == UIState.Previewing || CurrentState == UIState.Extracting;
        public bool IsProgressVisible => CurrentState == UIState.Extracting;
        public bool IsExtractButtonEnabled => CurrentState == UIState.Previewing;
        public bool IsDragOverActive => CurrentState == UIState.DragOver;
        public bool IsBackButtonVisible => !string.IsNullOrEmpty(CurrentPath);
        public int FileCount => ArchiveEntries.Count(e => !e.IsDirectory);

        public MainWindowViewModel(ArchivePreviewService previewService)
        {
            _previewService = previewService;
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
            NotifyVisibilityChanged();
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

        public void TransitionBackToPreviewing()
        {
            CurrentState = UIState.Previewing;
            ExtractionProgress = 0;
            CurrentExtractionFile = string.Empty;
            NotifyVisibilityChanged();
        }

        public void ShowError(string message)
        {
            StatusMessage = message;
            IsStatusError = true;
            IsStatusVisible = true;
        }

        public void ShowSuccess(string message)
        {
            StatusMessage = message;
            IsStatusError = false;
            IsStatusVisible = true;
        }

        private void NotifyVisibilityChanged()
        {
            OnPropertyChanged(nameof(IsIdleVisible));
            OnPropertyChanged(nameof(IsPreviewVisible));
            OnPropertyChanged(nameof(IsProgressVisible));
            OnPropertyChanged(nameof(IsExtractButtonEnabled));
            OnPropertyChanged(nameof(IsDragOverActive));
            OnPropertyChanged(nameof(IsBackButtonVisible));
            OnPropertyChanged(nameof(FileCount));
        }

        // Navigation helpers
        private void RefreshEntriesForCurrentPath()
        {
            ArchiveEntries.Clear();
            foreach (var entry in _allEntries)
            {
                if (GetImmediateParent(entry.FileName) == CurrentPath)
                    ArchiveEntries.Add(new ArchiveEntryViewModel(entry));
            }
            OnPropertyChanged(nameof(FileCount));
            OnPropertyChanged(nameof(IsBackButtonVisible));
        }

        internal static string GetImmediateParent(string entryName)
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
                Filter = "Archive Files|*.zip;*.rar;*.7z;*.tar;*.gz|All Files|*.*"
            };

            if (dialog.ShowDialog() != true) return;

            LoadArchive(dialog.FileName);
        }

        [RelayCommand]
        private async Task Extract()
        {
            if (string.IsNullOrEmpty(LoadedArchivePath)) return;

            var dialog = new System.Windows.Forms.FolderBrowserDialog
            {
                Description = "Select output folder",
                UseDescriptionForTitle = true
            };

            if (dialog.ShowDialog() != System.Windows.Forms.DialogResult.OK) return;

            string outputDir = dialog.SelectedPath;
            TransitionToExtracting();
            IsStatusVisible = false;

            try
            {
                int fileCount = await ExtractionManager.ExtractAsync(
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
                TransitionBackToPreviewing();
                ShowSuccess($"Successfully extracted {fileCount} files to {outputDir}");
            }
            catch (ExtractionException ex) when (ex.ErrorCode == unchecked((int)0x2004))
            {
                TransitionBackToPreviewing();
                await HandlePasswordRequiredForExtraction();
            }
            catch (ExtractionException ex)
            {
                TransitionBackToPreviewing();
                ShowError($"Extraction failed: {ex.Message}");
            }
            catch (Exception ex)
            {
                TransitionBackToPreviewing();
                ShowError($"Unexpected error: {ex.Message}");
            }
        }

        private async Task HandlePasswordRequiredForExtraction()
        {
            _passwordAttempts++;
            if (_passwordAttempts > 3)
            {
                _pendingPassword = null;
                _passwordAttempts = 0;
                ShowError("密碼錯誤，已取消開啟壓縮檔");
                return;
            }

            string? errorMsg = _passwordAttempts > 1 ? "密碼錯誤，請重試" : null;
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
                ShowError("Unsupported format. Supported: .zip, .rar, .7z, .tar, .gz");
                return;
            }
            LoadArchive(filePath);
        }

        private void LoadArchive(string archivePath)
        {
            IsStatusVisible = false;
            _navigationStack.Clear();
            CurrentPath = string.Empty;
            _passwordAttempts = 0;

            try
            {
                var (result, entries, _) = _previewService.ListArchiveContentsWithPassword(archivePath, _pendingPassword);

                if (result == ListResult.PasswordRequired)
                {
                    _passwordAttempts++;
                    if (_passwordAttempts > 3)
                    {
                        _pendingPassword = null;
                        _passwordAttempts = 0;
                        ShowError("密碼錯誤，已取消開啟壓縮檔");
                        TransitionToIdle();
                        return;
                    }

                    WpfApplication.Current.Dispatcher.BeginInvoke(() =>
                    {
                        string? errorMsg = _passwordAttempts > 1 ? "密碼錯誤，請重試" : null;
                        var pwDialog = new PasswordDialog(errorMsg);
                        bool confirmed = pwDialog.ShowDialog() == true;

                        if (!confirmed)
                        {
                            _pendingPassword = null;
                            _passwordAttempts = 0;
                            TransitionToIdle();
                            return;
                        }

                        _pendingPassword = pwDialog.Password;
                        LoadArchive(archivePath);
                    });
                    return;
                }

                _allEntries = entries;
                TransitionToPreviewing(archivePath);
                RefreshEntriesForCurrentPath();
                _passwordAttempts = 0;
            }
            catch (ExtractionException ex)
            {
                ShowError($"Failed to load archive: {ex.Message}");
                TransitionToIdle();
            }
            catch (Exception ex)
            {
                ShowError($"Unexpected error: {ex.Message}");
                TransitionToIdle();
            }
        }
    }
}
