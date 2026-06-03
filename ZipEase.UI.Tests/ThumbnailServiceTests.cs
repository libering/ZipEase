using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

/// <summary>
/// Unit tests for <see cref="ThumbnailService"/>.
/// Tests the IsPreviewable extension classification and size guard logic.
/// FFI-dependent methods (GetThumbnailAsync) are not tested here as they require
/// the Rust backend — those are covered by integration tests.
/// </summary>
public class ThumbnailServiceTests
{
    // ─── IsPreviewable: Supported Extensions ──────────────────────────────

    [Theory]
    [InlineData("photo.png", false, true)]
    [InlineData("photo.jpg", false, true)]
    [InlineData("photo.jpeg", false, true)]
    [InlineData("photo.gif", false, true)]
    [InlineData("photo.bmp", false, true)]
    [InlineData("photo.webp", false, true)]
    [InlineData("photo.tiff", false, true)]
    [InlineData("photo.tif", false, true)]
    [InlineData("photo.ico", false, true)]
    public void IsPreviewable_SupportedExtensions_ReturnsTrue(string name, bool isDir, bool expected)
    {
        Assert.Equal(expected, ThumbnailService.IsPreviewable(name, isDir));
    }

    // ─── IsPreviewable: Case Insensitivity ────────────────────────────────

    [Theory]
    [InlineData("PHOTO.PNG")]
    [InlineData("Photo.Jpg")]
    [InlineData("image.WEBP")]
    [InlineData("file.TiFF")]
    public void IsPreviewable_CaseInsensitive_ReturnsTrue(string name)
    {
        Assert.True(ThumbnailService.IsPreviewable(name));
    }

    // ─── IsPreviewable: Non-Image Extensions ──────────────────────────────

    [Theory]
    [InlineData("document.pdf")]
    [InlineData("archive.zip")]
    [InlineData("readme.txt")]
    [InlineData("script.py")]
    [InlineData("video.mp4")]
    [InlineData("music.mp3")]
    public void IsPreviewable_NonImageExtensions_ReturnsFalse(string name)
    {
        Assert.False(ThumbnailService.IsPreviewable(name));
    }

    // ─── IsPreviewable: Directories ───────────────────────────────────────

    [Theory]
    [InlineData("images/", true)]
    [InlineData("photo.png/", true)]
    [InlineData("folder", true)]
    public void IsPreviewable_Directories_ReturnsFalse(string name, bool isDir)
    {
        Assert.False(ThumbnailService.IsPreviewable(name, isDir));
    }

    // ─── IsPreviewable: No Dot in Name ────────────────────────────────────

    [Theory]
    [InlineData("filename")]
    [InlineData("nodot")]
    public void IsPreviewable_NoDot_ReturnsFalse(string name)
    {
        Assert.False(ThumbnailService.IsPreviewable(name));
    }

    // ─── IsPreviewable: Dot at End ────────────────────────────────────────

    [Fact]
    public void IsPreviewable_DotAtEnd_ReturnsFalse()
    {
        Assert.False(ThumbnailService.IsPreviewable("file."));
    }

    // ─── IsPreviewable: Path with Separators ──────────────────────────────

    [Theory]
    [InlineData("folder/subfolder/image.png", true)]
    [InlineData("deep/path/to/photo.jpg", true)]
    [InlineData("folder\\image.webp", true)]
    public void IsPreviewable_WithPathSeparators_ExtractsFileName(string name, bool expected)
    {
        Assert.Equal(expected, ThumbnailService.IsPreviewable(name));
    }

    // ─── IsPreviewable: Multiple Dots ─────────────────────────────────────

    [Theory]
    [InlineData("file.backup.png", true)]
    [InlineData("my.photo.2024.jpg", true)]
    [InlineData("archive.tar.gz", false)]  // .gz is not a supported image extension
    public void IsPreviewable_MultipleDots_UsesLastExtension(string name, bool expected)
    {
        Assert.Equal(expected, ThumbnailService.IsPreviewable(name));
    }

    // ─── IsPreviewable: Empty/Null ────────────────────────────────────────

    [Theory]
    [InlineData("")]
    [InlineData(null)]
    public void IsPreviewable_EmptyOrNull_ReturnsFalse(string? name)
    {
        Assert.False(ThumbnailService.IsPreviewable(name ?? string.Empty));
    }

    // ─── Size Guard: Skip Oversized Entries ───────────────────────────────

    [Fact]
    public async Task GetThumbnailAsync_ExceedsMaxSize_ReturnsNull()
    {
        using var service = new ThumbnailService();
        long oversized = 101L * 1024 * 1024; // 101 MB

        var result = await service.GetThumbnailAsync("dummy.png", oversized);

        Assert.Null(result);
    }

    [Fact]
    public async Task GetThumbnailAsync_ExactlyAtLimit_ReturnsNull()
    {
        using var service = new ThumbnailService();
        long exactLimit = 100L * 1024 * 1024 + 1; // 100 MB + 1 byte

        var result = await service.GetThumbnailAsync("dummy.png", exactLimit);

        Assert.Null(result);
    }

    // ─── CancelAndClear ───────────────────────────────────────────────────

    [Fact]
    public void CancelAndClear_DoesNotThrow()
    {
        using var service = new ThumbnailService();
        service.CancelAndClear();
        // Should not throw and should be reusable after clear
        service.CancelAndClear();
    }

    // ─── Dispose ──────────────────────────────────────────────────────────

    [Fact]
    public async Task GetThumbnailAsync_AfterDispose_ReturnsNull()
    {
        var service = new ThumbnailService();
        service.Dispose();

        var result = await service.GetThumbnailAsync("test.png", 1024);

        Assert.Null(result);
    }
}
