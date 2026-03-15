using System.Configuration;
using System.Data;
using System.IO;
using System.Windows;

namespace ZipEase.UI;

/// <summary>
/// Interaction logic for App.xaml
/// </summary>
public partial class App : System.Windows.Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

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
        }
    }
}

