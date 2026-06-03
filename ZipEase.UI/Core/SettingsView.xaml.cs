using System.Windows.Controls;
using WpfUserControl = System.Windows.Controls.UserControl;

namespace ZipEase.UI.Core
{
    public partial class SettingsView : WpfUserControl
    {
        public SettingsView()
        {
            InitializeComponent();
            DataContext = new SettingsViewModel();
        }
    }
}
