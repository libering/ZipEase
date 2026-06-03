using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.ShellExtension;

namespace ZipEase.ShellExtension.Tests;

// Feature: context-menu, Property 6: Localization returns correct language
/// <summary>
/// Property-based tests verifying that LocalizedStrings.GetExtractTitle and GetCompressTitle
/// return correct language text based on locale: zh-* → Traditional Chinese, others → English.
/// Results must always be non-empty.
/// Validates: Requirements 5.1
/// </summary>
public class LocalizationPropertyTests
{
    // Expected Chinese text
    private const string ChineseExtractTitle = "用 ZipEase 解壓縮";
    private const string ChineseCompressTitle = "用 ZipEase 壓縮";

    // Expected English text
    private const string EnglishExtractTitle = "Extract with ZipEase";
    private const string EnglishCompressTitle = "Compress with ZipEase";

    /// <summary>
    /// Generator for zh-* locale variants (Chinese locales).
    /// </summary>
    private static Gen<string> GenChineseLocale()
    {
        return Gen.Elements(
            "zh", "zh-TW", "zh-CN", "zh-HK", "zh-SG", "zh-MO",
            "zh-Hant", "zh-Hans", "zh-Hant-TW", "zh-Hans-CN",
            "zh-Hant-HK", "zh-Hans-SG"
        );
    }

    /// <summary>
    /// Generator for non-zh locale variants (non-Chinese locales).
    /// </summary>
    private static Gen<string> GenNonChineseLocale()
    {
        return Gen.Elements(
            "en", "en-US", "en-GB", "en-AU", "en-CA",
            "fr", "fr-FR", "fr-CA",
            "de", "de-DE", "de-AT",
            "ja", "ja-JP",
            "ko", "ko-KR",
            "es", "es-ES", "es-MX",
            "pt", "pt-BR", "pt-PT",
            "it", "it-IT",
            "ru", "ru-RU",
            "ar", "ar-SA",
            "hi", "hi-IN",
            "th", "th-TH",
            "vi", "vi-VN",
            "nl", "nl-NL",
            "sv", "sv-SE",
            "pl", "pl-PL"
        );
    }

    /// <summary>
    /// Generator for random locale-like strings that do NOT start with "zh".
    /// </summary>
    private static Gen<string> GenRandomNonZhLocale()
    {
        // Generate random 2-letter language codes that are not "zh"
        var langCodes = Gen.Elements(
            "aa", "ab", "af", "ak", "am", "an", "ar", "as", "av", "ay",
            "ba", "be", "bg", "bh", "bi", "bm", "bn", "bo", "br", "bs",
            "ca", "ce", "ch", "co", "cr", "cs", "cu", "cv", "cy", "da",
            "de", "dv", "dz", "ee", "el", "en", "eo", "es", "et", "eu",
            "fa", "ff", "fi", "fj", "fo", "fr", "fy", "ga", "gd", "gl",
            "gn", "gu", "gv", "ha", "he", "hi", "ho", "hr", "ht", "hu",
            "hy", "hz", "ia", "id", "ie", "ig", "ii", "ik", "in", "io",
            "is", "it", "iu", "ja", "jv", "ka", "kg", "ki", "kj", "kk",
            "kl", "km", "kn", "ko", "kr", "ks", "ku", "kv", "kw", "ky",
            "la", "lb", "lg", "li", "ln", "lo", "lt", "lu", "lv", "mg",
            "mh", "mi", "mk", "ml", "mn", "mr", "ms", "mt", "my", "na",
            "nb", "nd", "ne", "ng", "nl", "nn", "no"
        );

        var regionCodes = Gen.Elements(
            "US", "GB", "CA", "AU", "FR", "DE", "JP", "KR", "BR", "IN"
        );

        return from lang in langCodes
               from includeRegion in Gen.Elements(true, false)
               from region in regionCodes
               select includeRegion ? $"{lang}-{region}" : lang;
    }

    // ─── Property Tests ───────────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property ChineseLocale_GetExtractTitle_ReturnsChineseText()
    {
        // **Validates: Requirements 5.1**
        var gen = GenChineseLocale().ToArbitrary();

        return Prop.ForAll(gen, locale =>
            LocalizedStrings.GetExtractTitle(locale) == ChineseExtractTitle);
    }

    [Property(MaxTest = 100)]
    public Property ChineseLocale_GetCompressTitle_ReturnsChineseText()
    {
        // **Validates: Requirements 5.1**
        var gen = GenChineseLocale().ToArbitrary();

        return Prop.ForAll(gen, locale =>
            LocalizedStrings.GetCompressTitle(locale) == ChineseCompressTitle);
    }

    [Property(MaxTest = 100)]
    public Property NonChineseLocale_GetExtractTitle_ReturnsEnglishText()
    {
        // **Validates: Requirements 5.1**
        var gen = GenNonChineseLocale().ToArbitrary();

        return Prop.ForAll(gen, locale =>
            LocalizedStrings.GetExtractTitle(locale) == EnglishExtractTitle);
    }

    [Property(MaxTest = 100)]
    public Property NonChineseLocale_GetCompressTitle_ReturnsEnglishText()
    {
        // **Validates: Requirements 5.1**
        var gen = GenNonChineseLocale().ToArbitrary();

        return Prop.ForAll(gen, locale =>
            LocalizedStrings.GetCompressTitle(locale) == EnglishCompressTitle);
    }

    [Property(MaxTest = 100)]
    public Property RandomNonZhLocale_GetExtractTitle_ReturnsEnglishText()
    {
        // **Validates: Requirements 5.1**
        var gen = GenRandomNonZhLocale().ToArbitrary();

        return Prop.ForAll(gen, locale =>
            LocalizedStrings.GetExtractTitle(locale) == EnglishExtractTitle);
    }

    [Property(MaxTest = 100)]
    public Property RandomNonZhLocale_GetCompressTitle_ReturnsEnglishText()
    {
        // **Validates: Requirements 5.1**
        var gen = GenRandomNonZhLocale().ToArbitrary();

        return Prop.ForAll(gen, locale =>
            LocalizedStrings.GetCompressTitle(locale) == EnglishCompressTitle);
    }

    [Property(MaxTest = 100)]
    public Property GetExtractTitle_AlwaysReturnsNonEmpty()
    {
        // **Validates: Requirements 5.1**
        // For any locale (zh or non-zh), result is always non-empty
        var gen = Gen.OneOf(GenChineseLocale(), GenNonChineseLocale(), GenRandomNonZhLocale())
            .ToArbitrary();

        return Prop.ForAll(gen, locale =>
            !string.IsNullOrEmpty(LocalizedStrings.GetExtractTitle(locale)));
    }

    [Property(MaxTest = 100)]
    public Property GetCompressTitle_AlwaysReturnsNonEmpty()
    {
        // **Validates: Requirements 5.1**
        // For any locale (zh or non-zh), result is always non-empty
        var gen = Gen.OneOf(GenChineseLocale(), GenNonChineseLocale(), GenRandomNonZhLocale())
            .ToArbitrary();

        return Prop.ForAll(gen, locale =>
            !string.IsNullOrEmpty(LocalizedStrings.GetCompressTitle(locale)));
    }
}
