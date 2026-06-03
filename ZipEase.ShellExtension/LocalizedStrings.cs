using System.Globalization;

namespace ZipEase.ShellExtension;

/// <summary>
/// Provides localized menu text for the shell extension context menu items.
/// Returns Traditional Chinese for zh-* locales, English for all others.
/// </summary>
internal static class LocalizedStrings
{
    /// <summary>
    /// Gets the localized title for the "Extract with ZipEase" menu item.
    /// </summary>
    /// <param name="locale">The locale identifier (e.g., "zh-TW", "en-US").</param>
    /// <returns>Localized extract menu title.</returns>
    public static string GetExtractTitle(string locale)
        => locale.StartsWith("zh", StringComparison.OrdinalIgnoreCase)
            ? "用 ZipEase 解壓縮"
            : "Extract with ZipEase";

    /// <summary>
    /// Gets the localized title for the "Compress with ZipEase" menu item.
    /// </summary>
    /// <param name="locale">The locale identifier (e.g., "zh-TW", "en-US").</param>
    /// <returns>Localized compress menu title.</returns>
    public static string GetCompressTitle(string locale)
        => locale.StartsWith("zh", StringComparison.OrdinalIgnoreCase)
            ? "用 ZipEase 壓縮"
            : "Compress with ZipEase";

    /// <summary>
    /// Gets the current system UI culture locale name.
    /// </summary>
    /// <returns>The current UI culture name (e.g., "zh-TW", "en-US").</returns>
    public static string GetCurrentLocale()
        => CultureInfo.CurrentUICulture.Name;
}
