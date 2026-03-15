using System.Windows;
using WpfApplication = System.Windows.Application;

namespace ZipEase.UI.Core
{
    public partial class PasswordDialog : Window
    {
        public string? Password { get; private set; }
        public bool WasCancelled { get; private set; } = true;

        public PasswordDialog(string? errorMessage = null)
        {
            InitializeComponent();
            if (!string.IsNullOrEmpty(errorMessage))
            {
                ErrorText.Text = errorMessage;
                ErrorText.Visibility = Visibility.Visible;
            }

            // Set owner to main window for modality
            if (WpfApplication.Current?.MainWindow != null && WpfApplication.Current.MainWindow != this)
                Owner = WpfApplication.Current.MainWindow;
        }

        private void OnConfirm(object sender, RoutedEventArgs e)
        {
            Password = PasswordBox.Password;
            WasCancelled = false;
            DialogResult = true;
        }

        private void OnCancel(object sender, RoutedEventArgs e)
        {
            WasCancelled = true;
            DialogResult = false;
        }
    }
}
