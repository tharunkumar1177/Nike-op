using System.Windows.Input;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.ViewModels;

public sealed class DashboardViewModel : ObservableObject
{
    private const int SetupStepCount = 3;
    private ProfileWorkspace? _profile;

    public DashboardViewModel(Action<string> navigate, ICommand? activateCommand = null)
    {
        NavigateCommand = new RelayCommand<string>(page =>
        {
            if (!string.IsNullOrWhiteSpace(page))
            {
                navigate(page);
            }
        });
        ActivateCommand = activateCommand ?? new RelayCommand(() => { }, () => false);
    }

    public ICommand NavigateCommand { get; }
    public ICommand ActivateCommand { get; }
    public ProfileWorkspace? SelectedProfile => _profile;
    public int SelectedAppCount => _profile?.Processes.Count(process => process.IsSelected) ?? 0;
    public bool HasSelectedApps => SelectedAppCount > 0;
    public bool HasCrosshair => _profile is { OverlayEnabled: true, HasCrosshairImage: true };
    public string CrosshairStatus => HasCrosshair ? "Enabled" : _profile?.OverlayEnabled == true ? "No image" : "Off";
    public string MacroShortcut => AssignedMacro?.Shortcut ?? MacroValidation.Unassigned;
    public bool HasMacroShortcut => AssignedMacro is not null;
    public int MacroCount => _profile?.Macros.Count ?? 0;
    public int CompletedSetupSteps => (HasSelectedApps ? 1 : 0) + (HasCrosshair ? 1 : 0) + (HasMacroShortcut ? 1 : 0);
    public double SetupProgress => 100d * CompletedSetupSteps / SetupStepCount;
    public string SetupProgressLabel => $"{CompletedSetupSteps} of {SetupStepCount} set up";
    public bool IsReady => _profile is not null && CompletedSetupSteps == SetupStepCount;
    public string ReadinessLabel => IsReady ? "Ready" : "Setup required";

    private MacroDefinition? AssignedMacro =>
        _profile?.Macros.FirstOrDefault(macro => macro.IsEnabled && MacroValidation.IsShortcutAssigned(macro.Shortcut));

    public void LoadProfile(ProfileWorkspace profile)
    {
        _profile = profile;
        OnPropertyChanged(string.Empty);
    }
}
