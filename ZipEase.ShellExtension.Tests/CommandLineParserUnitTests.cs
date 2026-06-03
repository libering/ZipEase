using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.ShellExtension.Tests;

/// <summary>
/// Unit tests for CommandLineParser.Parse.
/// Validates: Requirements 7.1, 7.2, 7.3, 7.4, 7.5, 7.6
/// </summary>
public class CommandLineParserUnitTests : IDisposable
{
    private readonly List<string> _tempFiles = new();
    private readonly List<string> _tempDirs = new();

    private string CreateTempFile()
    {
        var path = Path.GetTempFileName();
        _tempFiles.Add(path);
        return path;
    }

    private string CreateTempDirectory()
    {
        var path = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(path);
        _tempDirs.Add(path);
        return path;
    }

    public void Dispose()
    {
        foreach (var f in _tempFiles)
        {
            try { File.Delete(f); } catch { }
        }
        foreach (var d in _tempDirs)
        {
            try { Directory.Delete(d, true); } catch { }
        }
    }

    // ─── Extract mode (bare paths) ───────────────────────────────────────────

    [Fact]
    public void Parse_SingleExistingFilePath_ReturnsExtractMode()
    {
        var file = CreateTempFile();
        var result = CommandLineParser.Parse(new[] { file });

        Assert.Equal(CommandLineParser.Mode.Extract, result.Mode);
        Assert.Single(result.ValidPaths);
        Assert.Equal(file, result.ValidPaths[0]);
    }

    [Fact]
    public void Parse_MultipleExistingFilePaths_ReturnsExtractModeWithAllPaths()
    {
        var file1 = CreateTempFile();
        var file2 = CreateTempFile();
        var result = CommandLineParser.Parse(new[] { file1, file2 });

        Assert.Equal(CommandLineParser.Mode.Extract, result.Mode);
        Assert.Equal(2, result.ValidPaths.Length);
        Assert.Contains(file1, result.ValidPaths);
        Assert.Contains(file2, result.ValidPaths);
    }

    [Fact]
    public void Parse_ExistingDirectoryPath_ReturnsExtractMode()
    {
        var dir = CreateTempDirectory();
        var result = CommandLineParser.Parse(new[] { dir });

        Assert.Equal(CommandLineParser.Mode.Extract, result.Mode);
        Assert.Single(result.ValidPaths);
        Assert.Equal(dir, result.ValidPaths[0]);
    }

    // ─── Compress mode (--compress + paths) ──────────────────────────────────

    [Fact]
    public void Parse_CompressFlagWithSinglePath_ReturnsCompressMode()
    {
        var file = CreateTempFile();
        var result = CommandLineParser.Parse(new[] { "--compress", file });

        Assert.Equal(CommandLineParser.Mode.Compress, result.Mode);
        Assert.Single(result.ValidPaths);
        Assert.Equal(file, result.ValidPaths[0]);
    }

    [Fact]
    public void Parse_CompressFlagWithMultiplePaths_ReturnsCompressModeWithAllPaths()
    {
        var file1 = CreateTempFile();
        var file2 = CreateTempFile();
        var dir1 = CreateTempDirectory();
        var result = CommandLineParser.Parse(new[] { "--compress", file1, file2, dir1 });

        Assert.Equal(CommandLineParser.Mode.Compress, result.Mode);
        Assert.Equal(3, result.ValidPaths.Length);
        Assert.Contains(file1, result.ValidPaths);
        Assert.Contains(file2, result.ValidPaths);
        Assert.Contains(dir1, result.ValidPaths);
    }

    [Theory]
    [InlineData("--compress")]
    [InlineData("--COMPRESS")]
    [InlineData("--Compress")]
    public void Parse_CompressFlagCaseInsensitive_ReturnsCompressMode(string flag)
    {
        var file = CreateTempFile();
        var result = CommandLineParser.Parse(new[] { flag, file });

        Assert.Equal(CommandLineParser.Mode.Compress, result.Mode);
        Assert.Single(result.ValidPaths);
    }

    // ─── Register/Unregister modes ───────────────────────────────────────────

    [Fact]
    public void Parse_RegisterShellFlag_ReturnsRegisterShellMode()
    {
        var result = CommandLineParser.Parse(new[] { "--register-shell" });

        Assert.Equal(CommandLineParser.Mode.RegisterShell, result.Mode);
        Assert.Empty(result.ValidPaths);
    }

