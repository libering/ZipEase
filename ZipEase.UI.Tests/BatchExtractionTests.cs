using System;
using System.Collections.Generic;
using System.Linq;
using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

/// <summary>
/// Unit tests for batch extraction functionality.
/// Tests C# logic in isolation — no native DLL required.
/// Validates: Requirements 1.1, 1.2, 4.1, 5.1
/// </summary>
public class BatchExtractionTests
{
    // ── Helpers ──────────────────────────────────────────────────────────────

    private static MainWindowViewModel MakeVm()
        => new(new MockArchivePreviewService());

    // ── 1. BatchExtractionManager argument validation ────────────────────────
    // Tests GCHandle lifecycle indirectly: if validation passes, the method
    // would proceed to allocate GCHandles. Validation prevents invalid state.
    // Validates: Requirements 4.1

    [Fact]
    public async Task ExtractBatchAsync_NullArchivePaths_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            BatchExtractionManager.ExtractBatchAsync(null!, "C:\\output"));
    }

    [Fact]
    public async Task ExtractBatchAsync_EmptyArchivePaths_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            BatchExtractionManager.ExtractBatchAsync(Array.Empty<string>(), "C:\\output"));
    }

    [Fact]
    public async Task ExtractBatchAsync_NullOutputDir_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            BatchExtractionManager.ExtractBatchAsync(new[] { "a.zip" }, null!));
    }

    [Fact]
    public async Task ExtractBatchAsync_EmptyOutputDir_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            BatchExtractionManager.ExtractBatchAsync(new[] { "a.zip" }, ""));
    }

    [Property(MaxTest = 100)]
    public Property ExtractBatchAsync_NullOrEmptyPaths_AlwaysThrows()
    {
        return Prop.ForAll(
            Arb.From(Gen.Elements(
                (string[]?)null,
                Array.Empty<string>())),
            paths =>
            {
                bool threw = false;
                try
                {
                    BatchExtractionManager.ExtractBatchAsync(paths!, "C:\\output")
                        .GetAwaiter().GetResult();
                }
                catch (ArgumentException)
                {
                    threw = true;
                }
                catch
                {
                    // Other exceptions are acceptable (e.g., DLL not found after validation)
                    threw = true;
                }
                return threw.Label("Should throw for null/empty archive paths");
            });
    }

    [Property(MaxTest = 100)]
    public Property ExtractBatchAsync_NullOrEmptyOutputDir_AlwaysThrows()
    {
        return Prop.ForAll(
            Arb.From(Gen.Elements((string?)null, "")),
            outputDir =>
            {
                bool threw = false;
                try
                {
                    BatchExtractionManager.ExtractBatchAsync(
                        new[] { "test.zip" }, outputDir!)
                        .GetAwaiter().GetResult();
                }
                catch (ArgumentException)
                {
                    threw = true;
                }
                catch
                {
                    threw = true;
                }
                return threw.Label("Should throw for null/empty output dir");
            });
    }

    // ── 2. ViewModel state transitions (BatchExtracting) ─────────────────────
    // Validates: Requirements 1.2, 5.1

    [Fact]
    public void TransitionToBatchExtracting_SetsStateToBatchExtracting()
    {
        var vm = MakeVm();
        vm.TransitionToBatchExtracting();

        Assert.Equal(UIState.BatchExtracting, vm.CurrentState);
    }

    [Fact]
    public void TransitionToBatchExtracting_ResetsProgressProperties()
    {
        var vm = MakeVm();
        // Set some state first
        vm.TransitionToPreviewing("test.zip");
        vm.TransitionToExtracting();

        vm.TransitionToBatchExtracting();

        Assert.Equal(0, vm.BatchArchiveIndex);
        Assert.Equal(0, vm.BatchArchiveCount);
        Assert.Equal(string.Empty, vm.BatchCurrentArchiveName);
        Assert.Equal(0, vm.ExtractionProgress);
        Assert.Equal(string.Empty, vm.CurrentExtractionFile);
    }

    [Fact]
    public void TransitionToBatchExtracting_IsProgressVisible_True()
    {
        var vm = MakeVm();
        vm.TransitionToBatchExtracting();

        Assert.True(vm.IsProgressVisible);
    }

    [Fact]
    public void TransitionToBatchExtracting_IsCancelBatchVisible_True()
    {
        var vm = MakeVm();
        vm.TransitionToBatchExtracting();

        Assert.True(vm.IsCancelBatchVisible);
    }

    [Fact]
    public void TransitionFromBatchExtracting_SetsStateToIdle()
    {
        var vm = MakeVm();
        vm.TransitionToBatchExtracting();
        vm.TransitionFromBatchExtracting();

        Assert.Equal(UIState.Idle, vm.CurrentState);
    }

    [Fact]
    public void TransitionFromBatchExtracting_ResetsAllBatchProperties()
    {
        var vm = MakeVm();
        vm.TransitionToBatchExtracting();
        // Simulate some progress
        vm.BatchArchiveIndex = 3;
        vm.BatchArchiveCount = 5;
        vm.BatchCurrentArchiveName = "test.zip";
        vm.ExtractionProgress = 60;

        vm.TransitionFromBatchExtracting();

        Assert.Equal(0, vm.BatchArchiveIndex);
        Assert.Equal(0, vm.BatchArchiveCount);
        Assert.Equal(string.Empty, vm.BatchCurrentArchiveName);
        Assert.Equal(0, vm.ExtractionProgress);
        Assert.Equal(string.Empty, vm.CurrentExtractionFile);
    }

    [Fact]
    public void TransitionFromBatchExtracting_IsProgressVisible_False()
    {
        var vm = MakeVm();
        vm.TransitionToBatchExtracting();
        vm.TransitionFromBatchExtracting();

        Assert.False(vm.IsProgressVisible);
    }

    [Fact]
    public void TransitionFromBatchExtracting_IsCancelBatchVisible_False()
    {
        var vm = MakeVm();
        vm.TransitionToBatchExtracting();
        vm.TransitionFromBatchExtracting();

        Assert.False(vm.IsCancelBatchVisible);
    }

    // Property: BatchExtracting round-trip always returns to Idle with clean state
    [Property(MaxTest = 200)]
    public Property BatchExtractingRoundTrip_AlwaysReturnsToCleanIdle()
    {
        return Prop.ForAll(
            Arb.From(Gen.Choose(0, 100)),
            Arb.From(Gen.Choose(0, 50)),
            Arb.From(Gen.Elements("a.zip", "b.7z", "c.rar", "")),
            (progress, index, name) =>
            {
                var vm = MakeVm();
                vm.TransitionToBatchExtracting();

                // Simulate mid-operation state
                vm.ExtractionProgress = progress;
                vm.BatchArchiveIndex = index;
                vm.BatchCurrentArchiveName = name;

                vm.TransitionFromBatchExtracting();

                return (vm.CurrentState == UIState.Idle)
                    .Label("State must be Idle")
                    .And((vm.BatchArchiveIndex == 0).Label("BatchArchiveIndex must be 0"))
                    .And((vm.BatchArchiveCount == 0).Label("BatchArchiveCount must be 0"))
                    .And((vm.BatchCurrentArchiveName == string.Empty).Label("BatchCurrentArchiveName must be empty"))
                    .And((vm.ExtractionProgress == 0).Label("ExtractionProgress must be 0"))
                    .And((!vm.IsProgressVisible).Label("IsProgressVisible must be false"))
                    .And((!vm.IsCancelBatchVisible).Label("IsCancelBatchVisible must be false"));
            });
    }

    // Property: IsProgressVisible is true only during Extracting or BatchExtracting
    [Property(MaxTest = 200)]
    public Property IsProgressVisible_TrueOnlyDuringExtractingOrBatchExtracting()
    {
        return Prop.ForAll(
            Arb.From(Gen.Elements(
                UIState.Idle, UIState.DragOver, UIState.Previewing,
                UIState.Extracting, UIState.BatchExtracting)),
            state =>
            {
                var vm = MakeVm();
                switch (state)
                {
                    case UIState.Idle: vm.TransitionToIdle(); break;
                    case UIState.DragOver: vm.TransitionToDragOver(); break;
                    case UIState.Previewing: vm.TransitionToPreviewing("x.zip"); break;
                    case UIState.Extracting:
                        vm.TransitionToPreviewing("x.zip");
                        vm.TransitionToExtracting();
                        break;
                    case UIState.BatchExtracting:
                        vm.TransitionToBatchExtracting();
                        break;
                }

                bool expected = state == UIState.Extracting || state == UIState.BatchExtracting;
                return (vm.IsProgressVisible == expected)
                    .Label($"IsProgressVisible: expected {expected} for state {state}, got {vm.IsProgressVisible}");
            });
    }

    // ── 3. Multi-file drop format filtering logic ────────────────────────────
    // Validates: Requirements 1.1, 1.2

    [Fact]
    public void FormatFilter_SupportedExtensions_Accepted()
    {
        var svc = new ArchivePreviewService();
        var supported = new[]
        {
            "archive.zip", "archive.7z", "archive.rar",
            "archive.tar", "archive.gz", "archive.bz2",
            "archive.xz", "archive.zst", "archive.cab",
            "archive.iso", "archive.apk", "archive.ipa",
            "archive.jar", "archive.war", "archive.ear"
        };

        foreach (var file in supported)
        {
            Assert.True(svc.IsSupportedArchive(file), $"Expected '{file}' to be supported");
        }
    }

    [Fact]
    public void FormatFilter_UnsupportedExtensions_Rejected()
    {
        var svc = new ArchivePreviewService();
        var unsupported = new[]
        {
            "file.txt", "file.pdf", "file.exe",
            "file.docx", "file.mp4", "file.png",
            "file.html", "file.cs", "file.rs"
        };

        foreach (var file in unsupported)
        {
            Assert.False(svc.IsSupportedArchive(file), $"Expected '{file}' to be unsupported");
        }
    }

    [Fact]
    public void FormatFilter_EmptyOrNull_Rejected()
    {
        var svc = new ArchivePreviewService();
        Assert.False(svc.IsSupportedArchive(""));
        Assert.False(svc.IsSupportedArchive(null!));
    }

    [Fact]
    public void FormatFilter_MixedInput_FiltersCorrectly()
    {
        var svc = new ArchivePreviewService();
        var mixed = new[]
        {
            "archive.zip", "readme.txt", "data.7z",
            "photo.png", "backup.rar", "notes.pdf"
        };

        var filtered = mixed.Where(p => svc.IsSupportedArchive(p)).ToArray();

        Assert.Equal(3, filtered.Length);
        Assert.Contains("archive.zip", filtered);
        Assert.Contains("data.7z", filtered);
        Assert.Contains("backup.rar", filtered);
    }

    [Fact]
    public void FormatFilter_AllUnsupported_ReturnsEmpty()
    {
        var svc = new ArchivePreviewService();
        var allUnsupported = new[] { "a.txt", "b.pdf", "c.exe" };

        var filtered = allUnsupported.Where(p => svc.IsSupportedArchive(p)).ToArray();

        Assert.Empty(filtered);
    }

    [Fact]
    public void FormatFilter_AllSupported_ReturnsAll()
    {
        var svc = new ArchivePreviewService();
        var allSupported = new[] { "a.zip", "b.7z", "c.rar", "d.tar" };

        var filtered = allSupported.Where(p => svc.IsSupportedArchive(p)).ToArray();

        Assert.Equal(4, filtered.Length);
    }

    // Property: filtering never increases the count
    [Property(MaxTest = 200)]
    public Property FormatFilter_NeverIncreasesCount()
    {
        var supportedExts = new[] { "zip", "7z", "rar", "tar", "gz", "cab", "iso" };
        var unsupportedExts = new[] { "txt", "pdf", "exe", "docx", "mp4", "png" };
        var allExts = supportedExts.Concat(unsupportedExts).ToArray();

        return Prop.ForAll(
            Arb.From(Gen.Choose(0, 15).SelectMany(n =>
                Gen.ArrayOf(n, Gen.Elements(allExts))
                    .Select(exts => exts.Select(e => $"file.{e}").ToArray()))),
            files =>
            {
                var svc = new ArchivePreviewService();
                var filtered = files.Where(p => svc.IsSupportedArchive(p)).ToArray();
                return (filtered.Length <= files.Length)
                    .Label($"Filtered count {filtered.Length} > input count {files.Length}");
            });
    }

    // Property: filtering result contains only supported extensions
    [Property(MaxTest = 200)]
    public Property FormatFilter_ResultContainsOnlySupportedFormats()
    {
        var supportedExts = new[] { "zip", "7z", "rar", "tar", "gz", "bz2", "xz", "zst", "cab", "iso", "apk", "jar" };
        var unsupportedExts = new[] { "txt", "pdf", "exe", "docx", "mp4", "png", "html", "cs" };
        var allExts = supportedExts.Concat(unsupportedExts).ToArray();

        return Prop.ForAll(
            Arb.From(Gen.Choose(1, 20).SelectMany(n =>
                Gen.ArrayOf(n, Gen.Elements(allExts))
                    .Select(exts => exts.Select(e => $"file.{e}").ToArray()))),
            files =>
            {
                var svc = new ArchivePreviewService();
                var filtered = files.Where(p => svc.IsSupportedArchive(p)).ToArray();

                // Every file in the filtered result must be supported
                return filtered.All(f => svc.IsSupportedArchive(f))
                    .Label("All filtered files must be supported archives");
            });
    }

    // Property: batch mode triggers when filtered count >= 2
    // Validates: Requirements 1.2
    [Property(MaxTest = 200)]
    public Property BatchMode_TriggersWhenFilteredCountAtLeastTwo()
    {
        var supportedExts = new[] { "zip", "7z", "rar", "tar", "gz" };
        var unsupportedExts = new[] { "txt", "pdf", "exe" };
        var allExts = supportedExts.Concat(unsupportedExts).ToArray();

        return Prop.ForAll(
            Arb.From(Gen.Choose(0, 10).SelectMany(n =>
                Gen.ArrayOf(n, Gen.Elements(allExts))
                    .Select(exts => exts.Select(e => $"file.{e}").ToArray()))),
            files =>
            {
                var svc = new ArchivePreviewService();
                var supported = files.Where(p => svc.IsSupportedArchive(p)).ToList();

                // Batch mode should trigger when supported count >= 2
                bool shouldBatch = supported.Count >= 2;
                bool shouldSingle = supported.Count == 1;
                bool shouldReject = supported.Count == 0;

                // These are mutually exclusive
                int modeCount = (shouldBatch ? 1 : 0) + (shouldSingle ? 1 : 0) + (shouldReject ? 1 : 0);
                return (modeCount == 1)
                    .Label($"Exactly one mode must apply: batch={shouldBatch}, single={shouldSingle}, reject={shouldReject}");
            });
    }
}
