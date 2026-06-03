using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using System.Threading.Tasks;
using Wpf.Ui.Appearance;

namespace ZipEase.UI.Core
{
    public partial class SettingsViewModel : ObservableObject
    {
        private readonly AppSettings _s = AppSettings.Instance;

        public bool ForceExtract
        {
            get => _s.ForceExtract;
            set
            {
                _s.ForceExtract = value;
                OnPropertyChanged();
                _s.Save();
                _s.RaiseForceExtractChanged();
            }
        }

        public bool AutoTrashAfterExtract
        {
            get => _s.AutoTrashAfterExtract;
            set { _s.AutoTrashAfterExtract = value; OnPropertyChanged(); _s.Save(); }
        }

        public bool LockDetection
        {
            get => _s.LockDetection;
            set { _s.LockDetection = value; OnPropertyChanged(); _s.Save(); }
        }

        public bool ToastNotifications
        {
            get => _s.ToastNotifications;
            set { _s.ToastNotifications = value; OnPropertyChanged(); _s.Save(); }
        }

        public int Theme
        {
            get => _s.Theme;
            set
            {
                _s.Theme = value;
                OnPropertyChanged();
                _s.Save();
                ApplyTheme(value);
            }
        }

        public string Language
        {
            get => _s.Language;
            set
            {
                _s.Language = value;
                OnPropertyChanged();
                _s.Save();
                LocalizationManager.SetLanguage(value);
            }
        }

        public System.Collections.IEnumerable AvailableLanguages
            => LocalizationManager.SupportedLanguages;

        public System.Collections.IEnumerable InstalledPlugins
            => Plugin.PluginRegistry.Plugins;

        public bool HasPlugins => Plugin.PluginRegistry.Plugins.Count > 0;
        public bool HasNoPlugins => !HasPlugins;

        [RelayCommand]
        private void OpenPluginsFolder()
        {
            try
            {
                var dir = Plugin.PluginRegistry.PluginsDir;
                if (!Directory.Exists(dir))
                {
                    Directory.CreateDirectory(dir);
                }
                System.Diagnostics.Process.Start("explorer.exe", dir);
            }
            catch { }
        }

        [RelayCommand]
        private void ReloadPlugins()
        {
            Plugin.PluginRegistry.Reload();
            OnPropertyChanged(nameof(InstalledPlugins));
            OnPropertyChanged(nameof(HasPlugins));
            OnPropertyChanged(nameof(HasNoPlugins));
        }

        // ── Appearance: Backdrop ──────────────────────────────────────────────

        public int BackdropType
        {
            get => _s.BackdropType;
            set
            {
                if (!BackdropSwitcher.IsSupported(value))
                {
                    _s.BackdropType = 0;
                    OnPropertyChanged();
                    _s.Save();
                    BackdropFallbackMessage = "此效果需要 Windows 11，已切換為「無」";
                    return;
                }
                _s.BackdropType = value;
                OnPropertyChanged();
                _s.Save();
                try
                {
                    BackdropSwitcher.Apply(value, System.Windows.Application.Current?.MainWindow);
                }
                catch (InvalidOperationException)
                {
                    // Cross-thread access to Application.MainWindow — expected in some contexts.
                }
                BackdropFallbackMessage = null;
            }
        }

        [ObservableProperty] private string? _backdropFallbackMessage;

        public bool HasBackdropFallback => !string.IsNullOrEmpty(BackdropFallbackMessage);

        partial void OnBackdropFallbackMessageChanged(string? value)
        {
            OnPropertyChanged(nameof(HasBackdropFallback));
        }

        /// <summary>已載入的自訂主題檔案數量</summary>
        public int ThemeFileCount => ThemeLoader.Instance.LoadedCount;

        [RelayCommand]
        private void OpenThemesFolder()
            => System.Diagnostics.Process.Start("explorer.exe", ThemeLoader.ThemesFolder);

        [RelayCommand]
        private void OpenIconsFolder()
            => System.Diagnostics.Process.Start("explorer.exe", IconResolver.IconsFolder);

        // ── Zip Bomb Protection Thresholds ────────────────────────────────────

        [ObservableProperty] private string? _zipBombValidationError;

        public bool HasZipBombValidationError => !string.IsNullOrEmpty(ZipBombValidationError);

        partial void OnZipBombValidationErrorChanged(string? value)
        {
            OnPropertyChanged(nameof(HasZipBombValidationError));
        }

        public double ZipBombMaxTotalGb
        {
            get => _s.ZipBombThresholds.MaxTotalGb;
            set
            {
                if (double.IsNaN(value) || value < 1 || value > 1000)
                {
                    ZipBombValidationError = (double.IsNaN(value) || value < 1)
                        ? "請輸入有效的大小上限（最小 1 GB）"
                        : "請輸入有效的大小上限（1–1000 GB）";
                    return;
                }
                ZipBombValidationError = null;
                _s.ZipBombThresholds.MaxTotalGb = value;
                OnPropertyChanged();
                _s.Save();
            }
        }

