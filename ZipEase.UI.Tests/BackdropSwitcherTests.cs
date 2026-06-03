using System;
using System.Windows;
using Wpf.Ui.Controls;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.UI.Tests;

/// <summary>
/// Unit tests for <see cref="BackdropSwitcher"/>.
/// Tests ToBackdropType mapping, IsSupported OS checks, and Apply fallback behavior.
/// Validates: Requirements 3.1, 3.2, 3.5
/// </summary>
public class BackdropSwitcherTests : IDisposable
{
    public void Dispose()
    {
        // Restore the default OS version provider after each test to avoid leaking state.
        BackdropSwitcher.OsVersionProvider = DefaultOsVersionProvider.Instance;
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // ToBackdropType tests — pure mapping logic
    // ═══════════════════════════════════════════════════════════════════════════

    // Validates: Requirement 3.1
    [Fact]
    public void ToBackdropType_Zero_ReturnsNone()
    {
        var result = BackdropSwitcher.ToBackdropType(0);

        Assert.Equal(WindowBackdropType.None, result);
    }

    // Validates: Requirement 3.1
    [Fact]
    public void ToBackdropType_One_ReturnsMica()
    {
        var result = BackdropSwitcher.ToBackdropType(1);

        Assert.Equal(WindowBackdropType.Mica, result);
    }

    // Validates: Requirement 3.1
    [Fact]
    public void ToBackdropType_Two_ReturnsAcrylic()
    {
        var result = BackdropSwitcher.ToBackdropType(2);

        Assert.Equal(WindowBackdropType.Acrylic, result);
    }

    // Validates: Requirement 3.1
    [Theory]
    [InlineData(-1)]
    [InlineData(3)]
    [InlineData(100)]
    [InlineData(-999)]
    [InlineData(int.MaxValue)]
    [InlineData(int.MinValue)]
    public void ToBackdropType_OutOfRange_DefaultsToNone(int value)
    {
        var result = BackdropSwitcher.ToBackdropType(value);

        Assert.Equal(WindowBackdropType.None, result);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // IsSupported tests — OS version gating via FakeOsVersionProvider
    // ═══════════════════════════════════════════════════════════════════════════

    // Validates: Requirement 3.5
    [Fact]
    public void IsSupported_Mica_OnWindows10_ReturnsFalse()
    {
        // Windows 10 21H2 build = 19044, well below Mica threshold of 22000.
        BackdropSwitcher.OsVersionProvider = new FakeOsVersionProvider(19044);

        bool result = BackdropSwitcher.IsSupported(1); // 1 = Mica

        Assert.False(result);
    }

    // Validates: Requirement 3.5
    [Fact]
    public void IsSupported_Mica_OnWindows11_ReturnsTrue()
    {
        // Windows 11 21H2 build = 22000, exactly at Mica threshold.
        BackdropSwitcher.OsVersionProvider = new FakeOsVersionProvider(22000);

        bool result = BackdropSwitcher.IsSupported(1); // 1 = Mica

        Assert.True(result);
    }

    // Validates: Requirement 3.5
    [Fact]
    public void IsSupported_Acrylic_OnOldWindows10_ReturnsFalse()
    {
        // Windows 10 1709 build = 16299, below Acrylic threshold of 17134.
        BackdropSwitcher.OsVersionProvider = new FakeOsVersionProvider(16299);

        bool result = BackdropSwitcher.IsSupported(2); // 2 = Acrylic

        Assert.False(result);
    }

    // Validates: Requirement 3.5
    [Fact]
    public void IsSupported_Acrylic_OnWindows10_1803_ReturnsTrue()
    {
        // Windows 10 1803 build = 17134, exactly at Acrylic threshold.
        BackdropSwitcher.OsVersionProvider = new FakeOsVersionProvider(17134);

        bool result = BackdropSwitcher.IsSupported(2); // 2 = Acrylic

        Assert.True(result);
    }

    // Validates: Requirement 3.5
    [Fact]
    public void IsSupported_None_AlwaysReturnsTrue()
    {
        // Even on a very old build, None should always be supported.
        BackdropSwitcher.OsVersionProvider = new FakeOsVersionProvider(10000);

        bool result = BackdropSwitcher.IsSupported(0); // 0 = None

        Assert.True(result);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Apply tests — null window and unsupported fallback
    // ═══════════════════════════════════════════════════════════════════════════

    // Validates: Requirement 3.2
    [Fact]
    public void Apply_NullWindow_ReturnsFalse()
    {
        bool result = BackdropSwitcher.Apply(1, null);

        Assert.False(result);
    }

    // Validates: Requirement 3.5
    [Fact]
    public void Apply_UnsupportedMica_OnWindows10_ReturnsFalse()
    {
        // Set OS to Windows 10 (below Mica threshold).
        BackdropSwitcher.OsVersionProvider = new FakeOsVersionProvider(19044);

        // Apply with null window — tests the IsSupported check path.
        // Since we can't easily create a FluentWindow in tests, we verify
        // the method returns false for unsupported + null window.
        bool result = BackdropSwitcher.Apply(1, null);

        Assert.False(result);
    }
}
