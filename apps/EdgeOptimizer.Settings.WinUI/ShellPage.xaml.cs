using System.ComponentModel;
using EdgeOptimizer.Settings.Core.ViewModels;
using EdgeOptimizer.Settings.WinUI.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace EdgeOptimizer.Settings.WinUI;

public sealed partial class ShellPage : Page
{
    private readonly MainWindowViewModel _viewModel;
    public UIElement DragRegion => MainTitleBarDragRegion;

    public ShellPage(MainWindowViewModel viewModel)
    {
        InitializeComponent();
        _viewModel = viewModel;
        DataContext = viewModel;
        _viewModel.PropertyChanged += ViewModelPropertyChanged;
        var version = typeof(ShellPage).Assembly.GetName().Version;
        VersionText.Text = version is null ? string.Empty : $"Version {version.Major}.{version.Minor}.{Math.Max(version.Build, 0)}";
        RenderPage(_viewModel.CurrentPageLabel);
    }

    private void OnNavigateClick(object sender, RoutedEventArgs args)
    {
        if (sender is Button { Tag: string target }) _viewModel.NavigateTo(target);
    }

    private void OnDuplicateProfileClick(object sender, RoutedEventArgs args) =>
        _viewModel.DuplicateProfileCommand.Execute(null);

    private async void OnDeleteProfileClick(object sender, RoutedEventArgs args)
    {
        var profile = _viewModel.SelectedProfile;
        if (profile is null) return;
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = $"Delete {profile.Name}?",
            Content = profile.IsActive
                ? "This profile is active. Deleting it removes its apps, crosshair, and macros from Runner. This cannot be undone."
                : "This removes the profile's apps, crosshair, and macros from Runner. This cannot be undone.",
            PrimaryButtonText = "Delete",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary && _viewModel.SelectedProfile == profile)
            await _viewModel.DeleteProfileCommand.ExecuteAsync(null);
    }

    private void ViewModelPropertyChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (args.PropertyName == nameof(MainWindowViewModel.CurrentPageLabel)) RenderPage(_viewModel.CurrentPageLabel);
    }

    private void RenderPage(string target)
    {
        FrameworkElement page = target switch
        {
            "Crosshair" => new CrosshairView(),
            "Macros" => new MacrosView(),
            "System Tweaks" => new SystemTweaksView(),
            _ => new DashboardView(),
        };
        page.DataContext = _viewModel.CurrentPage;
        PageHost.Content = page;
        UpdateNavigationSelection();
    }

    private void UpdateNavigationSelection()
    {
        var resources = Application.Current.Resources;
        var selected = (Style)resources["SidebarButtonSelectedStyle"];
        var normal = (Style)resources["SidebarButtonStyle"];
        foreach (var (button, isSelected) in new[]
        {
            (NavDashboard, _viewModel.IsDashboardSelected),
            (NavCrosshair, _viewModel.IsCrosshairSelected),
            (NavMacros, _viewModel.IsMacrosSelected),
            (NavSystemTweaks, _viewModel.IsSystemTweaksSelected),
        })
        {
            button.Style = isSelected ? selected : normal;
            AutomationProperties.SetItemStatus(button, isSelected ? "Current page" : string.Empty);
        }
    }
}
