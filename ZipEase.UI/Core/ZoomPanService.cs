namespace ZipEase.UI.Core
{
    /// <summary>
    /// Pure calculation service for zoom and pan operations on the image preview panel.
    /// No UI dependencies — uses only System.Windows.Point, Size, and Vector for coordinates.
    /// </summary>
    internal class ZoomPanService
    {
        // ─── Constants ────────────────────────────────────────────────────

        /// <summary>Minimum allowed zoom level (10%).</summary>
        public const double MinZoom = 0.10;

        /// <summary>Maximum allowed zoom level (500%).</summary>
        public const double MaxZoom = 5.00;

        /// <summary>Zoom step as a fraction of current zoom (10%).</summary>
        private const double ZoomStepFraction = 0.10;

        // ─── Properties ───────────────────────────────────────────────────

        /// <summary>
        /// Current zoom level, always within [<see cref="MinZoom"/>, <see cref="MaxZoom"/>].
        /// A value of 1.0 means the image is displayed at its original size.
        /// </summary>
        public double ZoomLevel { get; private set; } = 1.0;

        /// <summary>
        /// Current pan offset in panel coordinates.
        /// Represents how far the image has been dragged from its centered position.
        /// </summary>
        public System.Windows.Point PanOffset { get; private set; } = new System.Windows.Point(0, 0);

        /// <summary>
        /// Whether panning is currently enabled.
        /// Pan is enabled only when the zoomed image exceeds the panel in at least one dimension.
        /// </summary>
        public bool IsPanEnabled { get; private set; }

        // ─── Public Methods ───────────────────────────────────────────────

        /// <summary>
        /// Computes the fit-to-window zoom level that displays the entire image
        /// within the panel while preserving aspect ratio.
        /// If the image is smaller than the panel in both dimensions, returns 1.0
        /// (no upscaling).
        /// </summary>
        /// <param name="imageSize">Original image dimensions (width, height).</param>
        /// <param name="panelSize">Available panel dimensions (width, height).</param>
        /// <returns>The zoom level that fits the image within the panel.</returns>
        public static double ComputeFitZoom(System.Windows.Size imageSize, System.Windows.Size panelSize)
        {
            if (imageSize.Width <= 0 || imageSize.Height <= 0 ||
                panelSize.Width <= 0 || panelSize.Height <= 0)
            {
                return 1.0;
            }

            double scaleX = panelSize.Width / imageSize.Width;
            double scaleY = panelSize.Height / imageSize.Height;

            // FitZoom = min(panelWidth / imageWidth, panelHeight / imageHeight, 1.0)
            return Math.Min(Math.Min(scaleX, scaleY), 1.0);
        }

        /// <summary>
        /// Adjusts the zoom level by a delta (positive = zoom in, negative = zoom out),
        /// keeping the point under the cursor stable in the viewport.
        /// The zoom step is 10% of the current zoom level.
        /// </summary>
        /// <param name="delta">Positive to zoom in, negative to zoom out.</param>
        /// <param name="cursorPosition">Cursor position in panel coordinates.</param>
        /// <param name="panelSize">Current panel dimensions.</param>
        /// <param name="imageSize">Original image dimensions.</param>
        public void ZoomAtPoint(double delta, System.Windows.Point cursorPosition, System.Windows.Size panelSize, System.Windows.Size imageSize)
        {
            double oldZoom = ZoomLevel;

            // Calculate zoom step as 10% of current zoom
            double step = oldZoom * ZoomStepFraction;

            // Apply delta direction
            double newZoom = delta > 0
                ? oldZoom + step
                : oldZoom - step;

            // Clamp to valid range
            newZoom = Clamp(newZoom, MinZoom, MaxZoom);

            // If zoom didn't change (already at boundary), nothing to do
            if (Math.Abs(newZoom - oldZoom) < 1e-10)
            {
                return;
            }

            // Keep the point under the cursor stable:
            // The cursor position relative to the image content should remain the same.
            //
            // Before zoom: imagePoint = (cursor - panelCenter - panOffset) / oldZoom
            // After zoom:  cursor - panelCenter - newPanOffset = imagePoint * newZoom
            // Therefore:   newPanOffset = cursor - panelCenter - imagePoint * newZoom

            double panelCenterX = panelSize.Width / 2.0;
            double panelCenterY = panelSize.Height / 2.0;

            // Image point under cursor (in image coordinates)
            double imagePointX = (cursorPosition.X - panelCenterX - PanOffset.X) / oldZoom;
            double imagePointY = (cursorPosition.Y - panelCenterY - PanOffset.Y) / oldZoom;

            // New pan offset to keep cursor stable
            double newPanX = cursorPosition.X - panelCenterX - imagePointX * newZoom;
            double newPanY = cursorPosition.Y - panelCenterY - imagePointY * newZoom;

            ZoomLevel = newZoom;
            PanOffset = new System.Windows.Point(newPanX, newPanY);

            UpdatePanEnabled(imageSize, panelSize);

            // If pan is no longer enabled, reset offset to center
            if (!IsPanEnabled)
            {
                PanOffset = new System.Windows.Point(0, 0);
            }
        }

        /// <summary>
        /// Applies a drag-to-pan offset delta. Only effective when <see cref="IsPanEnabled"/> is true
        /// (i.e., the zoomed image exceeds the panel in at least one dimension).
        /// </summary>
        /// <param name="delta">The drag delta vector (pixels moved).</param>
        public void Pan(System.Windows.Vector delta)
        {
            if (!IsPanEnabled)
            {
                return;
            }

            PanOffset = new System.Windows.Point(
                PanOffset.X + delta.X,
                PanOffset.Y + delta.Y);
        }

        /// <summary>
        /// Resets zoom to fit-to-window level and centers the image (pan offset = 0).
        /// </summary>
        /// <param name="imageSize">Original image dimensions.</param>
        /// <param name="panelSize">Available panel dimensions.</param>
        public void FitToWindow(System.Windows.Size imageSize, System.Windows.Size panelSize)
        {
            ZoomLevel = ComputeFitZoom(imageSize, panelSize);
            PanOffset = new System.Windows.Point(0, 0);
            UpdatePanEnabled(imageSize, panelSize);
        }

        /// <summary>
        /// Resets the service to its default state (zoom = 1.0, no pan).
        /// </summary>
        public void Reset()
        {
            ZoomLevel = 1.0;
            PanOffset = new System.Windows.Point(0, 0);
            IsPanEnabled = false;
        }

        /// <summary>
        /// Updates the <see cref="IsPanEnabled"/> flag based on current zoom and dimensions.
        /// Call this after any zoom change or panel resize.
        /// </summary>
        /// <param name="imageSize">Original image dimensions.</param>
        /// <param name="panelSize">Available panel dimensions.</param>
        public void UpdatePanEnabled(System.Windows.Size imageSize, System.Windows.Size panelSize)
        {
            if (imageSize.Width <= 0 || imageSize.Height <= 0 ||
                panelSize.Width <= 0 || panelSize.Height <= 0)
            {
                IsPanEnabled = false;
                return;
            }

            // PanEnabled = (imageWidth * zoom > panelWidth) || (imageHeight * zoom > panelHeight)
            double displayWidth = imageSize.Width * ZoomLevel;
            double displayHeight = imageSize.Height * ZoomLevel;

            IsPanEnabled = displayWidth > panelSize.Width || displayHeight > panelSize.Height;
        }

        // ─── Private Helpers ──────────────────────────────────────────────

        private static double Clamp(double value, double min, double max)
        {
            if (value < min) return min;
            if (value > max) return max;
            return value;
        }
    }
}
