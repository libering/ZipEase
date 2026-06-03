using System;
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Converts a boolean to Visibility: true → Collapsed, false → Visible.
    /// Inverse of the built-in BooleanToVisibilityConverter.
    /// </summary>
    public class InverseBoolToVisibilityConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
            => value is bool b && b ? Visibility.Collapsed : Visibility.Visible;

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
            => value is Visibility v && v != Visibility.Visible;
    }
}
