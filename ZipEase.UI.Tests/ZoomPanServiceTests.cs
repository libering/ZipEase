using System.Windows;
using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

// ═══════════════════════════════════════════════════════════════════════════════
// Property 4: Fit-to-window preserves aspect ratio
// ═══════════════════════════════════════════════════════════════════════════════

/// <summary>
/// Property-based tests for <see cref="ZoomPanService.ComputeFitZoom"/>.
/// Verifies that fit-to-window zoom preserves aspect ratio, doesn't upscale,
/// and keeps the image within panel bounds.
/// </summary>
public class FitToWindowPropertyTests
{
    // ── image-preview-plugin Property 4: Fit-to-window preserves aspect ratio ──
    // **Validates: Requirements 2.6, 4.5**
    //
    // For any image dimensions (w, h) where w > 0 and h > 0, and any panel
    // dimensions (pw, ph) where pw > 0 and ph > 0, the computed display dimensions
    // satisfy: (1) display_width ≤ pw and display_height ≤ ph, (2) display_width /
    // display_height ≈ w / h (within floating-point tolerance), and (3) if w ≤ pw
    // and h ≤ ph then display_width = w and display_height = h.

    [Property(MaxTest = 200)]
    public Property Prop_FitToWindow_PreservesAspectRatio()
    {
        // Generate positive image and panel dimensions as tuples (1 to 10000).
        var dimPairGen = Gen.Choose(1, 10000).SelectMany(w =>
            Gen.Choose(1, 10000).Select(h => (W: (double)w, H: (double)h)));

        return Prop.ForAll(
            dimPairGen.ToArbitrary(),
            dimPairGen.ToArbitrary(),
            (image, panel) =>
            {
                var imageSize = new Size(image.W, image.H);
                var panelSize = new Size(panel.W, panel.H);

                double fitZoom = ZoomPanService.ComputeFitZoom(imageSize, panelSize);

                double displayW = image.W * fitZoom;
                double displayH = image.H * fitZoom;

                // (1) Display dimensions fit within panel
                bool fitsInPanel = displayW <= panel.W + 1e-9 && displayH <= panel.H + 1e-9;

                // (2) Aspect ratio preserved: displayW/displayH ≈ imageW/imageH
                double originalRatio = image.W / image.H;
                double displayRatio = displayW / displayH;
                bool aspectPreserved = Math.Abs(originalRatio - displayRatio) < 1e-9;

                // (3) No upscaling: if image fits in panel, zoom = 1.0
                bool noUpscale = true;
                if (image.W <= panel.W && image.H <= panel.H)
                {
                    noUpscale = Math.Abs(fitZoom - 1.0) < 1e-9;
                }

                // (4) Zoom is always <= 1.0
                bool zoomNotAboveOne = fitZoom <= 1.0 + 1e-9;

                return (fitsInPanel && aspectPreserved && noUpscale && zoomNotAboveOne)
                    .Label($"image=({image.W}x{image.H}), panel=({panel.W}x{panel.H}), " +
                           $"fitZoom={fitZoom:F6}, display=({displayW:F2}x{displayH:F2}), " +
                           $"fitsInPanel={fitsInPanel}, aspectPreserved={aspectPreserved}, " +
                           $"noUpscale={noUpscale}, zoomNotAboveOne={zoomNotAboveOne}");
            });
    }

