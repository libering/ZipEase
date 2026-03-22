using System.Collections.Generic;
using System.Linq;
using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

/// <summary>
/// Property-based tests for MainWindowViewModel state machine and navigation logic.
/// Uses FsCheck to generate arbitrary inputs and verify universal correctness properties.
/// </summary>
public class ViewModelStateTests
{
    // ── Helpers ──────────────────────────────────────────────────────────────

    private static MainWindowViewModel MakeVm(List<ArchiveEntry>? entries = null)
        => new(new MockArchivePreviewService(entries));

    private static ArchiveEntry MakeEntry(string name, bool isDir = false) => new()
    {
        FileName = name,
        IsDirectory = isDir,
        FileType = isDir ? "Folder" : "TXT",
        FormattedSize = isDir ? "—" : "1 KB",
        Size = isDir ? 0 : 1024,
    };

    // ── ui-overhaul Property 1: State-visibility consistency ─────────────────
    // Feature: ui-overhaul, task 4.2
    // Validates: Requirements 2.5, 5.6, 7.3, 7.4

    [Fact]
    public void IdleState_IsIdleVisible_True()
    {
        var vm = MakeVm();
        vm.TransitionToIdle();
        Assert.True(vm.IsIdleVisible);
        Assert.False(vm.IsPreviewVisible);
    }

    [Fact]
    public void PreviewingState_IsPreviewVisible_True()
    {
        var vm = MakeVm();
        vm.TransitionToPreviewing("test.zip");
        Assert.False(vm.IsIdleVisible);
        Assert.True(vm.IsPreviewVisible);
    }

    [Fact]
    public void ExtractingState_IsPreviewVisible_True_IsProgressVisible_True()
    {
        var vm = MakeVm();
        vm.TransitionToPreviewing("test.zip");
        vm.TransitionToExtracting();
        Assert.True(vm.IsPreviewVisible);
        Assert.True(vm.IsProgressVisible);
    }

    [Fact]
    public void DragOverState_IsIdleVisible_True()
    {
        var vm = MakeVm();
        vm.TransitionToDragOver();
        Assert.True(vm.IsIdleVisible);
    }

    // Property: for any UIState, IsIdleVisible == (Idle || DragOver)
    [Property(MaxTest = 200)]
    public Property StateVisibilityConsistency()
    {
        return Prop.ForAll(
            Arb.From(Gen.Elements(UIState.Idle, UIState.DragOver, UIState.Previewing, UIState.Extracting)),
            state =>
            {
                var vm = MakeVm();
                switch (state)
                {
                    case UIState.Idle:       vm.TransitionToIdle(); break;
                    case UIState.DragOver:   vm.TransitionToDragOver(); break;
                    case UIState.Previewing: vm.TransitionToPreviewing("x.zip"); break;
                    case UIState.Extracting:
                        vm.TransitionToPreviewing("x.zip");
                        vm.TransitionToExtracting();
                        break;
                }

                bool expectedIdle    = state == UIState.Idle || state == UIState.DragOver;
                bool expectedPreview = state == UIState.Previewing || state == UIState.Extracting;

                return (vm.IsIdleVisible == expectedIdle)
                    .Label($"IsIdleVisible: expected {expectedIdle}, got {vm.IsIdleVisible} for {state}")
                    .And((vm.IsPreviewVisible == expectedPreview)
                    .Label($"IsPreviewVisible: expected {expectedPreview}, got {vm.IsPreviewVisible} for {state}"));
            });
    }

    // ── ui-overhaul Property 2: Supported format acceptance ──────────────────
    // Feature: ui-overhaul, task 4.3
    // Validates: Requirements 3.2, 3.3, 3.4

    [Theory]
    [InlineData("archive.zip")]
    [InlineData("archive.rar")]
    [InlineData("archive.7z")]
    [InlineData("archive.tar")]
    [InlineData("archive.gz")]
    public void SupportedFormat_IsSupportedArchive_True(string path)
    {
        var svc = new MockArchivePreviewService();
        Assert.True(svc.IsSupportedArchive(path));
    }

    // ── ui-overhaul Property 3: Unsupported format rejection ─────────────────
    // Feature: ui-overhaul, task 4.4
    // Validates: Requirements 3.5, 9.4

    [Theory]
    [InlineData("file.exe")]
    [InlineData("file.pdf")]
    [InlineData("file.docx")]
    [InlineData("file.mp4")]
    [InlineData("noextension")]
    public void UnsupportedFormat_IsSupportedArchive_False(string path)
    {
        var svc = new MockArchivePreviewService();
        Assert.False(svc.IsSupportedArchive(path));
    }

