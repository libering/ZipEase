using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using Wpf.Ui.Controls;
using ZipEase.UI.Core;
using DragEventArgs = System.Windows.DragEventArgs;
using DragDropEffects = System.Windows.DragDropEffects;
using DataFormats = System.Windows.DataFormats;
using WpfDataGrid = System.Windows.Controls.DataGrid;

namespace ZipEase.UI;

public partial class MainWindow : FluentWindow
{
    private MainWindowViewModel ViewModel => (MainWindowViewModel)DataContext;

    public MainWindow()
    {
        InitializeComponent();
        DataContext = new MainWindowViewModel(new ArchivePreviewService());
        CompressPanel.DataContext = new CompressViewModel();
    }

    // Tab switching
    private void OnExtractTabClick(object sender, RoutedEventArgs e)
    {
        ExtractPanel.Visibility = Visibility.Visible;
        CompressPanel.Visibility = Visibility.Collapsed;
    }

    private void OnCompressTabClick(object sender, RoutedEventArgs e)
    {
        ExtractPanel.Visibility = Visibility.Collapsed;
        CompressPanel.Visibility = Visibility.Visible;
    }

    private void OnDragEnter(object sender, DragEventArgs e)
    {
        if (CompressPanel.Visibility == Visibility.Visible) { e.Effects = DragDropEffects.None; return; }
        if (ViewModel.CurrentState != UIState.Idle) { e.Effects = DragDropEffects.None; return; }

        if (e.Data.GetDataPresent(DataFormats.FileDrop))
        {
            var files = (string[])e.Data.GetData(DataFormats.FileDrop);
            if (files?.Length > 0 && new ArchivePreviewService().IsSupportedArchive(files[0]))
            {
                e.Effects = DragDropEffects.Copy;
                ViewModel.TransitionToDragOver();
                e.Handled = true;
                return;
            }
        }
        e.Effects = DragDropEffects.None;
        e.Handled = true;
    }

    private void OnDragOver(object sender, DragEventArgs e)
    {
        e.Handled = true;
    }

    private void OnDragLeave(object sender, DragEventArgs e)
    {
        if (ViewModel.CurrentState == UIState.DragOver)
            ViewModel.TransitionToIdle();
    }

    private void OnDrop(object sender, DragEventArgs e)
    {
        if (CompressPanel.Visibility == Visibility.Visible) return;
        if (!e.Data.GetDataPresent(DataFormats.FileDrop)) return;
        var files = (string[])e.Data.GetData(DataFormats.FileDrop);
        if (files?.Length > 0)
            ViewModel.DropCommand.Execute(files[0]);
    }

    private void OnDropZoneClick(object sender, MouseButtonEventArgs e)
    {
        ViewModel.BrowseFileCommand.Execute(null);
    }

    private void OnDataGridDoubleClick(object sender, MouseButtonEventArgs e)
    {
        if (sender is WpfDataGrid grid && grid.SelectedItem is ArchiveEntryViewModel entry)
            ViewModel.NavigateIntoCommand.Execute(entry);
    }

    private System.Windows.Point _dragStartPoint;

    private void OnDataGridPreviewMouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
    {
        _dragStartPoint = e.GetPosition(null);
    }

    private void OnDataGridPreviewMouseMove(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (e.LeftButton != System.Windows.Input.MouseButtonState.Pressed) return;

        var pos = e.GetPosition(null);
        var diff = pos - _dragStartPoint;

        // Only start drag if mouse moved enough to distinguish from a click
        if (Math.Abs(diff.X) < SystemParameters.MinimumHorizontalDragDistance &&
            Math.Abs(diff.Y) < SystemParameters.MinimumVerticalDragDistance)
            return;

        if (ArchiveDataGrid.SelectedItem is not ZipEase.UI.Core.ArchiveEntryViewModel entry)
            return;

        if (entry.IsDirectory) return;

        // Delegate actual extraction + drag to ViewModel command
        if (ViewModel.ExtractSingleEntryCommand.CanExecute(entry))
            ViewModel.ExtractSingleEntryCommand.Execute(entry);
    }
}
