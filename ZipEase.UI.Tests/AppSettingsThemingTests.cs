using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

/// <summary>
/// Unit tests for AppSettings theming-related fields and validation.
/// Validates: Requirements 5.1, 5.4
/// </summary>
public class AppSettingsThemingTests
{
    // Validates: Requirement 5.1
    [Fact]
    public void DefaultBackdropType_IsMica()
    {
        var settings = new AppSettings();

        Assert.Equal(1, settings.BackdropType);
    }

    // Validates: Requirement 5.1
    [Fact]
    public void DefaultActiveThemeFile_IsEmptyString()
    {
        var settings = new AppSettings();

        Assert.Equal(string.Empty, settings.ActiveThemeFile);
    }

    // Validates: Requirement 5.4
    [Fact]
    public void ValidateThemingFields_StaleThemeFile_ClearedToEmpty()
    {
        var settings = new AppSettings();
        // Set to a filename that definitely does not exist in the themes folder.
        settings.ActiveThemeFile = "nonexistent_theme_that_does_not_exist.xaml";

        settings.ValidateThemingFields();

        Assert.Equal(string.Empty, settings.ActiveThemeFile);
    }
}