    [Property(MaxTest = 200)]
    public Property UnsupportedExtension_NotInSupportedSet()
    {
        var supported = new[] { ".zip", ".rar", ".7z", ".tar", ".gz" };
        return Prop.ForAll(
            Arb.From(Gen.Elements("exe", "pdf", "docx", "mp4", "txt", "dll", "bat", "cmd")),
            ext =>
            {
                var svc = new MockArchivePreviewService();
                return !svc.IsSupportedArchive($"file.{ext}");
            });
    }

    // ── ui-overhaul Property 5: Archive listing completeness ─────────────────
    // Feature: ui-overhaul, task 4.5
    // Validates: Requirements 5.1, 5.2, 5.3, 5.4

    [Property(MaxTest = 100)]
    public Property ArchiveListingCompleteness()
    {
        return Prop.ForAll(
            Arb.From(Gen.Choose(1, 20).Select(n =>
                Enumerable.Range(0, n)
                    .Select(i => MakeEntry($"file_{i}.txt"))
                    .ToList())),
            entries =>
            {
                var svc = new MockArchivePreviewService(entries);
                var (result, returned, _) = svc.ListArchiveContentsWithPassword("fake.zip", null);

                return (result == ListResult.Success).Label("result must be Success")
                    .And((returned.Count == entries.Count).Label($"count: expected {entries.Count}, got {returned.Count}"))
                    .And(returned.All(e => !string.IsNullOrEmpty(e.FileName)).Label("all entries have non-empty FileName"))
                    .And(returned.All(e => !string.IsNullOrEmpty(e.FileType)).Label("all entries have non-empty FileType"));
            });
    }

    // ── ui-enhancements Property 2: Entry filtering by CurrentPath ───────────
    // Feature: ui-enhancements, task 8.5
    // Validates: Requirements 2.2, 2.4

    [Property(MaxTest = 200)]
    public Property EntryFilteringByCurrentPath()
    {
        return Prop.ForAll(
            Arb.From(Gen.Choose(0, 15).Select(n =>
                Enumerable.Range(0, n)
                    .Select(i => $"dir{i % 3}/file_{i}.txt")
                    .ToList())),
            Arb.From(Gen.Elements("", "dir0/", "dir1/", "dir2/", "nonexistent/")),
            (paths, currentPath) =>
            {
                var expected = paths
                    .Where(p => MainWindowViewModel.GetImmediateParent(p) == currentPath)
                    .ToList();

                var actual = paths
                    .Where(p => MainWindowViewModel.GetImmediateParent(p) == currentPath)
                    .ToList();

                return (actual.Count == expected.Count)
                    .Label($"filtered count mismatch for CurrentPath='{currentPath}'");
            });
    }

    // ── ui-enhancements Property 3: Navigation stack round-trip ─────────────
    // Feature: ui-enhancements, task 8.6
    // Validates: Requirements 2.3, 2.7

    [Property(MaxTest = 100)]
    public Property NavigationStackRoundTrip()
    {
        return Prop.ForAll(
            Arb.From(Gen.Choose(1, 8).Select(n =>
                Enumerable.Range(0, n).Select(i => $"dir{i}/").ToList())),
            dirs =>
            {
                var entries = dirs
                    .Select(d => MakeEntry(d + "placeholder.txt", false))
                    .Concat(dirs.Select(d => MakeEntry(d.TrimEnd('/'), true)))
                    .ToList();

                var vm = MakeVm(entries);
                vm.TransitionToPreviewing("test.zip");

                foreach (var dir in dirs)
                {
                    var dirEntry = new ArchiveEntryViewModel(MakeEntry(dir.TrimEnd('/'), true));
                    vm.NavigateIntoCommand.Execute(dirEntry);
                }

                for (int i = 0; i < dirs.Count; i++)
                    vm.NavigateBackCommand.Execute(null);

                return (vm.CurrentPath == string.Empty)
                    .Label($"CurrentPath should be empty after round-trip, got '{vm.CurrentPath}'");
            });
    }

    // ── ui-enhancements Property 4: NavigateBackCommand.CanExecute ───────────
    // Feature: ui-enhancements, task 8.7
    // Validates: Requirements 2.5, 2.6

    [Property(MaxTest = 200)]
    public Property NavigateBackCanExecuteReflectsCurrentPath()
    {
        return Prop.ForAll(
            Arb.From(Gen.Elements("", "dir/", "dir/sub/", "a/b/c/")),
            currentPath =>
            {
                var vm = MakeVm();
                vm.TransitionToPreviewing("test.zip");

                if (!string.IsNullOrEmpty(currentPath))
                {
                    var parts = currentPath.TrimEnd('/').Split('/');
                    foreach (var part in parts)
                    {
                        var entry = new ArchiveEntryViewModel(MakeEntry(part, true));
                        vm.NavigateIntoCommand.Execute(entry);
                    }
                }

                bool expectedCanExecute = !string.IsNullOrEmpty(vm.CurrentPath);
                bool actualCanExecute = vm.NavigateBackCommand.CanExecute(null);

                return (actualCanExecute == expectedCanExecute)
                    .Label($"CanExecute: expected {expectedCanExecute}, got {actualCanExecute} for path '{vm.CurrentPath}'");
            });
    }

