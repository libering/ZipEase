using Xunit;
using ZipEase.ShellExtension;

namespace ZipEase.ShellExtension.Tests;

/// <summary>
/// Unit tests for ArchiveExtensions.IsArchiveFile.
/// Validates: Requirements 1.3
/// </summary>
public class ArchiveExtensionsUnitTests
{
    [Theory]
    [InlineData("archive.zip")]
    [InlineData("archive.7z")]
    [InlineData("archive.rar")]
    [InlineData("archive.tar")]
    [InlineData("archive.gz")]
    [InlineData("archive.bz2")]
    [InlineData("archive.cab")]
    [InlineData("archive.iso")]
    [InlineData("archive.apk")]
    [InlineData("archive.tgz")]
    [InlineData("archive.001")]
    [InlineData("archive.z01")]
    [InlineData("archive.z02")]
    [InlineData("archive.z03")]
    [InlineData("archive.z04")]
    [InlineData("archive.z05")]
    [InlineData("archive.z06")]
    [InlineData("archive.z07")]
    [InlineData("archive.z08")]
    [InlineData("archive.z09")]
    public void IsArchiveFile_KnownExtensions_ReturnsTrue(string filePath)
    {
        Assert.True(ArchiveExtensions.IsArchiveFile(filePath));
    }

    [Theory]
    [InlineData("archive.tar.gz")]
    [InlineData("archive.tar.bz2")]
    [InlineData("C:\\Users\\test\\documents\\backup.tar.gz")]
    [InlineData("folder/subfolder/data.tar.bz2")]
    public void IsArchiveFile_CompoundExtensions_ReturnsTrue(string filePath)
    {
        Assert.True(ArchiveExtensions.IsArchiveFile(filePath));
    }

    [Theory]
    [InlineData("archive.ZIP")]
    [InlineData("archive.Rar")]
    [InlineData("archive.7Z")]
    [InlineData("archive.Tar")]
    [InlineData("archive.GZ")]
    [InlineData("archive.BZ2")]
    [InlineData("archive.CAB")]
    [InlineData("archive.ISO")]
    [InlineData("archive.APK")]
    [InlineData("archive.TGZ")]
    [InlineData("archive.TAR.GZ")]
    [InlineData("archive.TAR.BZ2")]
    [InlineData("archive.Z01")]
    public void IsArchiveFile_CaseInsensitive_ReturnsTrue(string filePath)
    {
        Assert.True(ArchiveExtensions.IsArchiveFile(filePath));
    }

    [Theory]
    [InlineData("document.txt")]
    [InlineData("program.exe")]
    [InlineData("library.dll")]
    [InlineData("image.png")]
    [InlineData("video.mp4")]
    [InlineData("readme.md")]
    [InlineData("config.json")]
    public void IsArchiveFile_NonArchiveExtensions_ReturnsFalse(string filePath)
    {
        Assert.False(ArchiveExtensions.IsArchiveFile(filePath));
    }

    [Theory]
    [InlineData("")]
    [InlineData(null)]
    public void IsArchiveFile_NullOrEmpty_ReturnsFalse(string? filePath)
    {
        Assert.False(ArchiveExtensions.IsArchiveFile(filePath!));
    }

    [Fact]
    public void IsArchiveFile_FileWithNoExtension_ReturnsFalse()
    {
        Assert.False(ArchiveExtensions.IsArchiveFile("README"));
    }

    [Theory]
    [InlineData("C:\\Users\\test\\Downloads\\backup.zip")]
    [InlineData("/home/user/files/archive.7z")]
    [InlineData("relative/path/to/file.rar")]
    public void IsArchiveFile_FullPaths_ReturnsTrue(string filePath)
    {
        Assert.True(ArchiveExtensions.IsArchiveFile(filePath));
    }
}
