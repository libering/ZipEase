using Xunit;
using ZipEase.ShellExtension;

namespace ZipEase.ShellExtension.Tests;

/// <summary>
/// Unit tests for LocalizedStrings.GetExtractTitle and GetCompressTitle.
/// Validates: Requirements 5.1
/// </summary>
public class LocalizedStringsUnitTests
{
    [Theory]
    [InlineData("zh-TW")]
    [InlineData("zh-CN")]
    [InlineData("zh-HK")]
    [InlineData("zh-Hant")]
    [InlineData("zh-Hans")]
    public void GetExtractTitle_ChineseLocale_ReturnsChineseText(string locale)
    {
        var result = LocalizedStrings.GetExtractTitle(locale);
        Assert.Equal("用 ZipEase 解壓縮", result);
    }

    [Theory]
    [InlineData("zh-TW")]
    [InlineData("zh-CN")]
    [InlineData("zh-HK")]
    [InlineData("zh-Hant")]
    [InlineData("zh-Hans")]
    public void GetCompressTitle_ChineseLocale_ReturnsChineseText(string locale)
    {
        var result = LocalizedStrings.GetCompressTitle(locale);
        Assert.Equal("用 ZipEase 壓縮", result);
    }

    [Theory]
    [InlineData("en-US")]
    [InlineData("en-GB")]
    public void GetExtractTitle_EnglishLocale_ReturnsEnglishText(string locale)
    {
        var result = LocalizedStrings.GetExtractTitle(locale);
        Assert.Equal("Extract with ZipEase", result);
    }

    [Theory]
    [InlineData("en-US")]
    [InlineData("en-GB")]
    public void GetCompressTitle_EnglishLocale_ReturnsEnglishText(string locale)
    {
        var result = LocalizedStrings.GetCompressTitle(locale);
        Assert.Equal("Compress with ZipEase", result);
    }

    [Theory]
    [InlineData("fr-FR")]
    [InlineData("de-DE")]
    [InlineData("ja-JP")]
    [InlineData("ko-KR")]
    [InlineData("es-ES")]
    [InlineData("unknown")]
    public void GetExtractTitle_UnknownLocale_ReturnsEnglishFallback(string locale)
    {
        var result = LocalizedStrings.GetExtractTitle(locale);
        Assert.Equal("Extract with ZipEase", result);
    }

    [Theory]
    [InlineData("fr-FR")]
    [InlineData("de-DE")]
    [InlineData("ja-JP")]
    [InlineData("ko-KR")]
    [InlineData("es-ES")]
    [InlineData("unknown")]
    public void GetCompressTitle_UnknownLocale_ReturnsEnglishFallback(string locale)
    {
        var result = LocalizedStrings.GetCompressTitle(locale);
        Assert.Equal("Compress with ZipEase", result);
    }
}
