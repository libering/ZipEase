using System;
using System.Windows;
using Wpf.Ui.Appearance;
using Wpf.Ui.Controls;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Abstracts OS version lookup so property tests can inject arbitrary build numbers.
    /// </summary>
    internal interface IOsVersionProvider
    {
        /// <summary>Returns the OS build number (e.g. 22000 for Windows 11 21H2).</summary>
        int BuildNumber { get; }
    }

    /// <summary>
    /// Default implementation that reads <see cref="Environment.OSVersion"/>.
    /// </summary>
    internal sealed class DefaultOsVersionProvider : IOsVersionProvider
    {
        public static readonly DefaultOsVersionProvider Instance = new();
        public int BuildNumber => Environment.OSVersion.Version.Build;
    }

    /// <summary>
    /// 靜態工具類，封裝 WPF-UI 4.x 的 backdrop 切換邏輯。
    /// Maps settings int → WindowBackdropType, checks OS support, and applies.
    /// </summary>
    public static class BackdropSwitcher
    {
        // Minimum OS build numbers for each backdrop type.
        private const int MicaMinBuild = 22000;    // Windows 11 21H2
        private const int AcrylicMinBuild = 17134;  // Windows 10 1803

        /// <summary>
        /// Injected for testing. Production code uses <see cref="DefaultOsVersionProvider"/>.
        /// </summary>
        internal static IOsVersionProvider OsVersionProvider { get; set; } = DefaultOsVersionProvider.Instance;

        /// <summary>
        /// 將 settings int 轉換為 WPF-UI WindowBackdropType。
        /// 0 → None, 1 → Mica, 2 → Acrylic.
        /// Out-of-range values default to None.
        /// </summary>
        public static WindowBackdropType ToBackdropType(int value) => value switch
        {
            1 => WindowBackdropType.Mica,
            2 => WindowBackdropType.Acrylic,
            _ => WindowBackdropType.None,
        };

        /// <summary>
        /// 檢查當前 OS 是否支援指定的 backdrop 類型。
        /// None is always supported. Mica requires Build ≥ 22000. Acrylic requires Build ≥ 17134.
        /// </summary>
        public static bool IsSupported(int backdropType)
        {
            int build = OsVersionProvider.BuildNumber;
            return backdropType switch
            {
                1 => build >= MicaMinBuild,
                2 => build >= AcrylicMinBuild,
                _ => true, // None (0) and any unknown value treated as None → always supported
            };
        }

        /// <summary>
        /// 套用指定的 backdrop 類型到目標視窗。
        /// 若 window 為 null，回傳 false（no-op）。
        /// 若 OS 不支援所選類型，自動 fallback 到 None 並回傳 false。
        /// </summary>
        /// <param name="backdropType">0=None, 1=Mica, 2=Acrylic</param>
        /// <param name="window">目標視窗（FluentWindow）。Null is safe — returns false.</param>
        /// <returns>true if applied as requested, false if fell back to None or window was null.</returns>
        public static bool Apply(int backdropType, Window? window)
        {
            if (window is null)
                return false;

            if (!IsSupported(backdropType))
            {
                // Fallback: apply None instead of the unsupported type.
                try
                {
                    var currentTheme = ApplicationThemeManager.GetAppTheme();
                    ApplicationThemeManager.Apply(currentTheme, WindowBackdropType.None);
                    if (window is FluentWindow fluentNone)
                        fluentNone.WindowBackdropType = WindowBackdropType.None;
                }
                catch
                {
                    // Best-effort — never crash on backdrop apply failure.
                }
                return false;
            }

            try
            {
                var backdrop = ToBackdropType(backdropType);
                var currentTheme = ApplicationThemeManager.GetAppTheme();
                ApplicationThemeManager.Apply(currentTheme, backdrop);
                if (window is FluentWindow fluent)
                    fluent.WindowBackdropType = backdrop;
                return true;
            }
            catch
            {
                // Fallback to None on any exception.
                try
                {
                    var currentTheme = ApplicationThemeManager.GetAppTheme();
                    ApplicationThemeManager.Apply(currentTheme, WindowBackdropType.None);
                    if (window is FluentWindow fluentFallback)
                        fluentFallback.WindowBackdropType = WindowBackdropType.None;
                }
                catch { /* best-effort */ }
                return false;
            }
        }
    }
}