        public double ZipBombMaxSingleEntryGb
        {
            get => _s.ZipBombThresholds.MaxSingleEntryGb;
            set
            {
                if (double.IsNaN(value) || value < 1 || value > 500)
                {
                    ZipBombValidationError = (double.IsNaN(value) || value < 1)
                        ? "請輸入有效的大小上限（最小 1 GB）"
                        : "請輸入有效的大小上限（1–500 GB）";
                    return;
                }
                ZipBombValidationError = null;
                _s.ZipBombThresholds.MaxSingleEntryGb = value;
                OnPropertyChanged();
                _s.Save();
            }
        }

        public double ZipBombMaxCompressionRatio
        {
            get => _s.ZipBombThresholds.MaxCompressionRatio;
            set
            {
                if (double.IsNaN(value) || value < 10 || value > 10000)
                {
                    ZipBombValidationError = (double.IsNaN(value) || value < 1)
                        ? "請輸入有效的大小上限（最小 1 GB）"
                        : "請輸入有效的壓縮比上限（10–10000）";
                    return;
                }
                ZipBombValidationError = null;
                _s.ZipBombThresholds.MaxCompressionRatio = value;
                OnPropertyChanged();
                _s.Save();
            }
        }

        public int ZipBombMaxNestingDepth
        {
            get => _s.ZipBombThresholds.MaxNestingDepth;
            set
            {
                if (value < 1 || value > 10)
                {
                    ZipBombValidationError = (value < 1)
                        ? "請輸入有效的大小上限（最小 1 GB）"
                        : "請輸入有效的嵌套深度上限（1–10）";
                    return;
                }
                ZipBombValidationError = null;
                _s.ZipBombThresholds.MaxNestingDepth = value;
                OnPropertyChanged();
                _s.Save();
            }
        }

        [RelayCommand]
        private void ResetZipBombDefaults()
        {
            _s.ZipBombThresholds = new ZipBombThresholdsSettings();
            ZipBombValidationError = null;
            OnPropertyChanged(nameof(ZipBombMaxTotalGb));
            OnPropertyChanged(nameof(ZipBombMaxSingleEntryGb));
            OnPropertyChanged(nameof(ZipBombMaxCompressionRatio));
            OnPropertyChanged(nameof(ZipBombMaxNestingDepth));
            _s.Save();
        }

        // ── Shell Extension Integration ──────────────────────────────────────────

        private readonly RegistrationManager _registrationManager = new();

        /// <summary>
        /// Localized status text for the shell extension (e.g. "✅ 已啟用").
        /// </summary>
        public string ShellExtensionStatusText => _s.ShellStatus switch
        {
            ShellExtensionStatus.Enabled  => LocalizationManager.Get("Settings_Shell_Enabled"),
            ShellExtensionStatus.Failed   => LocalizationManager.Get("Settings_Shell_Failed"),
            _                             => LocalizationManager.Get("Settings_Shell_Disabled"),
        };

        /// <summary>
        /// Whether the shell extension is currently enabled (controls button visibility).
        /// </summary>
        public bool IsShellExtensionEnabled => _s.ShellStatus == ShellExtensionStatus.Enabled;

        [RelayCommand]
        private async Task RegisterShellAsync()
        {
            var result = await _registrationManager.RegisterAsync();
            _s.ShellStatus = result.Success ? ShellExtensionStatus.Enabled : ShellExtensionStatus.Failed;
            _s.ShellRegistrationError = result.ErrorMessage;
            _s.Save();
            OnPropertyChanged(nameof(ShellExtensionStatusText));
            OnPropertyChanged(nameof(IsShellExtensionEnabled));
        }

        [RelayCommand]
        private async Task UnregisterShellAsync()
        {
            var result = await _registrationManager.UnregisterAsync();
            _s.ShellStatus = result.Success ? ShellExtensionStatus.Disabled : ShellExtensionStatus.Failed;
            _s.ShellRegistrationError = result.ErrorMessage;
            _s.Save();
            OnPropertyChanged(nameof(ShellExtensionStatusText));
            OnPropertyChanged(nameof(IsShellExtensionEnabled));
        }

        private static void ApplyTheme(int theme)
        {
            ApplicationTheme t = theme switch
            {
                1 => ApplicationTheme.Light,
                2 => ApplicationTheme.Dark,
                _ => ApplicationTheme.Unknown   // Unknown = follow system
            };
            ApplicationThemeManager.Apply(t);
        }
    }
}
