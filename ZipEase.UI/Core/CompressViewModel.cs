using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

namespace ZipEase.UI.Core
{
    public enum CompressState { Idle, FilesSelected, Compressing, Done, Error }

    public partial class CompressViewModel : ObservableObject
    {
        private readonly CompressionService _service = new();
        private CancellationTokenSource? _cts;

        [ObservableProperty] private CompressState _state = CompressState.Idle;
        [ObservableProperty] private string _outputPath = string.Empty;
        [ObservableProperty] private string _selectedFormat = "zip";
        [ObservableProperty] private int _level = 6;
        [ObservableProperty] private int _compressProgress;
        [ObservableProperty] private string _currentFile = string.Empty;
        [ObservableProperty] private string _statusMessage = string.Empty;
        [ObservableProperty] private bool _isStatusVisible;
        [ObservableProperty] private bool _isStatusError;

        public ObservableCollection<string> SelectedFiles { get; } = new();

        // Computed
        public bool IsCompressing => State == CompressState.Compressing;
        public bool CanCompress => State == CompressState.FilesSelected && !string.IsNullOrEmpty(OutputPath);
        public bool IsProgressVisible => State == CompressState.Compressing;

        partial void OnStateChanged(CompressState value)
        {
            OnPropertyChanged(nameof(IsCompressing));
            OnPropertyChanged(nameof(CanCompress));
            OnPropertyChanged(nameof(IsProgressVisible));
            CompressCommand.NotifyCanExecuteChanged();
        }

        partial void OnOutputPathChanged(string value)
        {
            OnPropertyChanged(nameof(CanCompress));
            CompressCommand.NotifyCanExecuteChanged();
        }

        [RelayCommand]
        private void AddFiles()
        {
            var dialog = new Microsoft.Win32.OpenFileDialog
            {
                Title = "選擇要壓縮的檔案",
                Filter = "All Files|*.*",
                Multiselect = true
            };
            if (dialog.ShowDialog() != true) return;

            foreach (var file in dialog.FileNames)
                if (!SelectedFiles.Contains(file))
                    SelectedFiles.Add(file);

            if (SelectedFiles.Count > 0 && State == CompressState.Idle)
                State = CompressState.FilesSelected;

            OnPropertyChanged(nameof(CanCompress));
            CompressCommand.NotifyCanExecuteChanged();
        }

        [RelayCommand]
        private void BrowseOutput()
        {
            var ext = SelectedFormat switch
            {
                "7z" => "7z",
                "tar.gz" => "gz",
                _ => "zip"
            };
            var filter = SelectedFormat switch
            {
                "7z" => "7-Zip Archive|*.7z",
                "tar.gz" => "TAR GZ Archive|*.tar.gz",
                _ => "ZIP Archive|*.zip"
            };

            var dialog = new Microsoft.Win32.SaveFileDialog
            {
                Title = "選擇輸出位置",
                Filter = filter,
                DefaultExt = ext
            };
            if (dialog.ShowDialog() != true) return;

            OutputPath = dialog.FileName;
        }

        [RelayCommand(CanExecute = nameof(CanCompress))]
        private async Task Compress()
        {
            if (SelectedFiles.Count == 0 || string.IsNullOrEmpty(OutputPath)) return;

            State = CompressState.Compressing;
            IsStatusVisible = false;
            CompressProgress = 0;
            CurrentFile = string.Empty;

            _cts = new CancellationTokenSource();

            var progress = new Progress<(int Pct, string File)>(report =>
            {
                System.Windows.Application.Current.Dispatcher.BeginInvoke(() =>
                {
                    CompressProgress = report.Pct;
                    CurrentFile = report.File;
                });
            });

            try
            {
                await _service.CompressAsync(
                    new System.Collections.Generic.List<string>(SelectedFiles),
                    OutputPath,
                    Level,
                    progress,
                    _cts.Token);

                State = CompressState.Done;
                StatusMessage = $"壓縮完成：{Path.GetFileName(OutputPath)}";
                IsStatusError = false;
                IsStatusVisible = true;
            }
            catch (OperationCanceledException)
            {
                State = CompressState.FilesSelected;
                IsStatusVisible = false;
            }
            catch (CompressionException ex)
            {
                State = CompressState.Error;
                StatusMessage = $"壓縮失敗：{ex.Message}";
                IsStatusError = true;
                IsStatusVisible = true;
            }
            catch (Exception ex)
            {
                State = CompressState.Error;
                StatusMessage = $"未預期的錯誤：{ex.Message}";
                IsStatusError = true;
                IsStatusVisible = true;
            }
            finally
            {
                _cts?.Dispose();
                _cts = null;
            }
        }

        [RelayCommand]
        private void Reset()
        {
            _cts?.Cancel();
            SelectedFiles.Clear();
            OutputPath = string.Empty;
            CompressProgress = 0;
            CurrentFile = string.Empty;
            IsStatusVisible = false;
            State = CompressState.Idle;
        }
    }
}
