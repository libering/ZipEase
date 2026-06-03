using System.Windows;
using System.Windows.Input;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Preview panel for displaying images from archive entries.
    /// Pure UI — all business logic lives in the bound ViewModel.
    /// Handles keyboard events (arrows, Escape), mouse wheel (zoom),
    /// and mouse drag (pan) by routing to ViewModel commands/properties.
    /// </summary>
    public partial class PreviewPanel : System.Windows.Controls.UserControl
    {
        private bool _isDragging;
        private System.Windows.Point _lastMousePosition;

        public PreviewPanel()
        {
            InitializeComponent();
            Loaded += OnLoaded;
        }

        private void OnLoaded(object sender, RoutedEventArgs e)
        {
            // Grab keyboard focus so arrow keys and Escape work immediately
            Focus();
        }

        /// <summary>
        /// Routes keyboard events to ViewModel commands.
        /// Left/Right arrows → navigation, Escape → close.
        /// </summary>
        private void OnKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
        {
            if (DataContext is not IPreviewPanelCommands commands)
                return;

            switch (e.Key)
            {
                case Key.Left:
                    if (commands.PreviousCommand.CanExecute(null))
                        commands.PreviousCommand.Execute(null);
                    e.Handled = true;
                    break;

                case Key.Right:
                    if (commands.NextCommand.CanExecute(null))
                        commands.NextCommand.Execute(null);
                    e.Handled = true;
                    break;

                case Key.Escape:
                    if (commands.CloseCommand.CanExecute(null))
                        commands.CloseCommand.Execute(null);
                    e.Handled = true;
                    break;
            }
        }

        /// <summary>
        /// Routes mouse wheel to ViewModel for zoom (10% increments centered on cursor).
        /// </summary>
        private void OnMouseWheel(object sender, MouseWheelEventArgs e)
        {
            if (DataContext is not IPreviewPanelZoom zoom)
                return;

            var cursorPosition = e.GetPosition(PreviewImage);
            double delta = e.Delta > 0 ? 0.10 : -0.10;

            zoom.ZoomAtPoint(delta, cursorPosition);
            e.Handled = true;
        }

        /// <summary>
        /// Begins drag-to-pan when left mouse button is pressed on the image area.
        /// </summary>
        private void OnImageAreaMouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.LeftButton != MouseButtonState.Pressed)
                return;

            if (DataContext is not IPreviewPanelZoom zoom || !zoom.IsPanEnabled)
                return;

            _isDragging = true;
            _lastMousePosition = e.GetPosition(this);
            ((UIElement)sender).CaptureMouse();
            e.Handled = true;
        }

        /// <summary>
        /// Ends drag-to-pan when left mouse button is released.
        /// </summary>
        private void OnImageAreaMouseUp(object sender, MouseButtonEventArgs e)
        {
            if (!_isDragging)
                return;

            _isDragging = false;
            ((UIElement)sender).ReleaseMouseCapture();
            e.Handled = true;
        }

        /// <summary>
        /// Routes mouse move delta to ViewModel for pan offset calculation.
        /// </summary>
        private void OnImageAreaMouseMove(object sender, System.Windows.Input.MouseEventArgs e)
        {
            if (!_isDragging)
                return;

            if (DataContext is not IPreviewPanelZoom zoom)
                return;

            var currentPosition = e.GetPosition(this);
            var delta = currentPosition - _lastMousePosition;
            _lastMousePosition = currentPosition;

            zoom.Pan(delta);
            e.Handled = true;
        }
    }

    /// <summary>
    /// Interface for ViewModel commands that the PreviewPanel routes keyboard events to.
    /// Keeps code-behind decoupled from concrete ViewModel type.
    /// </summary>
    public interface IPreviewPanelCommands
    {
        ICommand PreviousCommand { get; }
        ICommand NextCommand { get; }
        ICommand CloseCommand { get; }
    }

    /// <summary>
    /// Interface for ViewModel zoom/pan operations that the PreviewPanel routes mouse events to.
    /// Keeps code-behind decoupled from concrete ViewModel type.
    /// </summary>
    public interface IPreviewPanelZoom
    {
        bool IsPanEnabled { get; }
        void ZoomAtPoint(double delta, System.Windows.Point cursorPosition);
        void Pan(System.Windows.Vector delta);
    }
}