    [Fact]
    public void Parse_UnregisterShellFlag_ReturnsUnregisterShellMode()
    {
        var result = CommandLineParser.Parse(new[] { "--unregister-shell" });

        Assert.Equal(CommandLineParser.Mode.UnregisterShell, result.Mode);
        Assert.Empty(result.ValidPaths);
    }

    [Theory]
    [InlineData("--register-shell")]
    [InlineData("--REGISTER-SHELL")]
    [InlineData("--Register-Shell")]
    public void Parse_RegisterShellCaseInsensitive_ReturnsRegisterShellMode(string flag)
    {
        var result = CommandLineParser.Parse(new[] { flag });

        Assert.Equal(CommandLineParser.Mode.RegisterShell, result.Mode);
        Assert.Empty(result.ValidPaths);
    }

    [Theory]
    [InlineData("--unregister-shell")]
    [InlineData("--UNREGISTER-SHELL")]
    [InlineData("--Unregister-Shell")]
    public void Parse_UnregisterShellCaseInsensitive_ReturnsUnregisterShellMode(string flag)
    {
        var result = CommandLineParser.Parse(new[] { flag });

        Assert.Equal(CommandLineParser.Mode.UnregisterShell, result.Mode);
        Assert.Empty(result.ValidPaths);
    }

    // ─── Invalid path filtering ──────────────────────────────────────────────

    [Fact]
    public void Parse_MixOfValidAndInvalidPaths_FiltersOutInvalid()
    {
        var validFile = CreateTempFile();
        var invalidPath = @"C:\NonExistent\Path\file.zip";
        var result = CommandLineParser.Parse(new[] { validFile, invalidPath });

        Assert.Equal(CommandLineParser.Mode.Extract, result.Mode);
        Assert.Single(result.ValidPaths);
        Assert.Equal(validFile, result.ValidPaths[0]);
    }

    [Fact]
    public void Parse_CompressModeWithMixedPaths_FiltersOutInvalid()
    {
        var validFile = CreateTempFile();
        var invalidPath = @"C:\NonExistent\Path\folder";
        var result = CommandLineParser.Parse(new[] { "--compress", validFile, invalidPath });

        Assert.Equal(CommandLineParser.Mode.Compress, result.Mode);
        Assert.Single(result.ValidPaths);
        Assert.Equal(validFile, result.ValidPaths[0]);
    }

    // ─── All-invalid returns Normal mode ─────────────────────────────────────

    [Fact]
    public void Parse_AllInvalidPaths_ReturnsNormalMode()
    {
        var result = CommandLineParser.Parse(new[]
        {
            @"C:\NonExistent\file1.zip",
            @"C:\NonExistent\file2.rar"
        });

        Assert.Equal(CommandLineParser.Mode.Normal, result.Mode);
        Assert.Empty(result.ValidPaths);
    }

    [Fact]
    public void Parse_CompressFlagWithAllInvalidPaths_ReturnsNormalMode()
    {
        var result = CommandLineParser.Parse(new[]
        {
            "--compress",
            @"C:\NonExistent\file1.txt",
            @"C:\NonExistent\folder"
        });

        Assert.Equal(CommandLineParser.Mode.Normal, result.Mode);
        Assert.Empty(result.ValidPaths);
    }

    [Fact]
    public void Parse_NoArgs_ReturnsNormalMode()
    {
        var result = CommandLineParser.Parse(Array.Empty<string>());

        Assert.Equal(CommandLineParser.Mode.Normal, result.Mode);
        Assert.Empty(result.ValidPaths);
    }

    [Fact]
    public void Parse_NullArgs_ReturnsNormalMode()
    {
        var result = CommandLineParser.Parse(null!);

        Assert.Equal(CommandLineParser.Mode.Normal, result.Mode);
        Assert.Empty(result.ValidPaths);
    }

    [Fact]
    public void Parse_CompressFlagOnly_ReturnsNormalMode()
    {
        var result = CommandLineParser.Parse(new[] { "--compress" });

        Assert.Equal(CommandLineParser.Mode.Normal, result.Mode);
        Assert.Empty(result.ValidPaths);
    }
}
