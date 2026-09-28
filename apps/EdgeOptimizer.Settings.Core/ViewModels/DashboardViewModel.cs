using System.Windows.Input;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.ViewModels;

public sealed class DashboardViewModel : ObservableObject
{
    private ProfileWorkspace? _profile;

    public DashboardViewModel(Action<string> navigate)
    {
        NavigateCommand = new RelayCommand<string>(page =>
        {
            if (!string.IsNullOrWhiteSpace(page))
            {
                navigate(page);
            }
        });
    }

    public ICommand NavigateCommand { get; }
    public ProfileWorkspace? SelectedProfile => _profile;
    public int SelectedAppCount => _profile?.Processes.Count(process => process.IsSelected) ?? 0;
    public string CrosshairStatus => _profile?.OverlayEnabled == true && _profile.CrosshairImageName != "No image selected" ? "Enabled" : "Not configured";
    public string MacroShortcut => _profile?.Macros.FirstOrDefault()?.Shortcut ?? "Unassigned";
    public bool IsReady => _profile is not null && SelectedAppCount > 0 && CrosshairStatus == "Enabled" && MacroShortcut != "Unassigned";
    public string ReadinessLabel => IsReady ? "Ready" : "Setup required";

    public void LoadProfile(ProfileWorkspace profile)
    {
        _profile = profile;
        OnPropertyChanged(nameof(SelectedProfile));
        OnPropertyChanged(nameof(SelectedAppCount));
        OnPropertyChanged(nameof(CrosshairStatus));
        OnPropertyChanged(nameof(MacroShortcut));
        OnPropertyChanged(nameof(IsReady));
        OnPropertyChanged(nameof(ReadinessLabel));
    }
}
