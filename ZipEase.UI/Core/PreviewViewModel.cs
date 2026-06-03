using System;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Input;
using System.Windows.Media.Imaging;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using WpfApplication = System.Windows.Application;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Preview state machine: Idle → Loading → Displaying or Error.
    /// </summary>
    public enum PreviewState { Idle, Loading, Displaying, Error }

    /// <summary>
    /// ViewModel for the image preview panel.
    /// Manages preview state, image loading with cancellation/timeout,
    /// zoom/pan via <see cref="ZoomPanService"/>, and navigation via <see cref="NavigationService"/>.
    /// Contains zero business logic — only P/Invoke calls (via <see cref="ImagePreviewNative"/>)
    /// and UI state management.
    /// </summary>
    public partial class PreviewViewModel : ObservableObject, IPreviewPanelCommands, IPreviewPanelZoom
    {
        // ─── Services ─────────────────────────────────────────────────────

        private readonly ZoomPanService _zoomPan = new();
        private readonly NavigationService _navigation = new();

        // ─── Cancellation ─────────────────────────────────────────────────

        private CancellationTokenSource? _cts;

        /// <summary>End-to-end timeout for image load operations (30 seconds).</summary>
        private static readonly TimeSpan LoadTimeout = TimeSpan.FromSeconds(30);

        // ─── Events ───────────────────────────────────────────────────────

        /// <summary>
        /// Fired when the user requests to close the preview panel.
        /// The host (MainWindow) should hide the panel and return focus to the file list.
        /// </summary>
        public event EventHandler? Closed;

        // ─── Observable Properties ────────────────────────────────────────

        [ObservableProperty] private PreviewState _state = PreviewState.Idle;
        [ObservableProperty] private WriteableBitmap? _currentImage;
        [ObservableProperty] private string _errorMessage = string.Empty;
        [ObservableProperty] private bool _isLoading;
        [ObservableProperty] private bool _isError;

        // ─── Computed Properties (zoom/pan) ───────────────────────────────

        /// <summary>Current zoom level from ZoomPanService.</summary>
        public double ZoomLevel => _zoomPan.ZoomLevel;

        /// <summary>Zoom level as integer percentage for display (e.g., 100 = 100%).</summary>
        public int ZoomPercentage => (int)Math.Round(_zoomPan.ZoomLevel * 100);

        /// <summary>Horizontal pan offset from ZoomPanService.</summary>
        public double PanOffsetX => _zoomPan.PanOffset.X;

        /// <summary>Vertical pan offset from ZoomPanService.</summary>
        public double PanOffsetY => _zoomPan.PanOffset.Y;

        /// <summary>Whether drag-to-pan is currently enabled.</summary>
        public bool IsPanEnabled => _zoomPan.IsPanEnabled;

        // ─── Computed Properties (navigation) ─────────────────────────────

        /// <summary>True if there is a next image to navigate to.</summary>
        public bool CanGoNext => _navigation.CanGoNext;

        /// <summary>True if there is a previous image to navigate to.</summary>
        public bool CanGoPrevious => _navigation.CanGoPrevious;

        // ─── Constructor ──────────────────────────────────────────────────

        public PreviewViewModel()
        {
            _navigation.NavigationRequested += OnNavigationRequested;
        }

        // ─── Commands (IPreviewPanelCommands) ─────────────────────────────

        /// <summary>Closes the preview panel and cancels any in-progress operation.</summary>
        public ICommand CloseCommand => _closeCommand ??= new RelayCommand(ExecuteClose);
        private RelayCommand? _closeCommand;

        /// <summary>Navigates to the previous image in the directory.</summary>
        public ICommand PreviousCommand => _previousCommand ??= new RelayCommand(ExecutePrevious, () => CanGoPrevious);
        private RelayCommand? _previousCommand;

        /// <summary>Navigates to the next image in the directory.</summary>
        public ICommand NextCommand => _nextCommand ??= new RelayCommand(ExecuteNext, () => CanGoNext);
        private RelayCommand? _nextCommand;

        /// <summary>Zooms in by one step (10% of current zoom).</summary>
        public ICommand ZoomInCommand => _zoomInCommand ??= new RelayCommand(ExecuteZoomIn);
        private RelayCommand? _zoomInCommand;

        /// <summary>Zooms out by one step (10% of current zoom).</summary>
        public ICommand ZoomOutCommand => _zoomOutCommand ??= new RelayCommand(ExecuteZoomOut);
        private RelayCommand? _zoomOutCommand;

        /// <summary>Resets zoom to fit the image within the panel.</summary>
        public ICommand FitToWindowCommand => _fitToWindowCommand ??= new RelayCommand(ExecuteFitToWindow);
        private RelayCommand? _fitToWindowCommand;

        // ─── Public Methods ───────────────────────────────────────────────

        /// <summary>
        /// Loads and displays an image from the given file path.
        /// Cancels any previous in-progress operation. Applies a 30-second end-to-end timeout.
        /// </summary>
        /// <param name="filePath">Absolute path to the extracted image file on disk.</param>
        public async Task LoadImageAsync(string filePath)
        {
            // Cancel previous operation if still running
            CancelCurrentOperation();

            // Create new CTS with 30-second timeout
            _cts = new CancellationTokenSource(LoadTimeout);
            var token = _cts.Token;

            // Transition to Loading state
            SetLoadingState();

            try
            {
                // Call FFI decode on background thread, bitmap created on UI thread
                var bitmap = await ImagePreviewNative.DecodeImageAsync(filePath);

                // Check cancellation after async operation
                token.ThrowIfCancellationRequested();

                // Success — display the image
                await WpfApplication.Current.Dispatcher.InvokeAsync(() =>
                {
                    CurrentImage = bitmap;
                    SetDisplayingState();
                    ResetZoomForImage();
                });
            }
            catch (OperationCanceledException)
            {
                // Determine if this was a timeout or user-initiated cancel
                if (_cts != null && _cts.IsCancellationRequested)
                {
                    await WpfApplication.Current.Dispatcher.InvokeAsync(() =>
                    {
                        SetErrorState("圖片載入逾時，請稍後再試。");
                    });
                }
                // If user-initiated cancel (new image requested), state is already updated
            }
            catch (ImagePreviewException ex)
            {
                if (!token.IsCancellationRequested)
                {
                    await WpfApplication.Current.Dispatcher.InvokeAsync(() =>
                    {
                        SetErrorState(ex.Message);
                    });
                }
            }
            catch (Exception)
            {
                if (!token.IsCancellationRequested)
                {
                    await WpfApplication.Current.Dispatcher.InvokeAsync(() =>
                    {
                        SetErrorState("此檔案格式無法預覽。");
                    });
                }
            }
        }

        /// <summary>
        /// Initializes the navigation service with the list of previewable entries
        /// in the current directory and sets the current entry.
        /// </summary>
        /// <param name="entries">Sorted list of previewable entry names.</param>
        /// <param name="currentEntry">The entry currently being previewed.</param>
        public void InitializeNavigation(System.Collections.Generic.List<string> entries, string currentEntry)
        {
            _navigation.Initialize(entries, currentEntry);
            NotifyNavigationChanged();
        }

        /// <summary>
        /// Gets the underlying <see cref="NavigationService"/> for external access.
        /// </summary>
        public NavigationService Navigation => _navigation;

        // ─── IPreviewPanelZoom Implementation ─────────────────────────────

        /// <summary>
        /// Zooms at the specified cursor position. Called by PreviewPanel on mouse wheel.
        /// </summary>
        public void ZoomAtPoint(double delta, System.Windows.Point cursorPosition)
        {
            if (CurrentImage == null) return;

            var imageSize = new System.Windows.Size(CurrentImage.PixelWidth, CurrentImage.PixelHeight);
            var panelSize = GetPanelSize();

            _zoomPan.ZoomAtPoint(delta, cursorPosition, panelSize, imageSize);
            NotifyZoomPanChanged();
        }

        /// <summary>
        /// Applies a pan delta. Called by PreviewPanel on mouse drag.
        /// </summary>
        public void Pan(System.Windows.Vector delta)
        {
            _zoomPan.Pan(delta);
            NotifyZoomPanChanged();
        }

        // ─── Private Command Implementations ──────────────────────────────

        private void ExecuteClose()
        {
            CancelCurrentOperation();
            CurrentImage = null;
            State = PreviewState.Idle;
            IsLoading = false;
            IsError = false;
            ErrorMessage = string.Empty;
            _zoomPan.Reset();
            NotifyZoomPanChanged();
            Closed?.Invoke(this, EventArgs.Empty);
        }

        private void ExecutePrevious()
        {
            _navigation.GoPrevious();
        }

        private void ExecuteNext()
        {
            _navigation.GoNext();
        }

        private void ExecuteZoomIn()
        {
            if (CurrentImage == null) return;

            var imageSize = new System.Windows.Size(CurrentImage.PixelWidth, CurrentImage.PixelHeight);
            var panelSize = GetPanelSize();
            var center = new System.Windows.Point(panelSize.Width / 2, panelSize.Height / 2);

            _zoomPan.ZoomAtPoint(1.0, center, panelSize, imageSize);
            NotifyZoomPanChanged();
        }

        private void ExecuteZoomOut()
        {
            if (CurrentImage == null) return;

            var imageSize = new System.Windows.Size(CurrentImage.PixelWidth, CurrentImage.PixelHeight);
            var panelSize = GetPanelSize();
            var center = new System.Windows.Point(panelSize.Width / 2, panelSize.Height / 2);

            _zoomPan.ZoomAtPoint(-1.0, center, panelSize, imageSize);
            NotifyZoomPanChanged();
        }

        private void ExecuteFitToWindow()
        {
            if (CurrentImage == null) return;

            var imageSize = new System.Windows.Size(CurrentImage.PixelWidth, CurrentImage.PixelHeight);
            var panelSize = GetPanelSize();

            _zoomPan.FitToWindow(imageSize, panelSize);
            NotifyZoomPanChanged();
        }

        // ─── Private Helpers ──────────────────────────────────────────────

        /// <summary>
        /// Cancels the current in-progress operation and disposes the CTS.
        /// </summary>
        private void CancelCurrentOperation()
        {
            if (_cts != null)
            {
                _cts.Cancel();
                _cts.Dispose();
                _cts = null;
            }
        }

        /// <summary>
        /// Transitions to Loading state.
        /// </summary>
        private void SetLoadingState()
        {
            State = PreviewState.Loading;
            IsLoading = true;
            IsError = false;
            ErrorMessage = string.Empty;
            _navigation.IsLoading = true;
            NotifyNavigationChanged();
        }

        /// <summary>
        /// Transitions to Displaying state.
        /// </summary>
        private void SetDisplayingState()
        {
            State = PreviewState.Displaying;
            IsLoading = false;
            IsError = false;
            ErrorMessage = string.Empty;
            _navigation.IsLoading = false;
            NotifyNavigationChanged();
        }

        /// <summary>
        /// Transitions to Error state with a user-friendly message.
        /// </summary>
        private void SetErrorState(string message)
        {
            State = PreviewState.Error;
            IsLoading = false;
            IsError = true;
            ErrorMessage = message;
            CurrentImage = null;
            _navigation.IsLoading = false;
            NotifyNavigationChanged();
        }

        /// <summary>
        /// Resets zoom to fit-to-window for the newly loaded image.
        /// </summary>
        private void ResetZoomForImage()
        {
            if (CurrentImage == null) return;

            var imageSize = new System.Windows.Size(CurrentImage.PixelWidth, CurrentImage.PixelHeight);
            var panelSize = GetPanelSize();

            _zoomPan.FitToWindow(imageSize, panelSize);
            NotifyZoomPanChanged();
        }

        /// <summary>
        /// Notifies the UI that zoom/pan properties have changed.
        /// </summary>
        private void NotifyZoomPanChanged()
        {
            OnPropertyChanged(nameof(ZoomLevel));
            OnPropertyChanged(nameof(ZoomPercentage));
            OnPropertyChanged(nameof(PanOffsetX));
            OnPropertyChanged(nameof(PanOffsetY));
            OnPropertyChanged(nameof(IsPanEnabled));
        }

        /// <summary>
        /// Notifies the UI that navigation state has changed.
        /// </summary>
        private void NotifyNavigationChanged()
        {
            OnPropertyChanged(nameof(CanGoNext));
            OnPropertyChanged(nameof(CanGoPrevious));
            _previousCommand?.NotifyCanExecuteChanged();
            _nextCommand?.NotifyCanExecuteChanged();
        }

        /// <summary>
        /// Handles navigation requests from the NavigationService.
        /// Loads the new image when the user navigates.
        /// </summary>
        private async void OnNavigationRequested(object? sender, string entryPath)
        {
            await LoadImageAsync(entryPath);
        }

        /// <summary>
        /// Gets the current panel size for zoom calculations.
        /// Returns a reasonable default if the panel is not yet measured.
        /// </summary>
        private static System.Windows.Size GetPanelSize()
        {
            // Default panel size — the actual panel size should be provided
            // by the view via a binding or method call in a real scenario.
            // For now, use a reasonable default that works for calculations.
            return new System.Windows.Size(800, 600);
        }
    }
}
