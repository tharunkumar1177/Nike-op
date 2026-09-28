using EdgeOptimizer.Settings.Core.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace EdgeOptimizer.Settings.WinUI.Views;

public partial class SystemTweaksView : UserControl
{
    public SystemTweaksView()
    {
        InitializeComponent();
        Loaded += OnLoaded;
    }

    private void OnLoaded(object sender, RoutedEventArgs args)
    {
        if (DataContext is SystemTweaksViewModel viewModel && viewModel.RefreshCommand.CanExecute(null))
            _ = viewModel.RefreshCommand.ExecuteAsync(null);
    }
}