    [Property(MaxTest = 100)]
    public Property Prop_FitToWindow_InvalidDimensions_ReturnOne()
    {
        // Generate dimensions where at least one is zero.
        var zeroGen = Gen.Constant(0.0);
        var posGen = Gen.Choose(1, 1000).Select(i => (double)i);

        return Prop.ForAll(
            zeroGen.ToArbitrary(),
            posGen.ToArbitrary(),
            (badDim, goodDim) =>
            {
                // Test with bad image width
                double r1 = ZoomPanService.ComputeFitZoom(new Size(badDim, goodDim), new Size(goodDim, goodDim));
                // Test with bad image height
                double r2 = ZoomPanService.ComputeFitZoom(new Size(goodDim, badDim), new Size(goodDim, goodDim));
                // Test with bad panel width
                double r3 = ZoomPanService.ComputeFitZoom(new Size(goodDim, goodDim), new Size(badDim, goodDim));
                // Test with bad panel height
                double r4 = ZoomPanService.ComputeFitZoom(new Size(goodDim, goodDim), new Size(goodDim, badDim));

                bool allReturnOne = Math.Abs(r1 - 1.0) < 1e-9
                    && Math.Abs(r2 - 1.0) < 1e-9
                    && Math.Abs(r3 - 1.0) < 1e-9
                    && Math.Abs(r4 - 1.0) < 1e-9;

                return allReturnOne
                    .Label($"Invalid dimensions should return 1.0: r1={r1}, r2={r2}, r3={r3}, r4={r4}");
            });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Property 5: Zoom clamping invariant
// ═══════════════════════════════════════════════════════════════════════════════

/// <summary>
/// Property-based tests for zoom clamping in <see cref="ZoomPanService"/>.
/// Verifies that zoom level always stays within [0.10, 5.00] regardless of
/// how many zoom operations are applied.
/// </summary>
public class ZoomClampingPropertyTests
{
    // ── image-preview-plugin Property 5: Zoom clamping invariant ────────────
    // **Validates: Requirements 4.2, 4.3, 4.4**
    //
    // For any current zoom level in [0.10, 5.00] and any sequence of zoom-in or
    // zoom-out operations, the resulting zoom level is always within [0.10, 5.00].

    [Property(MaxTest = 200)]
    public Property Prop_ZoomClamping_AlwaysWithinBounds()
    {
        // Generate a sequence of zoom operations (positive = zoom in, negative = zoom out).
        var deltaGen = Gen.Elements(1.0, -1.0);
        var opsGen = Gen.ListOf(Gen.Choose(1, 50).SelectMany(count =>
            Gen.ArrayOf(count, deltaGen))).Select(arr => arr.SelectMany(a => a).ToArray());

        // Generate panel and image sizes.
        var sizeGen = Gen.Choose(100, 2000).Select(i => (double)i);

        return Prop.ForAll(
            opsGen.ToArbitrary(),
            sizeGen.ToArbitrary(),
            sizeGen.ToArbitrary(),
            (ops, panelDim, imageDim) =>
            {
                var service = new ZoomPanService();
                var panelSize = new Size(panelDim, panelDim);
                var imageSize = new Size(imageDim, imageDim);
                var cursor = new Point(panelDim / 2, panelDim / 2);

                bool allWithinBounds = true;
                double violatingZoom = 0;

                foreach (var delta in ops)
                {
                    service.ZoomAtPoint(delta, cursor, panelSize, imageSize);

                    if (service.ZoomLevel < ZoomPanService.MinZoom - 1e-9 ||
                        service.ZoomLevel > ZoomPanService.MaxZoom + 1e-9)
                    {
                        allWithinBounds = false;
                        violatingZoom = service.ZoomLevel;
                        break;
                    }
                }

                return allWithinBounds
                    .Label($"After {ops.Length} ops, zoom={service.ZoomLevel:F4}. " +
                           $"Violation at: {violatingZoom:F4}. " +
                           $"Bounds: [{ZoomPanService.MinZoom}, {ZoomPanService.MaxZoom}]");
            });
    }

    [Property(MaxTest = 100)]
    public Property Prop_ZoomClamping_ManyZoomIns_NeverExceedsMax()
    {
        // Apply many consecutive zoom-in operations.
        var countGen = Gen.Choose(10, 200);

        return Prop.ForAll(
            countGen.ToArbitrary(),
            count =>
            {
                var service = new ZoomPanService();
                var panelSize = new Size(800, 600);
                var imageSize = new Size(1000, 800);
                var cursor = new Point(400, 300);

                for (int i = 0; i < count; i++)
                {
                    service.ZoomAtPoint(1.0, cursor, panelSize, imageSize);
                }

                return (service.ZoomLevel <= ZoomPanService.MaxZoom + 1e-9)
                    .Label($"After {count} zoom-ins, zoom={service.ZoomLevel:F4}, max={ZoomPanService.MaxZoom}");
            });
    }

    [Property(MaxTest = 100)]
    public Property Prop_ZoomClamping_ManyZoomOuts_NeverBelowMin()
    {
        // Apply many consecutive zoom-out operations.
        var countGen = Gen.Choose(10, 200);

        return Prop.ForAll(
            countGen.ToArbitrary(),
            count =>
            {
                var service = new ZoomPanService();
                var panelSize = new Size(800, 600);
                var imageSize = new Size(1000, 800);
                var cursor = new Point(400, 300);

                for (int i = 0; i < count; i++)
                {
                    service.ZoomAtPoint(-1.0, cursor, panelSize, imageSize);
                }

                return (service.ZoomLevel >= ZoomPanService.MinZoom - 1e-9)
                    .Label($"After {count} zoom-outs, zoom={service.ZoomLevel:F4}, min={ZoomPanService.MinZoom}");
            });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Property 6: Pan bounds invariant
// ═══════════════════════════════════════════════════════════════════════════════

/// <summary>
/// Property-based tests for pan behavior in <see cref="ZoomPanService"/>.
/// Verifies that pan is disabled when image fits in panel, and enabled when
/// zoomed image exceeds panel.
/// </summary>
public class PanBoundsPropertyTests
{
    // ── image-preview-plugin Property 6: Pan bounds invariant ───────────────
    // **Validates: Requirements 4.6, 4.7**
    //
    // For any image display size, panel size, and drag delta, if the image display
    // size exceeds the panel in at least one dimension, pan is enabled. If the image
    // fits entirely within the panel, the pan offset remains (0, 0).

    [Property(MaxTest = 200)]
    public Property Prop_PanDisabled_WhenImageFitsInPanel()
    {
        // Generate panel sizes and image sizes as tuples.
        var panelGen = Gen.Choose(200, 2000).SelectMany(w =>
            Gen.Choose(200, 2000).Select(h => (W: (double)w, H: (double)h)));
        var imageGen = Gen.Choose(10, 200).SelectMany(w =>
            Gen.Choose(10, 200).Select(h => (W: (double)w, H: (double)h)));

        return Prop.ForAll(
            panelGen.ToArbitrary(),
            imageGen.ToArbitrary(),
            (panel, image) =>
            {
                // Ensure image fits: use small image relative to panel so at zoom=1.0 it fits.
                var service = new ZoomPanService();
                var imageSize = new Size(Math.Min(image.W, panel.W), Math.Min(image.H, panel.H));
                var panelSize = new Size(panel.W, panel.H);

                // FitToWindow should set zoom such that image fits
                service.FitToWindow(imageSize, panelSize);

                // At fit zoom, image should fit in panel → pan disabled
                bool panDisabled = !service.IsPanEnabled;

                // Try to pan — should have no effect
                var offsetBefore = service.PanOffset;
                service.Pan(new Vector(100, 100));
                var offsetAfter = service.PanOffset;

                bool panHadNoEffect = Math.Abs(offsetAfter.X - offsetBefore.X) < 1e-9
                    && Math.Abs(offsetAfter.Y - offsetBefore.Y) < 1e-9;

                return (panDisabled && panHadNoEffect)
                    .Label($"image=({imageSize.Width}x{imageSize.Height}), panel=({panel.W}x{panel.H}), " +
                           $"zoom={service.ZoomLevel:F4}, panEnabled={service.IsPanEnabled}, " +
                           $"offsetBefore=({offsetBefore.X},{offsetBefore.Y}), " +
                           $"offsetAfter=({offsetAfter.X},{offsetAfter.Y})");
            });
    }

    [Property(MaxTest = 200)]
    public Property Prop_PanEnabled_WhenZoomedImageExceedsPanel()
    {
        // Generate scenarios where zoomed image exceeds panel.
        var panelGen = Gen.Choose(100, 500).SelectMany(w =>
            Gen.Choose(100, 500).Select(h => (W: (double)w, H: (double)h)));
        var imageGen = Gen.Choose(200, 2000).SelectMany(w =>
            Gen.Choose(200, 2000).Select(h => (W: (double)w, H: (double)h)));

        return Prop.ForAll(
            panelGen.ToArbitrary(),
            imageGen.ToArbitrary(),
            (panel, image) =>
            {
                var service = new ZoomPanService();
                var imageSize = new Size(image.W, image.H);
                var panelSize = new Size(panel.W, panel.H);
                var cursor = new Point(panel.W / 2, panel.H / 2);

                // Zoom in until image exceeds panel
                for (int i = 0; i < 50; i++)
                {
                    service.ZoomAtPoint(1.0, cursor, panelSize, imageSize);
                }

                // Check if zoomed image exceeds panel
                double displayW = image.W * service.ZoomLevel;
                double displayH = image.H * service.ZoomLevel;
                bool exceedsPanel = displayW > panel.W || displayH > panel.H;

                if (!exceedsPanel)
                {
                    // If image still doesn't exceed panel after zooming, skip
                    return true.Label("Image doesn't exceed panel even after zoom-in (small image, large panel)");
                }

                bool panEnabled = service.IsPanEnabled;

                // Pan should move the offset
                var offsetBefore = service.PanOffset;
                service.Pan(new Vector(10, 10));
                var offsetAfter = service.PanOffset;

                bool panMoved = Math.Abs(offsetAfter.X - offsetBefore.X) > 1e-9
                    || Math.Abs(offsetAfter.Y - offsetBefore.Y) > 1e-9;

                return (panEnabled && panMoved)
                    .Label($"image=({image.W}x{image.H}), panel=({panel.W}x{panel.H}), " +
                           $"zoom={service.ZoomLevel:F4}, panEnabled={panEnabled}, panMoved={panMoved}");
            });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit Tests: ZoomPanService specific behaviors
// ═══════════════════════════════════════════════════════════════════════════════

/// <summary>
/// Unit tests for <see cref="ZoomPanService"/> covering specific examples and edge cases.
/// </summary>
public class ZoomPanServiceUnitTests
{
    // ── ZoomAtPoint: cursor position remains stable after zoom ──────────────

    [Fact]
    public void ZoomAtPoint_CursorPositionRemainsStable()
    {
        var service = new ZoomPanService();
        var panelSize = new Size(800, 600);
        var imageSize = new Size(1000, 800);
        var cursor = new Point(200, 150); // Off-center cursor

        double oldZoom = service.ZoomLevel;

        // Calculate image point under cursor before zoom
        double panelCenterX = panelSize.Width / 2.0;
        double panelCenterY = panelSize.Height / 2.0;
        double imagePointX = (cursor.X - panelCenterX - service.PanOffset.X) / oldZoom;
        double imagePointY = (cursor.Y - panelCenterY - service.PanOffset.Y) / oldZoom;

        // Zoom in
        service.ZoomAtPoint(1.0, cursor, panelSize, imageSize);

        double newZoom = service.ZoomLevel;

        // Calculate where the same image point maps to in panel coordinates after zoom
        double newPanelX = imagePointX * newZoom + panelCenterX + service.PanOffset.X;
        double newPanelY = imagePointY * newZoom + panelCenterY + service.PanOffset.Y;

        // The cursor position should map to the same image point
        Assert.True(Math.Abs(newPanelX - cursor.X) < 1e-6,
            $"Cursor X shifted: expected {cursor.X}, got {newPanelX}");
        Assert.True(Math.Abs(newPanelY - cursor.Y) < 1e-6,
            $"Cursor Y shifted: expected {cursor.Y}, got {newPanelY}");
    }

    [Fact]
    public void ZoomAtPoint_ZoomIn_IncreasesZoomLevel()
    {
        var service = new ZoomPanService();
        var panelSize = new Size(800, 600);
        var imageSize = new Size(1000, 800);
        var cursor = new Point(400, 300);

        double before = service.ZoomLevel;
        service.ZoomAtPoint(1.0, cursor, panelSize, imageSize);

        Assert.True(service.ZoomLevel > before);
    }

    [Fact]
    public void ZoomAtPoint_ZoomOut_DecreasesZoomLevel()
    {
        var service = new ZoomPanService();
        var panelSize = new Size(800, 600);
        var imageSize = new Size(1000, 800);
        var cursor = new Point(400, 300);

        double before = service.ZoomLevel;
        service.ZoomAtPoint(-1.0, cursor, panelSize, imageSize);

        Assert.True(service.ZoomLevel < before);
    }

    [Fact]
    public void ZoomAtPoint_AtMaxZoom_DoesNotExceed()
    {
        var service = new ZoomPanService();
        var panelSize = new Size(800, 600);
        var imageSize = new Size(1000, 800);
        var cursor = new Point(400, 300);

        // Zoom in many times to reach max
        for (int i = 0; i < 200; i++)
        {
            service.ZoomAtPoint(1.0, cursor, panelSize, imageSize);
        }

        Assert.True(service.ZoomLevel <= ZoomPanService.MaxZoom + 1e-9);
        Assert.True(Math.Abs(service.ZoomLevel - ZoomPanService.MaxZoom) < 1e-9);
    }

    [Fact]
    public void ZoomAtPoint_AtMinZoom_DoesNotGoBelowMin()
    {
        var service = new ZoomPanService();
        var panelSize = new Size(800, 600);
        var imageSize = new Size(1000, 800);
        var cursor = new Point(400, 300);

        // Zoom out many times to reach min
        for (int i = 0; i < 200; i++)
        {
            service.ZoomAtPoint(-1.0, cursor, panelSize, imageSize);
        }

        Assert.True(service.ZoomLevel >= ZoomPanService.MinZoom - 1e-9);
        Assert.True(Math.Abs(service.ZoomLevel - ZoomPanService.MinZoom) < 1e-9);
    }

    // ── FitToWindow: resets zoom and centers image ─────────────────────────

    [Fact]
    public void FitToWindow_LargeImage_ScalesDown()
    {
        var service = new ZoomPanService();
        var imageSize = new Size(2000, 1500);
        var panelSize = new Size(800, 600);

        service.FitToWindow(imageSize, panelSize);

        // Should scale down: min(800/2000, 600/1500) = min(0.4, 0.4) = 0.4
        Assert.True(Math.Abs(service.ZoomLevel - 0.4) < 1e-9);
        Assert.Equal(0, service.PanOffset.X);
        Assert.Equal(0, service.PanOffset.Y);
    }

    [Fact]
    public void FitToWindow_SmallImage_NoUpscale()
    {
        var service = new ZoomPanService();
        var imageSize = new Size(200, 150);
        var panelSize = new Size(800, 600);

        service.FitToWindow(imageSize, panelSize);

        // Image fits in panel → zoom = 1.0 (no upscaling)
        Assert.True(Math.Abs(service.ZoomLevel - 1.0) < 1e-9);
        Assert.Equal(0, service.PanOffset.X);
        Assert.Equal(0, service.PanOffset.Y);
    }

    [Fact]
    public void FitToWindow_WideImage_FitsByWidth()
    {
        var service = new ZoomPanService();
        var imageSize = new Size(1600, 400); // Wide image
        var panelSize = new Size(800, 600);

        service.FitToWindow(imageSize, panelSize);

        // min(800/1600, 600/400) = min(0.5, 1.5) = 0.5
        Assert.True(Math.Abs(service.ZoomLevel - 0.5) < 1e-9);
    }

    [Fact]
    public void FitToWindow_TallImage_FitsByHeight()
    {
        var service = new ZoomPanService();
        var imageSize = new Size(400, 1200); // Tall image
        var panelSize = new Size(800, 600);

        service.FitToWindow(imageSize, panelSize);

        // min(800/400, 600/1200) = min(2.0, 0.5) = 0.5
        Assert.True(Math.Abs(service.ZoomLevel - 0.5) < 1e-9);
    }

    [Fact]
    public void FitToWindow_ResetsAfterZoom()
    {
        var service = new ZoomPanService();
        var panelSize = new Size(800, 600);
        var imageSize = new Size(1000, 800);
        var cursor = new Point(400, 300);

        // Zoom in first
        for (int i = 0; i < 10; i++)
        {
            service.ZoomAtPoint(1.0, cursor, panelSize, imageSize);
        }

        Assert.True(service.ZoomLevel > 1.0);

        // FitToWindow should reset
        service.FitToWindow(imageSize, panelSize);

        double expectedFit = Math.Min(Math.Min(800.0 / 1000.0, 600.0 / 800.0), 1.0);
        Assert.True(Math.Abs(service.ZoomLevel - expectedFit) < 1e-9);
        Assert.Equal(0, service.PanOffset.X);
        Assert.Equal(0, service.PanOffset.Y);
    }

    // ── Pan: disabled when image fits, enabled when exceeds ────────────────

    [Fact]
    public void Pan_Disabled_WhenImageFitsInPanel()
    {
        var service = new ZoomPanService();
        var imageSize = new Size(400, 300);
        var panelSize = new Size(800, 600);

        service.FitToWindow(imageSize, panelSize);

        // Image fits → pan disabled
        Assert.False(service.IsPanEnabled);

        // Pan should have no effect
        service.Pan(new Vector(50, 50));
        Assert.Equal(0, service.PanOffset.X);
        Assert.Equal(0, service.PanOffset.Y);
    }

    [Fact]
    public void Pan_Enabled_WhenZoomedImageExceedsPanel()
    {
        var service = new ZoomPanService();
        var panelSize = new Size(800, 600);
        var imageSize = new Size(1000, 800);
        var cursor = new Point(400, 300);

        // Zoom in until image exceeds panel
        for (int i = 0; i < 20; i++)
        {
            service.ZoomAtPoint(1.0, cursor, panelSize, imageSize);
        }

        // Zoomed image should exceed panel
        Assert.True(service.IsPanEnabled);

        // Pan should move offset
        var before = service.PanOffset;
        service.Pan(new Vector(30, 20));
        Assert.True(Math.Abs(service.PanOffset.X - (before.X + 30)) < 1e-9);
        Assert.True(Math.Abs(service.PanOffset.Y - (before.Y + 20)) < 1e-9);
    }

    // ── Reset: returns to default state ────────────────────────────────────

    [Fact]
    public void Reset_ReturnsToDefaultState()
    {
        var service = new ZoomPanService();
        var panelSize = new Size(800, 600);
        var imageSize = new Size(1000, 800);
        var cursor = new Point(400, 300);

        // Modify state
        for (int i = 0; i < 10; i++)
        {
            service.ZoomAtPoint(1.0, cursor, panelSize, imageSize);
        }
        service.Pan(new Vector(50, 50));

        // Reset
        service.Reset();

        Assert.True(Math.Abs(service.ZoomLevel - 1.0) < 1e-9);
        Assert.Equal(0, service.PanOffset.X);
        Assert.Equal(0, service.PanOffset.Y);
        Assert.False(service.IsPanEnabled);
    }

    [Fact]
    public void Reset_AfterFitToWindow_ReturnsToDefault()
    {
        var service = new ZoomPanService();
        var imageSize = new Size(2000, 1500);
        var panelSize = new Size(800, 600);

        service.FitToWindow(imageSize, panelSize);
        Assert.True(service.ZoomLevel < 1.0); // Was scaled down

        service.Reset();

        Assert.True(Math.Abs(service.ZoomLevel - 1.0) < 1e-9);
        Assert.Equal(0, service.PanOffset.X);
        Assert.Equal(0, service.PanOffset.Y);
        Assert.False(service.IsPanEnabled);
    }
}
