using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Globalization;
using System.Resources;
using System.Runtime.CompilerServices;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Observable localisation provider.
    /// Bind in XAML: Text="{Binding Source={x:Static core:L.Current}, Path=Nav_Extract}"
    /// Use in C#:    LocalizationManager.F("Status_ExtractSuccess", count, dir)
    /// </summary>
    public sealed class LocalizationManager : INotifyPropertyChanged
    {
        // ── Singleton ─────────────────────────────────────────────────────────
        private static LocalizationManager? _current;
        public static LocalizationManager Current => _current ??= new LocalizationManager();

        // ── Language options ──────────────────────────────────────────────────
        public sealed class LanguageOption
        {
            public string Tag { get; }
            public string DisplayName { get; }
            public LanguageOption(string tag, string displayName) { Tag = tag; DisplayName = displayName; }
        }

        public static readonly IReadOnlyList<LanguageOption> SupportedLanguages =
            new List<LanguageOption>
            {
                new("zh-TW", "繁體中文"),
                new("en",    "English"),
            };

        // ── Internal state ────────────────────────────────────────────────────
        private ResourceManager _rm;
        private CultureInfo _culture;

        private LocalizationManager()
        {
            _rm = new ResourceManager("ZipEase.UI.Strings.Strings", typeof(LocalizationManager).Assembly);
            var tag = AppSettings.Instance.Language;
            _culture = tag switch
            {
                "zh-TW" => new CultureInfo("zh-TW"),
                "en"    => new CultureInfo("en"),
                _       => new CultureInfo("en"),
            };
        }

        // ── Language switch ───────────────────────────────────────────────────
        public static void SetLanguage(string tag)
        {
            Current._culture = tag switch
            {
                "zh-TW" => new CultureInfo("zh-TW"),
                "en"    => new CultureInfo("en"),
                _       => new CultureInfo("en"),
            };
            Current.RaiseAllChanged();
        }

        private void RaiseAllChanged() => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(string.Empty));

        // ── INotifyPropertyChanged ────────────────────────────────────────────
        public event PropertyChangedEventHandler? PropertyChanged;

        // ── String accessor (C# code) ─────────────────────────────────────────
        public static string Get(string key)
        {
            var s = Current._rm.GetString(key, Current._culture);
            return s ?? $"[{key}]";
        }

        public static string F(string key, params object[] args)
        {
            var t = Get(key);
            try { return string.Format(t, args); } catch { return t; }
        }

        // ── XAML-bindable properties ──────────────────────────────────────────
        private string G(string key) => Get(key);

        // Sidebar
        public string Nav_Extract   => G("Nav_Extract");
        public string Nav_Compress  => G("Nav_Compress");
        public string Nav_Settings  => G("Nav_Settings");

        // Drop Zone
        public string DropZone_Title    => G("DropZone_Title");
        public string DropZone_Subtitle => G("DropZone_Subtitle");

        // Extract toolbar
        public string Extract_Back_Tooltip      => G("Extract_Back_Tooltip");
        public string Extract_ForceExtract      => G("Extract_ForceExtract");
        public string Extract_ForceExtract_Tooltip => G("Extract_ForceExtract_Tooltip");
        public string Extract_ExtractAll        => G("Extract_ExtractAll");
        public string Extract_ExtractSelected   => G("Extract_ExtractSelected");
        public string Extract_Close             => G("Extract_Close");
        public string Extract_SearchPlaceholder => G("Extract_SearchPlaceholder");
        public string Extract_TrashButton       => G("Extract_TrashButton");

        // DataGrid headers
        public string Column_FileName => G("Column_FileName");
        public string Column_Type     => G("Column_Type");
        public string Column_Size     => G("Column_Size");

        // Compress
        public string Compress_AddFiles           => G("Compress_AddFiles");
        public string Compress_AddFolder          => G("Compress_AddFolder");
        public string Compress_Confirm           => G("Compress_Confirm");
        public string Compress_Reset              => G("Compress_Reset");
        public string Compress_OutputPlaceholder  => G("Compress_OutputPlaceholder");
        public string Compress_Browse             => G("Compress_Browse");
        public string Compress_Level              => G("Compress_Level");
        public string Compress_Password           => G("Compress_Password");
        public string Compress_PasswordPlaceholder => G("Compress_PasswordPlaceholder");
        public string Compress_Start              => G("Compress_Start");

        // Settings
        public string Settings_Section_Extraction  => G("Settings_Section_Extraction");
        public string Settings_ForceExtract_Title  => G("Settings_ForceExtract_Title");
        public string Settings_ForceExtract_Desc   => G("Settings_ForceExtract_Desc");
        public string Settings_Section_Cleanup     => G("Settings_Section_Cleanup");
        public string Settings_AutoTrash_Title     => G("Settings_AutoTrash_Title");
        public string Settings_AutoTrash_Desc      => G("Settings_AutoTrash_Desc");
        public string Settings_LockDetection_Title => G("Settings_LockDetection_Title");
        public string Settings_LockDetection_Desc  => G("Settings_LockDetection_Desc");
        public string Settings_Section_Appearance  => G("Settings_Section_Appearance");
        public string Settings_Toast_Title         => G("Settings_Toast_Title");
        public string Settings_Toast_Desc          => G("Settings_Toast_Desc");
        public string Settings_Theme_Title         => G("Settings_Theme_Title");
        public string Settings_Theme_Desc          => G("Settings_Theme_Desc");
        public string Settings_Theme_System        => G("Settings_Theme_System");
        public string Settings_Theme_Light         => G("Settings_Theme_Light");
        public string Settings_Theme_Dark          => G("Settings_Theme_Dark");
        public string Settings_Section_Language    => G("Settings_Section_Language");
        public string Settings_Language_Title      => G("Settings_Language_Title");
        public string Settings_Language_Desc       => G("Settings_Language_Desc");

        // Password dialog
        public string PasswordDialog_Title        => G("PasswordDialog_Title");
        public string PasswordDialog_Prompt       => G("PasswordDialog_Prompt");
        public string PasswordDialog_Confirm      => G("PasswordDialog_Confirm");
        public string PasswordDialog_Cancel       => G("PasswordDialog_Cancel");
        public string PasswordDialog_WrongPassword => G("PasswordDialog_WrongPassword");

        // Shell Extension Integration
        public string Settings_Integration_Title  => G("Settings_Integration_Title");
        public string Settings_Shell_Enabled      => G("Settings_Shell_Enabled");
        public string Settings_Shell_Disabled     => G("Settings_Shell_Disabled");
        public string Settings_Shell_Failed       => G("Settings_Shell_Failed");
        public string Settings_Shell_Register     => G("Settings_Shell_Register");
        public string Settings_Shell_Unregister   => G("Settings_Shell_Unregister");
    }

    /// <summary>Short alias for use in XAML x:Static.</summary>
    public static class L
    {
        public static LocalizationManager Current => LocalizationManager.Current;
    }
}
