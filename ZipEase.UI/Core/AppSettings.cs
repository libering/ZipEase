using System;
using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;
using CommunityToolkit.Mvvm.ComponentModel;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Configurable thresholds for Zip Bomb detection.
    /// Persisted as part of AppSettings.
    /// </summary>
    public class ZipBombThresholdsSettings
    {
        public double MaxCompressionRatio { get; set; } = 100.0;
        public double MaxTotalGb { get; set; } = 15.0;
        public double MaxSingleEntryGb { get; set; } = 8.0;
        public int MaxNestingDepth { get; set; } = 3;
    }

    /// <summary>
    /// Persisted user preferences. Saved to %AppData%\ZipEase\settings.json.
    /// All defaults are ADHD-friendly safe choices.
    /// </summary>
    public partial class AppSettings : ObservableObject
    {
        private static readonly string SettingsPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "ZipEase", "settings.json");

        private static readonly JsonSerializerOptions JsonOpts = new()
        {
            WriteIndented = true,
            DefaultIgnoreCondition = JsonIgnoreCondition.Never
        };

        // ── Extraction Behaviour ──────────────────────────────────────────────
        /// <summary>Skip CRC errors and extract whatever is readable.</summary>
        [ObservableProperty] private bool _forceExtract = false;

        // ── Cleanup & Operations ──────────────────────────────────────────────
        /// <summary>Move source archive to Recycle Bin after successful extraction.</summary>
        [ObservableProperty] private bool _autoTrashAfterExtract = false;

        /// <summary>Show which app is locking a file when access is denied.</summary>
        [ObservableProperty] private bool _lockDetection = true;

        // ── Notifications & Appearance ────────────────────────────────────────
        /// <summary>Show Windows toast when extraction completes.</summary>
        [ObservableProperty] private bool _toastNotifications = true;

        /// <summary>0 = follow system, 1 = light, 2 = dark</summary>
        [ObservableProperty] private int _theme = 0;

        /// <summary>Backdrop type: 0=None, 1=Mica, 2=Acrylic. Default=1 (Mica).</summary>
        [ObservableProperty] private int _backdropType = 1;

        /// <summary>目前啟用的自訂主題檔案名稱（不含路徑）。空字串 = 無自訂主題。</summary>
        [ObservableProperty] private string _activeThemeFile = string.Empty;

        // ── Last Used Paths ───────────────────────────────────────────────────
        /// <summary>Last output directory chosen for extraction. Empty = not set.</summary>
        [ObservableProperty] private string _lastOutputDir = string.Empty;

        /// <summary>BCP-47 language tag. Empty = follow system.</summary>
        [ObservableProperty] private string _language = "zh-TW";

        // ── Zip Bomb Protection ───────────────────────────────────────────────
        /// <summary>Configurable thresholds for Zip Bomb detection.</summary>
        [ObservableProperty] private ZipBombThresholdsSettings _zipBombThresholds = new();

        // ── Shell Extension Integration ───────────────────────────────────────
        /// <summary>Current status of the Windows Shell Extension (context menu) registration.</summary>
        [ObservableProperty] private ShellExtensionStatus _shellStatus = ShellExtensionStatus.Disabled;

        /// <summary>Error message from the last failed shell extension registration attempt. Null if no error.</summary>
        [ObservableProperty] private string? _shellRegistrationError;

        // ── Singleton ─────────────────────────────────────────────────────────
        private static AppSettings? _instance;
        public static AppSettings Instance => _instance ??= Load();

        /// <summary>Fired when ForceExtract is changed from the Settings page.</summary>
        public static event System.Action? ForceExtractChanged;

        public void RaiseForceExtractChanged() => ForceExtractChanged?.Invoke();

        private static AppSettings Load()
        {
            AppSettings settings;
            try
            {
                if (File.Exists(SettingsPath))
                {
                    string json = File.ReadAllText(SettingsPath);
                    settings = JsonSerializer.Deserialize<AppSettings>(json, JsonOpts) ?? new AppSettings();
                }
                else
                {
                    settings = new AppSettings();
                }
            }
            catch
            {
                /* corrupt file — fall back to defaults */
                settings = new AppSettings();
            }

            settings.ValidateThemingFields();
            return settings;
        }

        /// <summary>
        /// Validates theming-related fields after deserialization:
        /// - Clamps <see cref="BackdropType"/> to [0, 2].
        /// - Clears <see cref="ActiveThemeFile"/> if the referenced file no longer exists.
        /// </summary>
        internal void ValidateThemingFields()
        {
            // Clamp backdropType to valid range [0, 2]
            BackdropType = Math.Clamp(BackdropType, 0, 2);

            // Clear activeThemeFile if the referenced file no longer exists
            if (!string.IsNullOrEmpty(ActiveThemeFile))
            {
                var fullPath = Path.Combine(ThemeLoader.ThemesFolder, ActiveThemeFile);
                if (!File.Exists(fullPath))
                {
                    ActiveThemeFile = string.Empty;
                }
            }
        }

        public void Save()
        {
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(SettingsPath)!);
                File.WriteAllText(SettingsPath, JsonSerializer.Serialize(this, JsonOpts));
            }
            catch { /* best-effort — never crash on settings save */ }
        }
    }
}

