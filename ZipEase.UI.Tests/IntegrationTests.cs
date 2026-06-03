using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

public class IntegrationTests
{
    // ── Helpers ───────────────────────────────────────────────────────────────

    private static void CreateTestZip(string path, string[] fileNames)
    {
        using var archive = ZipFile.Open(path, ZipArchiveMode.Create);
        foreach (var name in fileNames)
        {
            var entry = archive.CreateEntry(name);
            using var writer = new StreamWriter(entry.Open());
            writer.Write($"dummy content for {name}");
        }
    }

    // ── Task 10.1: End-to-end extraction ─────────────────────────────────────

    [Fact(Skip = "Requires zipease_core.dll")]
    [Trait("Category", "Integration")]
    public async void ExtractAsync_EndToEnd_ExtractsAllFiles()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ZipEase_test_" + Guid.NewGuid().ToString("N"));
        string outputDir = Path.Combine(Path.GetTempPath(), "ZipEase_out_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        Directory.CreateDirectory(outputDir);

        string archivePath = Path.Combine(tempDir, "test.zip");
        var files = new[] { "hello.txt", "world.txt", "data.csv" };

        try
        {
            CreateTestZip(archivePath, files);

            int count = await ExtractionManager.ExtractAsync(archivePath, outputDir);

            Assert.Equal(3, count);
            foreach (var name in files)
                Assert.True(File.Exists(Path.Combine(outputDir, name)), $"{name} not found in output");
        }
        finally
        {
            Directory.Delete(tempDir, recursive: true);
            Directory.Delete(outputDir, recursive: true);
        }
    }

    // ── Task 10.2: Progress callback verification ─────────────────────────────

    [Fact(Skip = "Requires zipease_core.dll")]
    [Trait("Category", "Integration")]
    public async void ExtractAsync_ProgressCallback_ReportsMonotonicPercentages()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ZipEase_test_" + Guid.NewGuid().ToString("N"));
        string outputDir = Path.Combine(Path.GetTempPath(), "ZipEase_out_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        Directory.CreateDirectory(outputDir);

        string archivePath = Path.Combine(tempDir, "test.zip");
        var files = new[] { "a.txt", "b.txt", "c.txt", "d.txt", "e.txt" };

        try
        {
            CreateTestZip(archivePath, files);

            var invocations = new List<(int percentage, string fileName)>();

            await ExtractionManager.ExtractAsync(archivePath, outputDir, progressCallback: (pct, file) =>
            {
                invocations.Add((pct, file));
            });

            Assert.NotEmpty(invocations);

            foreach (var (pct, _) in invocations)
                Assert.InRange(pct, 0, 100);

            // Percentages must be non-decreasing
            for (int i = 1; i < invocations.Count; i++)
                Assert.True(invocations[i].percentage >= invocations[i - 1].percentage,
                    $"Percentage decreased: {invocations[i - 1].percentage} → {invocations[i].percentage}");

            // All 5 file names must appear in reported names
            var reportedNames = invocations.Select(x => x.fileName).ToHashSet();
            foreach (var name in files)
                Assert.Contains(name, reportedNames);
        }
        finally
        {
            Directory.Delete(tempDir, recursive: true);
            Directory.Delete(outputDir, recursive: true);
        }
    }

    // ── Task 10.3: Error scenario tests ──────────────────────────────────────

    [Fact]
    public async void ExtractAsync_MissingArchive_ThrowsFileNotFoundException()
    {
        await Assert.ThrowsAsync<FileNotFoundException>(() =>
            ExtractionManager.ExtractAsync(@"C:\does_not_exist_xyz_abc_99999.zip", @"C:\output"));
    }

    [Fact]
    public async void ExtractAsync_NullArchivePath_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            ExtractionManager.ExtractAsync(null!, @"C:\output"));
    }

    [Fact]
    public async void ExtractAsync_EmptyOutputDir_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            ExtractionManager.ExtractAsync(@"C:\archive.zip", ""));
    }

    [Fact(Skip = "Requires zipease_core.dll")]
    [Trait("Category", "Integration")]
    public async void ExtractAsync_InvalidArchiveFormat_ThrowsExtractionException()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ZipEase_test_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        string archivePath = Path.Combine(tempDir, "garbage.zip");

        try
        {
            File.WriteAllText(archivePath, "NOT A ZIP FILE");

            await Assert.ThrowsAsync<ExtractionException>(() =>
                ExtractionManager.ExtractAsync(archivePath, tempDir));
        }
        finally
        {
            Directory.Delete(tempDir, recursive: true);
        }
    }

    // ── Task 10.4: UI state transition tests ─────────────────────────────────

    [Fact]
    public void ViewModel_InitialState_IsIdle()
    {
        var vm = new MainWindowViewModel(new MockArchivePreviewService());

        Assert.Equal(UIState.Idle, vm.CurrentState);
        Assert.True(vm.IsIdleVisible);
        Assert.False(vm.IsPreviewVisible);
        Assert.False(vm.IsProgressVisible);
    }

    [Fact]
    public void ViewModel_TransitionToExtracting_ShowsProgress()
    {
        var vm = new MainWindowViewModel(new MockArchivePreviewService());
        vm.TransitionToPreviewing("test.zip");
        vm.TransitionToExtracting();

        Assert.True(vm.IsProgressVisible);
        Assert.False(vm.IsExtractButtonEnabled);
        Assert.True(vm.IsPreviewVisible);
    }

    [Fact]
    public void ViewModel_TransitionBackToPreviewing_HidesProgress()
    {
        var vm = new MainWindowViewModel(new MockArchivePreviewService());
        vm.TransitionToPreviewing("test.zip");
        vm.TransitionToExtracting();
        vm.TransitionBackToPreviewing();

        Assert.False(vm.IsProgressVisible);
        Assert.True(vm.IsPreviewVisible);
    }

    [Fact]
    public void ViewModel_TransitionToIdle_ResetsAllState()
    {
        var vm = new MainWindowViewModel(new MockArchivePreviewService());
        vm.TransitionToPreviewing("test.zip");
        vm.TransitionToExtracting();
        vm.TransitionToIdle();

        Assert.Equal(UIState.Idle, vm.CurrentState);
        Assert.Equal(0, vm.ExtractionProgress);
        Assert.Equal(string.Empty, vm.CurrentExtractionFile);
        Assert.False(vm.IsProgressVisible);
        Assert.True(vm.IsIdleVisible);
    }
}
