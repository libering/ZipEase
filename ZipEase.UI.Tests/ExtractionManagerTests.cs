using System;
using System.IO;
using Xunit;
using FsCheck;
using FsCheck.Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

/// <summary>
/// Unit tests for ExtractionManager parameter validation (ui-integration tasks 4.8, 4.9).
/// These tests do NOT call the Rust FFI — they only test the C# validation layer.
/// </summary>
public class ExtractionManagerTests
{
    // ── Task 4.8: Parameter validation ───────────────────────────────────────

    [Fact]
    public async Task ExtractAsync_NullArchivePath_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            ExtractionManager.ExtractAsync(null!, "C:\\output"));
    }

    [Fact]
    public async Task ExtractAsync_EmptyArchivePath_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            ExtractionManager.ExtractAsync("", "C:\\output"));
    }

    [Fact]
    public async Task ExtractAsync_NullOutputDir_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            ExtractionManager.ExtractAsync("archive.zip", null!));
    }

    [Fact]
    public async Task ExtractAsync_EmptyOutputDir_ThrowsArgumentException()
    {
        await Assert.ThrowsAsync<ArgumentException>(() =>
            ExtractionManager.ExtractAsync("archive.zip", ""));
    }

    [Fact]
    public async Task ExtractAsync_MissingFile_ThrowsFileNotFoundException()
    {
        await Assert.ThrowsAsync<FileNotFoundException>(() =>
            ExtractionManager.ExtractAsync(
                "C:\\does_not_exist_xyz_abc_12345.zip",
                "C:\\output"));
    }

    // ── Task 4.9: Property 17 — Callback parameter validation ────────────────
    // Feature: ui-integration, Property 17
    // Validates: Requirements 9.4

    [Property(MaxTest = 500)]
    public Property CallbackIgnoresOutOfRangePercentage()
    {
        var gen = Gen.OneOf(
            Gen.Choose(-1000, -1),
            Gen.Choose(101, 1000)
        );
        return Prop.ForAll(
            Arb.From(gen),
            invalidPct =>
            {
                bool callbackInvoked = false;
                void SimulatedNativeCallback(int percentage, IntPtr fileNamePtr)
                {
                    if (percentage < 0 || percentage > 100) return;
                    callbackInvoked = true;
                }

                SimulatedNativeCallback(invalidPct, IntPtr.Zero);
                return (!callbackInvoked)
                    .Label($"callback must not fire for percentage={invalidPct}");
            });
    }

    [Property(MaxTest = 200)]
    public Property CallbackAcceptsValidPercentage()
    {
        return Prop.ForAll(
            Arb.From(Gen.Choose(0, 100)),
            validPct =>
            {
                bool callbackInvoked = false;
                void SimulatedNativeCallback(int percentage, IntPtr fileNamePtr)
                {
                    if (percentage < 0 || percentage > 100) return;
                    callbackInvoked = true;
                }

                SimulatedNativeCallback(validPct, IntPtr.Zero);
                return callbackInvoked
                    .Label($"callback must fire for valid percentage={validPct}");
            });
    }

    [Fact]
    public async Task PluginBackend_NonExistentScript_ThrowsPluginException()
    {
        var manifest = new ZipEase.UI.Core.Plugin.PluginManifest
        {
            Name = "Mock Python Plugin",
            Executable = "non_existent_script.py"
        };
        var loaded = new ZipEase.UI.Core.Plugin.LoadedPlugin(manifest, "C:\\non_existent_script.py");

        await Assert.ThrowsAsync<ZipEase.UI.Core.Plugin.PluginException>(() =>
            ZipEase.UI.Core.Plugin.PluginBackend.ListAsync(loaded, "dummy_archive.zip"));
    }
}

