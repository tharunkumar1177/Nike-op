using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;

namespace EdgeOptimizer.Settings.Core.Models;

/// <summary>
/// Editable profile state. A new instance matches Runner's <c>create_profile</c>
/// defaults: nothing selected for termination, no macros, and no crosshair image.
/// </summary>
public sealed class ProfileWorkspace : ObservableObject
{
    public const string NoImageName = "No image selected";

    private bool _isActive;
    private bool _overlayEnabled = true;
    private string _crosshairImageName = NoImageName;
    private string? _crosshairImagePath;
    private int _crosshairXOffset;
    private int _crosshairYOffset;
    private bool _fanBoostEnabled;
    private bool _recycleBinEnabled;
    private bool _browserCacheEnabled;

    public ProfileWorkspace(string name, bool isActive)
    {
        Name = name;
        _isActive = isActive;
    }

    public string Name { get; }
    public ObservableCollection<MacroDefinition> Macros { get; } = new();
    public ObservableCollection<ProcessItem> Processes { get; } = new();

    public bool IsActive { get => _isActive; set => SetProperty(ref _isActive, value); }
    public bool OverlayEnabled { get => _overlayEnabled; set => SetProperty(ref _overlayEnabled, value); }
    public string CrosshairImageName { get => _crosshairImageName; set => SetProperty(ref _crosshairImageName, value); }
    public string? CrosshairImagePath
    {
        get => _crosshairImagePath;
        set { if (SetProperty(ref _crosshairImagePath, value)) OnPropertyChanged(nameof(HasCrosshairImage)); }
    }
    public bool HasCrosshairImage => !string.IsNullOrWhiteSpace(CrosshairImagePath);
    public int CrosshairXOffset { get => _crosshairXOffset; set => SetProperty(ref _crosshairXOffset, Math.Clamp(value, -500, 500)); }
    public int CrosshairYOffset { get => _crosshairYOffset; set => SetProperty(ref _crosshairYOffset, Math.Clamp(value, -500, 500)); }
    public bool FanBoostEnabled { get => _fanBoostEnabled; set => SetProperty(ref _fanBoostEnabled, value); }
    public bool RecycleBinEnabled { get => _recycleBinEnabled; set => SetProperty(ref _recycleBinEnabled, value); }
    public bool BrowserCacheEnabled { get => _browserCacheEnabled; set => SetProperty(ref _browserCacheEnabled, value); }
}