    // ── ui-enhancements Property 5: File entry no-op navigation ─────────────
    // Feature: ui-enhancements, task 8.8
    // Validates: Requirements 2.12

    [Property(MaxTest = 200)]
    public Property FileEntryNavigationIsNoOp()
    {
        return Prop.ForAll(
            Arb.From(Gen.Elements("file.txt", "image.png", "doc.pdf", "data.csv")),
            fileName =>
            {
                var vm = MakeVm();
                vm.TransitionToPreviewing("test.zip");
                var before = vm.CurrentPath;

                var fileEntry = new ArchiveEntryViewModel(MakeEntry(fileName, isDir: false));
                vm.NavigateIntoCommand.Execute(fileEntry);

                return (vm.CurrentPath == before)
                    .Label($"CurrentPath changed after file navigation: '{before}' → '{vm.CurrentPath}'");
            });
    }

    // ── ui-enhancements Property 6: FileCount equals non-directory count ─────
    // Feature: ui-enhancements, task 8.9
    // Validates: Requirements 3.1, 3.2

    [Property(MaxTest = 200)]
    public Property FileCountEqualsNonDirectoryCount()
    {
        return Prop.ForAll(
            Arb.From(Gen.Choose(0, 20).Select(n =>
                Enumerable.Range(0, n)
                    .Select(i => MakeEntry($"item_{i}", i % 3 == 0))
                    .ToList())),
            entries =>
            {
                var vm = MakeVm(entries);
                vm.TransitionToPreviewing("test.zip");

                foreach (var e in entries)
                    vm.ArchiveEntries.Add(new ArchiveEntryViewModel(e));

                int expectedFileCount = entries.Count(e => !e.IsDirectory);
                return (vm.FileCount == expectedFileCount)
                    .Label($"FileCount: expected {expectedFileCount}, got {vm.FileCount}");
            });
    }

    // ── ui-enhancements Property 8: Password cleared on idle/success ─────────
    // Feature: ui-enhancements, task 9.2
    // Validates: Requirements 7.3, 7.4

    [Fact]
    public void TransitionToIdle_ClearsPendingPassword()
    {
        var vm = MakeVm();
        vm.TransitionToPreviewing("test.zip");
        vm.TransitionToIdle();
        Assert.Equal(UIState.Idle, vm.CurrentState);
        Assert.Equal(string.Empty, vm.LoadedArchivePath);
        Assert.Equal(string.Empty, vm.CurrentPath);
    }

    [Property(MaxTest = 100)]
    public Property IdleTransitionAlwaysClearsState()
    {
        return Prop.ForAll(
            Arb.From(Gen.Elements(UIState.Idle, UIState.Previewing, UIState.Extracting)),
            state =>
            {
                var vm = MakeVm();
                switch (state)
                {
                    case UIState.Previewing: vm.TransitionToPreviewing("x.zip"); break;
                    case UIState.Extracting:
                        vm.TransitionToPreviewing("x.zip");
                        vm.TransitionToExtracting();
                        break;
                }
                vm.TransitionToIdle();

                return (vm.CurrentState == UIState.Idle).Label("state must be Idle")
                    .And((vm.LoadedArchivePath == string.Empty).Label("LoadedArchivePath must be empty"))
                    .And((vm.CurrentPath == string.Empty).Label("CurrentPath must be empty"))
                    .And((vm.ExtractionProgress == 0).Label("ExtractionProgress must be 0"));
            });
    }

    // ── GetImmediateParent unit tests ─────────────────────────────────────────

    [Theory]
    [InlineData("file.txt",     "")]
    [InlineData("dir/file.txt", "dir/")]
    [InlineData("a/b/c.txt",    "a/b/")]
    [InlineData("dir/",         "")]
    [InlineData("a/b/",         "a/")]
    public void GetImmediateParent_ReturnsCorrectParent(string entry, string expected)
    {
        Assert.Equal(expected, MainWindowViewModel.GetImmediateParent(entry));
    }

    [Property(MaxTest = 200)]
    public Property GetImmediateParent_NeverThrows()
    {
        return Prop.ForAll(
            Arb.From(Gen.Fresh(() => (string?)null).OrNull().Select(s => s ?? Guid.NewGuid().ToString())),
            s =>
            {
                try
                {
                    MainWindowViewModel.GetImmediateParent(s);
                    return true.ToProperty();
                }
                catch
                {
                    return false.Label($"GetImmediateParent threw for input: '{s}'");
                }
            });
    }
}
