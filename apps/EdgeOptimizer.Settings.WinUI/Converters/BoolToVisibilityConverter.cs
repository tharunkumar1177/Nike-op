using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Data;

namespace EdgeOptimizer.Settings.WinUI.Converters;

/// <summary>
/// Maps <c>true</c> to <see cref="Visibility.Visible"/> for <c>{Binding}</c> paths, which, unlike
/// <c>x:Bind</c>, do not convert booleans implicitly. Set <see cref="Invert"/> for the opposite mapping.
/// </summary>
public sealed partial class BoolToVisibilityConverter : IValueConverter
{
    public bool Invert { get; set; }

    public object Convert(object value, Type targetType, object parameter, string language) =>
        (value is true) != Invert ? Visibility.Visible : Visibility.Collapsed;

    public object ConvertBack(object value, Type targetType, object parameter, string language) =>
        (value is Visibility.Visible) != Invert;
}
