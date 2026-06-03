using System;
using System.IO;
using System.Threading;
using System.Windows;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

/// <summary>
/// Unit tests for <see cref="ThemeLoader"/>.
/// Tests ScanFolder (pure logic) and LoadTheme/UnloadTheme/ReloadTheme/UnloadAll
/// (require WPF Application context on an STA thread).
/// Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 2.4
/// </summary>
[Collection("WPF")]
public class ThemeLoaderTests : IDisposable
{
    private readonly string _tempDir;

    /// <summary>A valid XAML ResourceDictionary for testing.</summary>
    private const string ValidXaml =
        """<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><SolidColorBrush x:Key="TestBrush" Color="Red" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"/></ResourceDictionary>""";

    /// <summary>A second valid XAML ResourceDictionary with a different resource.</summary>
    private const string ValidXaml2 =
        """<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><SolidColorBrush x:Key="TestBrush2" Color="Blue" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"/></ResourceDictionary>""";

    /// <summary>Invalid XAML content that will cause XamlParseException.</summary>
    private const string InvalidXaml = "<Not valid XAML at all <<<>>>";

    public ThemeLoaderTests()
    {
        _tempDir = Path.Combine(Path.GetTempPath(), "ZipEase_ThemeLoaderTests_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(_tempDir);
    }

    public void Dispose()
    {
        try
        {
            if (Directory.Exists(_tempDir))
                Directory.Delete(_tempDir, recursive: true);
        }
        catch
        {
            // Best-effort cleanup.
        }
    }

    /// <summary>
    /// Runs an action on an STA thread with a minimal WPF Application context.
    /// Required for tests that access Application.Current.Resources.MergedDictionaries
    /// or use XamlReader.Load().
    /// </summary>
    private static void RunOnStaWithApp(Action action)
    {
        Exception? caught = null;
        var thread = new Thread(() =>
        {
            // Create a minimal WPF Application if one doesn't exist in this AppDomain.
            // WPF allows only one Application per AppDomain — ever. Even after
            // Shutdown(), creating a second instance throws. We guard with try/catch.
            try
            {
                if (Application.Current == null)
                {
                    try
                    {
                        _ = new Application { ShutdownMode = ShutdownMode.OnExplicitShutdown };
                    }
                    catch (InvalidOperationException)
                    {
                        // Another test already created an Application in this AppDomain.
                    }
                }

                action();
            }
            catch (Exception ex)
            {
                caught = ex;
            }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();

        if (caught != null)
            throw new AggregateException("STA test failed", caught);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // ScanFolder tests — pure logic, no WPF dependency
    // ═══════════════════════════════════════════════════════════════════════════

    // Validates: Requirement 1.1
    [Fact]
    public void ScanFolder_EmptyFolder_ReturnsEmpty()
    {
        var result = ThemeLoader.ScanFolder(_tempDir);

        Assert.Empty(result);
    }

    // Validates: Requirement 1.1
    [Fact]
    public void ScanFolder_NonExistentFolder_ReturnsEmpty()
    {
        var nonExistent = Path.Combine(_tempDir, "does_not_exist");

        var result = ThemeLoader.ScanFolder(nonExistent);

        Assert.Empty(result);
    }

    // Validates: Requirement 1.1
    [Fact]
    public void ScanFolder_MixedFileTypes_ReturnsOnlyXaml()
    {
        // Create a mix of file types.
        File.WriteAllText(Path.Combine(_tempDir, "theme1.xaml"), ValidXaml);
        File.WriteAllText(Path.Combine(_tempDir, "theme2.XAML"), ValidXaml);
        File.WriteAllText(Path.Combine(_tempDir, "readme.txt"), "text");
        File.WriteAllText(Path.Combine(_tempDir, "config.xml"), "<xml/>");
        File.WriteAllText(Path.Combine(_tempDir, "data.json"), "{}");
        File.WriteAllText(Path.Combine(_tempDir, "style.css"), "body{}");

        var result = ThemeLoader.ScanFolder(_tempDir);

        Assert.Equal(2, result.Length);
        Assert.All(result, f =>
            Assert.Equal(".xaml", Path.GetExtension(f), StringComparer.OrdinalIgnoreCase));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // LoadTheme tests — require WPF Application context
    // ═══════════════════════════════════════════════════════════════════════════

    // Validates: Requirement 1.2
    [Fact]
    public void LoadTheme_ValidXaml_ReturnsTrue()
    {
        var xamlPath = Path.Combine(_tempDir, "valid.xaml");
        File.WriteAllText(xamlPath, ValidXaml);

        RunOnStaWithApp(() =>
        {
            var loader = new ThemeLoader();
            int initialCount = Application.Current.Resources.MergedDictionaries.Count;

            bool result = loader.LoadTheme(xamlPath);

            Assert.True(result);
            Assert.Equal(1, loader.LoadedCount);
            Assert.Equal(initialCount + 1, Application.Current.Resources.MergedDictionaries.Count);

            // Cleanup: remove from MergedDictionaries.
            loader.UnloadAll();
        });
    }

    // Validates: Requirement 1.3
    [Fact]
    public void LoadTheme_InvalidXaml_ReturnsFalse_NoCrash()
    {
        var xamlPath = Path.Combine(_tempDir, "invalid.xaml");
        File.WriteAllText(xamlPath, InvalidXaml);

        RunOnStaWithApp(() =>
        {
            var loader = new ThemeLoader();
            int initialCount = Application.Current.Resources.MergedDictionaries.Count;

            bool result = loader.LoadTheme(xamlPath);

            Assert.False(result);
            Assert.Equal(0, loader.LoadedCount);
            Assert.Equal(initialCount, Application.Current.Resources.MergedDictionaries.Count);
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // UnloadTheme tests
    // ═══════════════════════════════════════════════════════════════════════════

    // Validates: Requirement 1.5
    [Fact]
    public void UnloadTheme_RemovesFromMergedDictionaries()
    {
        var xamlPath = Path.Combine(_tempDir, "toremove.xaml");
        File.WriteAllText(xamlPath, ValidXaml);

        RunOnStaWithApp(() =>
        {
            var loader = new ThemeLoader();
            loader.LoadTheme(xamlPath);
            int countAfterLoad = Application.Current.Resources.MergedDictionaries.Count;

            loader.UnloadTheme(xamlPath);

            Assert.Equal(0, loader.LoadedCount);
            Assert.Equal(countAfterLoad - 1, Application.Current.Resources.MergedDictionaries.Count);
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // UnloadAll tests
    // ═══════════════════════════════════════════════════════════════════════════

    // Validates: Requirement 1.5
    [Fact]
    public void UnloadAll_RestoresDefaults()
    {
        var xamlPath1 = Path.Combine(_tempDir, "theme1.xaml");
        var xamlPath2 = Path.Combine(_tempDir, "theme2.xaml");
        File.WriteAllText(xamlPath1, ValidXaml);
        File.WriteAllText(xamlPath2, ValidXaml2);

        RunOnStaWithApp(() =>
        {
            var loader = new ThemeLoader();

            loader.LoadTheme(xamlPath1);
            loader.LoadTheme(xamlPath2);
            Assert.Equal(2, loader.LoadedCount);

            int countBeforeUnload = Application.Current.Resources.MergedDictionaries.Count;

            loader.UnloadAll();

            Assert.Equal(0, loader.LoadedCount);
            // UnloadAll should have removed exactly the 2 dictionaries we loaded.
            Assert.Equal(countBeforeUnload - 2, Application.Current.Resources.MergedDictionaries.Count);
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // ReloadTheme tests
    // ═══════════════════════════════════════════════════════════════════════════

    // Validates: Requirement 2.4
    [Fact]
    public void ReloadTheme_InvalidXaml_KeepsPreviousVersion()
    {
        var xamlPath = Path.Combine(_tempDir, "reload.xaml");
        File.WriteAllText(xamlPath, ValidXaml);

        RunOnStaWithApp(() =>
        {
            var loader = new ThemeLoader();

            // Load the valid version first.
            bool loaded = loader.LoadTheme(xamlPath);
            Assert.True(loaded);
            Assert.Equal(1, loader.LoadedCount);

            // Overwrite the file with invalid XAML.
            File.WriteAllText(xamlPath, InvalidXaml);

            // Reload should fail but keep the previous version.
            bool reloaded = loader.ReloadTheme(xamlPath);

            Assert.False(reloaded);
            // Previous version should be restored — LoadedCount stays at 1.
            Assert.Equal(1, loader.LoadedCount);

            // Cleanup.
            loader.UnloadAll();
        });
    }
}
