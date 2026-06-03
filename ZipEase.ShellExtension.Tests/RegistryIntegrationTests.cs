using Microsoft.Win32;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.ShellExtension.Tests;

/// <summary>
/// Integration tests that verify actual Windows Registry operations for shell extension registration.
/// These tests write to and read from HKCU registry keys and clean up after themselves.
/// Mark with Category "Integration" so they can be excluded from CI environments.
/// </summary>
[Trait("Category", "Integration")]
public class RegistryIntegrationTests : IDisposable
{
    // Registry paths matching RegistrationManager's internal constants
    private const string ExtractKeyPath = @"Software\Classes\*\shell\ZipEaseExtract";
    private const string CompressKeyPath = @"Software\Classes\*\shell\ZipEaseCompress";
    private const string DirectoryCompressKeyPath = @"Software\Classes\Directory\shell\ZipEaseCompress";

    public RegistryIntegrationTests()
    {
        // Ensure clean state before each test
        CleanupRegistryKeys();
    }

    public void Dispose()
    {
        // Always clean up after tests
        CleanupRegistryKeys();
    }

    [Fact]
    public async Task RegisterAsync_CreatesExtractKeyUnderHKCU()
    {
        // Arrange
        var manager = new RegistrationManager();

        try
        {
            // Act
            var result = await manager.RegisterAsync();

            // Assert - Extract key should exist
            using var extractKey = Registry.CurrentUser.OpenSubKey(ExtractKeyPath);
            Assert.NotNull(extractKey);

            // Verify (Default) value is set (Chinese menu text)
            var defaultValue = extractKey.GetValue("") as string;
            Assert.NotNull(defaultValue);
            Assert.NotEmpty(defaultValue);

            // Verify Icon value is set
            var iconValue = extractKey.GetValue("Icon") as string;
            Assert.NotNull(iconValue);
            Assert.Contains("extract.ico", iconValue);

            // Verify AppliesTo value is set
            var appliesToValue = extractKey.GetValue("AppliesTo") as string;
            Assert.NotNull(appliesToValue);
            Assert.NotEmpty(appliesToValue);

            // Verify command subkey exists
            using var cmdKey = extractKey.OpenSubKey("command");
            Assert.NotNull(cmdKey);

            var cmdValue = cmdKey.GetValue("") as string;
            Assert.NotNull(cmdValue);
            Assert.Contains("ZipEase.exe", cmdValue);
            Assert.Contains("%1", cmdValue);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task RegisterAsync_CreatesCompressKeyUnderHKCU()
    {
        // Arrange
        var manager = new RegistrationManager();

        try
        {
            // Act
            var result = await manager.RegisterAsync();

            // Assert - Compress key for files should exist
            using var compressKey = Registry.CurrentUser.OpenSubKey(CompressKeyPath);
            Assert.NotNull(compressKey);

            // Verify (Default) value
            var defaultValue = compressKey.GetValue("") as string;
            Assert.NotNull(defaultValue);
            Assert.NotEmpty(defaultValue);

            // Verify Icon value
            var iconValue = compressKey.GetValue("Icon") as string;
            Assert.NotNull(iconValue);
            Assert.Contains("compress.ico", iconValue);

            // Verify command subkey
            using var cmdKey = compressKey.OpenSubKey("command");
            Assert.NotNull(cmdKey);

            var cmdValue = cmdKey.GetValue("") as string;
            Assert.NotNull(cmdValue);
            Assert.Contains("ZipEase.exe", cmdValue);
            Assert.Contains("--compress", cmdValue);
            Assert.Contains("%1", cmdValue);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task RegisterAsync_CreatesDirectoryCompressKeyUnderHKCU()
    {
        // Arrange
        var manager = new RegistrationManager();

        try
        {
            // Act
            var result = await manager.RegisterAsync();

            // Assert - Directory compress key should exist
            using var dirCompressKey = Registry.CurrentUser.OpenSubKey(DirectoryCompressKeyPath);
            Assert.NotNull(dirCompressKey);

            // Verify (Default) value
            var defaultValue = dirCompressKey.GetValue("") as string;
            Assert.NotNull(defaultValue);
            Assert.NotEmpty(defaultValue);

            // Verify Icon value
            var iconValue = dirCompressKey.GetValue("Icon") as string;
            Assert.NotNull(iconValue);
            Assert.Contains("compress.ico", iconValue);

            // Verify command subkey
            using var cmdKey = dirCompressKey.OpenSubKey("command");
            Assert.NotNull(cmdKey);

            var cmdValue = cmdKey.GetValue("") as string;
            Assert.NotNull(cmdValue);
            Assert.Contains("ZipEase.exe", cmdValue);
            Assert.Contains("--compress", cmdValue);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task UnregisterAsync_RemovesAllRegistryKeys()
    {
        // Arrange - first register to create keys
        var manager = new RegistrationManager();

        try
        {
            await manager.RegisterAsync();

            // Verify keys exist before unregistration
            Assert.NotNull(Registry.CurrentUser.OpenSubKey(ExtractKeyPath));
            Assert.NotNull(Registry.CurrentUser.OpenSubKey(CompressKeyPath));
            Assert.NotNull(Registry.CurrentUser.OpenSubKey(DirectoryCompressKeyPath));

            // Act
            var result = await manager.UnregisterAsync();

            // Assert - all keys should be removed
            using var extractKey = Registry.CurrentUser.OpenSubKey(ExtractKeyPath);
            using var compressKey = Registry.CurrentUser.OpenSubKey(CompressKeyPath);
            using var dirCompressKey = Registry.CurrentUser.OpenSubKey(DirectoryCompressKeyPath);

            Assert.Null(extractKey);
            Assert.Null(compressKey);
            Assert.Null(dirCompressKey);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task UnregisterAsync_SucceedsWhenKeysDoNotExist()
    {
        // Arrange - ensure no keys exist
        var manager = new RegistrationManager();
        CleanupRegistryKeys();

        try
        {
            // Act - unregister when nothing is registered should not throw
            var result = await manager.UnregisterAsync();

            // Assert - operation should succeed (best-effort removal)
            Assert.True(result.Success);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task RegisterAsync_AppliesToContainsAllSupportedExtensions()
    {
        // Arrange
        var manager = new RegistrationManager();
        var expectedExtensions = new[]
        {
            ".zip", ".7z", ".rar", ".tar", ".gz", ".bz2",
            ".cab", ".iso", ".apk", ".tgz",
            ".001", ".z01", ".z02", ".z03", ".z04", ".z05", ".z06", ".z07", ".z08", ".z09"
        };

        try
        {
            // Act
            await manager.RegisterAsync();

            // Assert
            using var extractKey = Registry.CurrentUser.OpenSubKey(ExtractKeyPath);
            Assert.NotNull(extractKey);

            var appliesToValue = extractKey.GetValue("AppliesTo") as string;
            Assert.NotNull(appliesToValue);

            // Verify each extension is present in the AppliesTo value
            foreach (var ext in expectedExtensions)
            {
                Assert.Contains($"System.FileExtension:={ext}", appliesToValue);
            }

            // Verify the format uses " OR " as separator
            Assert.Contains(" OR ", appliesToValue);

            // Verify the generated value matches what GenerateAppliesToValue produces
            var expectedValue = RegistrationManager.GenerateAppliesToValue();
            Assert.Equal(expectedValue, appliesToValue);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task RegisterAsync_CommandValueContainsCorrectExtractArguments()
    {
        // Arrange
        var manager = new RegistrationManager();

        try
        {
            // Act
            await manager.RegisterAsync();

            // Assert - Extract command should have quoted exe path and %1
            using var extractKey = Registry.CurrentUser.OpenSubKey(ExtractKeyPath);
            Assert.NotNull(extractKey);

            using var cmdKey = extractKey.OpenSubKey("command");
            Assert.NotNull(cmdKey);

            var cmdValue = cmdKey.GetValue("") as string;
            Assert.NotNull(cmdValue);

            // Command format: "\"<path>\ZipEase.exe\" \"%1\""
            // Verify it starts with a quote (quoted exe path)
            Assert.StartsWith("\"", cmdValue);
            // Verify it contains the exe name
            Assert.Contains("ZipEase.exe", cmdValue);
            // Verify it ends with "%1" pattern (quoted path argument)
            Assert.Contains("\"%1\"", cmdValue);
            // Extract command should NOT have --compress flag
            Assert.DoesNotContain("--compress", cmdValue);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task RegisterAsync_CommandValueContainsCorrectCompressArguments()
    {
        // Arrange
        var manager = new RegistrationManager();

        try
        {
            // Act
            await manager.RegisterAsync();

            // Assert - Compress command should have --compress flag
            using var compressKey = Registry.CurrentUser.OpenSubKey(CompressKeyPath);
            Assert.NotNull(compressKey);

            using var cmdKey = compressKey.OpenSubKey("command");
            Assert.NotNull(cmdKey);

            var cmdValue = cmdKey.GetValue("") as string;
            Assert.NotNull(cmdValue);

            // Command format: "\"<path>\ZipEase.exe\" --compress \"%1\""
            Assert.StartsWith("\"", cmdValue);
            Assert.Contains("ZipEase.exe", cmdValue);
            Assert.Contains("--compress", cmdValue);
            Assert.Contains("\"%1\"", cmdValue);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task RegisterAsync_ReturnsSuccessWithRegistryStrategy()
    {
        // Arrange
        var manager = new RegistrationManager();

        try
        {
            // Act
            var result = await manager.RegisterAsync();

            // Assert
            Assert.True(result.Success);
            // On most dev machines (especially Win10 or when MSIX APIs aren't available),
            // it should fall back to Registry strategy
            Assert.Equal(RegistrationManager.Strategy.Registry, result.UsedStrategy);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public void BuildArguments_ExtractInvocation_ProducesCorrectProcessStartArgs()
    {
        // Validates: Requirements 4.2, 6.3
        // Verifies that ExtractCommand.Invoke would pass correctly formatted arguments
        // to Process.Start (quoted paths, no --compress flag)
        var paths = new[] { @"C:\Users\Test\Documents\archive.zip", @"D:\My Files\backup.7z" };

        // Act - simulate what ExtractCommand.Invoke does
        string arguments = CommandBase.BuildArguments(paths);

        // Assert - extract uses just the quoted paths (no flags)
        Assert.Equal("\"C:\\Users\\Test\\Documents\\archive.zip\" \"D:\\My Files\\backup.7z\"", arguments);
        Assert.DoesNotContain("--compress", arguments);
    }

    [Fact]
    public void BuildArguments_CompressInvocation_ProducesCorrectProcessStartArgs()
    {
        // Validates: Requirements 4.2, 6.3
        // Verifies that CompressCommand.Invoke would pass correctly formatted arguments
        // to Process.Start (--compress flag followed by quoted paths)
        var paths = new[] { @"C:\Users\Test\Documents\report.docx", @"D:\My Files\photos" };

        // Act - simulate what CompressCommand.Invoke does
        string arguments = "--compress " + CommandBase.BuildArguments(paths);

        // Assert - compress prepends --compress flag before quoted paths
        Assert.StartsWith("--compress ", arguments);
        Assert.Contains("\"C:\\Users\\Test\\Documents\\report.docx\"", arguments);
        Assert.Contains("\"D:\\My Files\\photos\"", arguments);
    }

    [Fact]
    public async Task RegisterAsync_CheckStatus_ReturnsEnabled()
    {
        // Validates: Requirements 6.3
        // Verifies that after registration, CheckStatus correctly reports Enabled
        var manager = new RegistrationManager();

        try
        {
            // Act
            await manager.RegisterAsync();
            var status = manager.CheckStatus();

            // Assert
            Assert.Equal(ShellExtensionStatus.Enabled, status);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    [Fact]
    public async Task UnregisterAsync_CheckStatus_ReturnsDisabled()
    {
        // Validates: Requirements 6.3
        // Verifies that after unregistration, CheckStatus correctly reports Disabled
        var manager = new RegistrationManager();

        try
        {
            await manager.RegisterAsync();
            await manager.UnregisterAsync();
            var status = manager.CheckStatus();

            // Assert
            Assert.Equal(ShellExtensionStatus.Disabled, status);
        }
        finally
        {
            CleanupRegistryKeys();
        }
    }

    /// <summary>
    /// Removes all ZipEase registry keys to ensure clean test state.
    /// </summary>
    private static void CleanupRegistryKeys()
    {
        TryDeleteSubKeyTree(ExtractKeyPath);
        TryDeleteSubKeyTree(CompressKeyPath);
        TryDeleteSubKeyTree(DirectoryCompressKeyPath);
    }

    private static void TryDeleteSubKeyTree(string subKeyPath)
    {
        try
        {
            Registry.CurrentUser.DeleteSubKeyTree(subKeyPath, throwOnMissingSubKey: false);
        }
        catch
        {
            // Best-effort cleanup
        }
    }
}
