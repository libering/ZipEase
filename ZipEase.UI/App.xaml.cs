using System.Configuration;
using System.Data;
using System.IO;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Media;
using Wpf.Ui;
using Wpf.Ui.Appearance;
using ZipEase.UI.Core;

namespace ZipEase.UI;

public partial class App : System.Windows.Application
{
    /// <summary>
    /// Resource key for the light-mode card background override.
    /// Provides visual separation between CardControl and window background in Light theme.
    /// </summary>
    private const string CardBackgroundBrushKey = "CardBackgroundFillColorDefaultBrush";

    /// <summary>
    /// Light-mode card background color (#F5F5F5) — a light gray that is visually distinct
    /// from the white window background.
    /// </summary>
    private static readonly SolidColorBrush LightModeCardBrush =
        new(System.Windows.Media.Color.FromRgb(0xF5, 0xF5, 0xF5));

    /// <summary>
    /// Stores the parsed command-line result so MainWindow can access it on load.
    /// </summary>
    internal static CommandLineParser.ParseResult? StartupParseResult { get; private set; }

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

#if DEBUG_CONSOLE
        // Debug mode: allocate a console window so Rust's eprintln! output is visible.
        // Also tee stderr to a log file for post-mortem analysis.
        DebugConsole.Attach();
#endif

        // Initialise localisation before any UI is created
        LocalizationManager.SetLanguage(AppSettings.Instance.Language);

        // Dynamic Theming initialization
        ThemeLoader.Initialize();
        IconResolver.Initialize();
        // BackdropSwitcher.Apply() is called after MainWindow is created

        // Light-mode card background override: apply on startup and listen for theme changes.
        ApplyCardBackgroundForCurrentTheme();
        ApplicationThemeManager.Changed += OnApplicationThemeChanged;

        // Verify required dependencies are present
        string appDirectory = AppDomain.CurrentDomain.BaseDirectory;
        string coreLibPath = Path.Combine(appDirectory, "zipease_core.dll");

        if (!File.Exists(coreLibPath))
        {
            System.Windows.MessageBox.Show(
                "Missing required dependency: zipease_core.dll\n\n" +
                "The application cannot start because zipease_core.dll is missing from the application directory.\n" +
                "Please ensure the application is properly installed.",
                "Missing Dependency",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            Shutdown(1);
            return;
        }

        // Parse command-line arguments (skip first arg which is the executable path)
        var rawArgs = Environment.GetCommandLineArgs().Skip(1).ToArray();
        var parseResult = CommandLineParser.Parse(rawArgs);

        switch (parseResult.Mode)
        {
            case CommandLineParser.Mode.RegisterShell:
                {
                    var regManager = new RegistrationManager();
                    var result = regManager.RegisterAsync().GetAwaiter().GetResult();
                    AppSettings.Instance.ShellStatus = result.Success
                        ? ShellExtensionStatus.Enabled
                        : ShellExtensionStatus.Failed;
                    AppSettings.Instance.ShellRegistrationError = result.ErrorMessage;
                    AppSettings.Instance.Save();
                    Shutdown(result.Success ? 0 : 1);
                    return;
                }

            case CommandLineParser.Mode.UnregisterShell:
                {
                    var regManager = new RegistrationManager();
                    var result = regManager.UnregisterAsync().GetAwaiter().GetResult();
                    AppSettings.Instance.ShellStatus = result.Success
                        ? ShellExtensionStatus.Disabled
                        : ShellExtensionStatus.Failed;
                    AppSettings.Instance.ShellRegistrationError = result.ErrorMessage;
                    AppSettings.Instance.Save();
                    Shutdown(result.Success ? 0 : 1);
                    return;
                }

            case CommandLineParser.Mode.Extract:
            case CommandLineParser.Mode.Compress:
                // Store the result so MainWindow can pick it up and load files / navigate
                StartupParseResult = parseResult;
                break;

            case CommandLineParser.Mode.Normal:
            default:
                // Normal startup — nothing extra to do
                break;
        }

        // First-launch auto-registration: if shell extension was never registered and no error,
        // attempt registration in background so context menu is available immediately.
        if (AppSettings.Instance.ShellStatus == ShellExtensionStatus.Disabled
            && string.IsNullOrEmpty(AppSettings.Instance.ShellRegistrationError))
        {
            _ = Task.Run(async () =>
            {
                try
                {
                    var regManager = new RegistrationManager();
                    var result = await regManager.RegisterAsync();
                    AppSettings.Instance.ShellStatus = result.Success
                        ? ShellExtensionStatus.Enabled
                        : ShellExtensionStatus.Failed;
                    AppSettings.Instance.ShellRegistrationError = result.ErrorMessage;
                    AppSettings.Instance.Save();
                }
                catch
                {
                    // Best-effort — don't crash the app on first-launch registration failure
                }
            });
        }
    }

    /// <summary>
    /// Handles theme changes from WPF-UI's ApplicationThemeManager.
    /// Adds or removes the light-mode card background override as needed.
    /// </summary>
    private void OnApplicationThemeChanged(ApplicationTheme currentTheme, System.Windows.Media.Color systemAccent)
    {
        ApplyCardBackgroundForCurrentTheme();
    }

    /// <summary>
    /// Applies or removes the CardBackgroundFillColorDefaultBrush override based on the current theme.
    /// Light mode: adds #F5F5F5 brush for visual separation from window background.
    /// Dark mode: removes the override so the default dark theme brush is used.
    /// </summary>
    private static void ApplyCardBackgroundForCurrentTheme()
    {
        var theme = ApplicationThemeManager.GetAppTheme();
        var resources = System.Windows.Application.Current.Resources;

        if (theme == ApplicationTheme.Light)
        {
            // Add light-mode card background override
            resources[CardBackgroundBrushKey] = LightModeCardBrush;
        }
        else
        {
            // Remove override so dark theme default is used
            resources.Remove(CardBackgroundBrushKey);
        }
    }
}

