using Xunit;
using ZipEase.ShellExtension;

namespace ZipEase.ShellExtension.Tests;

/// <summary>
/// Placeholder test class to verify the test project builds and references are correct.
/// </summary>
public class PlaceholderTests
{
    [Fact]
    public void Project_Builds_And_References_Are_Valid()
    {
        // Verify we can reference types from ZipEase.ShellExtension
        var result = ArchiveExtensions.IsArchiveFile("test.zip");
        Assert.True(result);
    }

    [Fact]
    public void NonArchive_Extension_Returns_False()
    {
        var result = ArchiveExtensions.IsArchiveFile("readme.txt");
        Assert.False(result);
    }
}
