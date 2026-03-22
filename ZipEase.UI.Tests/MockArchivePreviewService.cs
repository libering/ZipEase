using System.Collections.Generic;
using System.IO;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

/// <summary>
/// Test double for ArchivePreviewService — returns configurable entry lists
/// without touching the Rust FFI layer.
/// </summary>
internal class MockArchivePreviewService : ArchivePreviewService
{
    private readonly List<ArchiveEntry> _entries;
    private readonly ListResult _result;

    public MockArchivePreviewService(
        List<ArchiveEntry>? entries = null,
        ListResult result = ListResult.Success)
    {
        _entries = entries ?? new List<ArchiveEntry>();
        _result = result;
    }

    public new (ListResult result, List<ArchiveEntry> entries, string? errorMessage)
        ListArchiveContentsWithPassword(string archivePath, string? password)
        => (_result, _entries, null);

    public new bool IsSupportedArchive(string filePath)
    {
        if (string.IsNullOrEmpty(filePath)) return false;
        var ext = Path.GetExtension(filePath).ToLowerInvariant();
        return ext is ".zip" or ".rar" or ".7z" or ".tar" or ".gz";
    }
}
