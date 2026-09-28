using System.Collections.ObjectModel;
using System.Windows.Input;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.Services;

namespace EdgeOptimizer.Settings.Core.ViewModels;

public sealed class MainWindowViewModel : ObservableObject
{
    private readonly IRunnerClient _runnerClient;
    private readonly IProcessSource? _processSource;
    private readonly IEngineClient? _engineClient;
    private ProfileWorkspace? _selectedProfile;
    private object? _currentPage;
    private string _currentPageLabel = "Dashboard";
    private string _currentPageTitle = "Gaming dashboard";
    private string _statusMessage = "Connecting to Runner…";
    private bool _isConnecting;
    private bool _hasHydrated;

    /// <param name="processSource">Local process enumeration; absent where it is unsupported.</param>
    /// <param name="engineClient">EngineSvc; absent in editions that ship without it.</param>
    public MainWindowViewModel(IFilePicker filePicker, IRunnerClient runnerClient, IProcessSource? processSource = null, IEngineClient? engineClient = null)
    {
        _runnerClient = runnerClient;
        _processSource = processSource;
        _engineClient = engineClient;
        NavigateCommand = new RelayCommand<string>(NavigateTo);
        NewProfileCommand = new RelayCommand(NewProfile);
        DuplicateProfileCommand = new RelayCommand(DuplicateProfile, () => SelectedProfile is not null);
        DeleteProfileCommand = new AsyncRelayCommand(DeleteProfileAsync, () => SelectedProfile is not null);
        SaveCommand = new AsyncRelayCommand(SaveProfilesAsync);
        ActivateProfileCommand = new AsyncRelayCommand(ActivateProfileAsync, () => SelectedProfile is not null && ActivationEnabled);
        ReconnectCommand = new AsyncRelayCommand(() => InitializeAsync(), () => !IsRunnerConnected && !IsConnecting);
        Dashboard = new DashboardViewModel(NavigateTo, ActivateProfileCommand);
        Crosshair = new CrosshairViewModel(filePicker, SaveProfilesAsync);
        Macros = new MacrosViewModel(SaveProfilesAsync);
        SystemTweaks = new SystemTweaksViewModel(SaveProfilesAsync, RequestCleanupAsync, processSource);

        _runnerClient.ConnectionChanged += (_, connected) =>
        {
            NotifyConnectionState();
            StatusMessage = connected ? "Connected to Runner. Loading profiles…" : "Runner is unavailable. Changes cannot be saved until it reconnects.";
        };
        _runnerClient.SnapshotReceived += (_, snapshot) => ApplySnapshot(snapshot);
        _runnerClient.StatusReceived += (_, status) => StatusMessage = status;
        _runnerClient.ActiveProfileChanged += (_, activeName) => ApplyActiveProfile(activeName);

        NavigateTo("Dashboard");
    }

    public ObservableCollection<ProfileWorkspace> Profiles { get; } = new();
    public DashboardViewModel Dashboard { get; }
    public CrosshairViewModel Crosshair { get; }
    public MacrosViewModel Macros { get; }
    public SystemTweaksViewModel SystemTweaks { get; }
    public ICommand NavigateCommand { get; }
    public ICommand NewProfileCommand { get; }
    public ICommand DuplicateProfileCommand { get; }
    public IAsyncRelayCommand DeleteProfileCommand { get; }
    public ICommand SaveCommand { get; }
    public ICommand ActivateProfileCommand { get; }
    public ICommand ReconnectCommand { get; }

    public ProfileWorkspace? SelectedProfile
    {
        get => _selectedProfile;
        set
        {
            if (!SetProperty(ref _selectedProfile, value)) return;
            OnPropertyChanged(nameof(HasSelectedProfile));
            OnPropertyChanged(nameof(ShowEmptyState));
            OnPropertyChanged(nameof(EmptyStateTitle));
            OnPropertyChanged(nameof(EmptyStateMessage));
            OnPropertyChanged(nameof(IsSelectedProfileActive));
            NotifyCommandState();
            if (value is null) return;
            Dashboard.LoadProfile(value);
            Crosshair.LoadProfile(value);
            Macros.LoadProfile(value);
            SystemTweaks.LoadProfile(value);
            StatusMessage = $"Loaded {value.Name}.";
        }
    }

    public object? CurrentPage { get => _currentPage; private set => SetProperty(ref _currentPage, value); }
    public string CurrentPageLabel { get => _currentPageLabel; private set => SetProperty(ref _currentPageLabel, value); }
    public string CurrentPageTitle { get => _currentPageTitle; private set => SetProperty(ref _currentPageTitle, value); }
    public string StatusMessage { get => _statusMessage; private set => SetProperty(ref _statusMessage, value); }
    public bool ActivationEnabled => _runnerClient.IsConnected;
    public bool IsRunnerConnected => _runnerClient.IsConnected;
    public bool IsRunnerOffline => !_runnerClient.IsConnected && !IsConnecting;
    public string RunnerStatusLabel => IsConnecting ? "Connecting…" : IsRunnerConnected ? "Runner linked" : "Runner offline";
    public bool HasSelectedProfile => SelectedProfile is not null;
    public bool IsSelectedProfileActive => SelectedProfile?.IsActive == true;
    public bool ShowEmptyState => SelectedProfile is null;
    public string EmptyStateTitle => IsRunnerConnected || _hasHydrated ? "No profiles yet" : IsConnecting ? "Connecting to Runner" : "Runner is not available";
    public string EmptyStateMessage => IsRunnerConnected || _hasHydrated
        ? "Create a profile to choose the apps to close, a crosshair, and macros for a game."
        : IsConnecting
            ? "Loading your profiles…"
            : "Profiles are stored by Runner. Make sure Edge Optimizer is running, then retry.";
    public bool IsDashboardSelected => CurrentPageLabel == "Dashboard";
    public bool IsCrosshairSelected => CurrentPageLabel == "Crosshair";
    public bool IsMacrosSelected => CurrentPageLabel == "Macros";
    public bool IsSystemTweaksSelected => CurrentPageLabel == "System Tweaks";

    public bool IsConnecting
    {
        get => _isConnecting;
        private set { if (SetProperty(ref _isConnecting, value)) NotifyConnectionState(); }
    }

    public async Task InitializeAsync(CancellationToken cancellationToken = default)
    {
        if (IsConnecting || _runnerClient.IsConnected) return;
        IsConnecting = true;
        StatusMessage = "Connecting to Runner…";
        try { await _runnerClient.StartAsync(cancellationToken); }
        catch (Exception error) { StatusMessage = $"Could not connect to Runner: {error.Message}"; }
        finally { IsConnecting = false; }
    }

    public void NavigateTo(string? page)
    {
        switch (page)
        {
            case "Crosshair": CurrentPage = Crosshair; CurrentPageLabel = "Crosshair"; CurrentPageTitle = "Crosshair overlay"; break;
            case "Macros": CurrentPage = Macros; CurrentPageLabel = "Macros"; CurrentPageTitle = "Macro editor"; break;
            case "SystemTweaks": CurrentPage = SystemTweaks; CurrentPageLabel = "System Tweaks"; CurrentPageTitle = "System tweaks"; break;
            default: CurrentPage = Dashboard; CurrentPageLabel = "Dashboard"; CurrentPageTitle = "Gaming dashboard"; break;
        }
        OnPropertyChanged(nameof(IsDashboardSelected));
        OnPropertyChanged(nameof(IsCrosshairSelected));
        OnPropertyChanged(nameof(IsMacrosSelected));
        OnPropertyChanged(nameof(IsSystemTweaksSelected));
        SystemTweaks.SetPageActive(IsSystemTweaksSelected);
    }

    /// <summary>Forward window activation so background work idles while Settings is not in use.</summary>
    public void SetWindowActive(bool active) => SystemTweaks.SetWindowActive(active);

    private void ApplySnapshot(RunnerSnapshot snapshot)
    {
        _hasHydrated = true;
        var previous = SelectedProfile?.Name;
        Profiles.Clear();
        foreach (var profile in snapshot.Profiles)
        {
            profile.IsActive = string.Equals(profile.Name, snapshot.ActiveProfileName, StringComparison.OrdinalIgnoreCase);
            Profiles.Add(profile);
        }
        SelectedProfile = Profiles.FirstOrDefault(profile => string.Equals(profile.Name, previous, StringComparison.OrdinalIgnoreCase))
            ?? Profiles.FirstOrDefault(profile => profile.IsActive)
            ?? Profiles.FirstOrDefault();
        OnPropertyChanged(nameof(EmptyStateTitle));
        OnPropertyChanged(nameof(EmptyStateMessage));
        StatusMessage = Profiles.Count == 0
            ? "Runner has no profiles yet. Create one to get started."
            : $"Loaded {Profiles.Count} profile(s) from Runner.";
    }

    private void ApplyActiveProfile(string? activeName)
    {
        foreach (var profile in Profiles)
            profile.IsActive = string.Equals(profile.Name, activeName, StringComparison.OrdinalIgnoreCase);
        OnPropertyChanged(nameof(IsSelectedProfileActive));
    }

    private void NewProfile()
    {
        var profile = new ProfileWorkspace(GetUniqueProfileName("New profile"), false);
        Profiles.Add(profile);
        SelectedProfile = profile;
        StatusMessage = "New profile created. Save changes to persist it.";
    }

    private void DuplicateProfile()
    {
        if (SelectedProfile is null) return;
        var copy = ProfileWorkspaceCopy.Create(SelectedProfile, GetUniqueProfileName($"{SelectedProfile.Name} copy"));
        Profiles.Add(copy);
        SelectedProfile = copy;
        StatusMessage = $"Created {copy.Name}. Save changes to persist it.";
    }

    private async Task DeleteProfileAsync()
    {
        if (SelectedProfile is null) return;
        var name = SelectedProfile.Name;
        var index = Profiles.IndexOf(SelectedProfile);
        Profiles.Remove(SelectedProfile);
        SelectedProfile = Profiles.Count == 0 ? null : Profiles[Math.Clamp(index, 0, Profiles.Count - 1)];
        if (await SaveProfilesAsync()) StatusMessage = $"Deleted {name}.";
    }

    private async Task<bool> SaveProfilesAsync()
    {
        if (!_runnerClient.IsConnected) { StatusMessage = "Runner is unavailable; changes were not saved."; return false; }
        try
        {
            await _runnerClient.SaveProfilesAsync(Profiles.ToArray());
            StatusMessage = "Changes saved to Runner.";
            return true;
        }
        catch (Exception error) when (IsRunnerFailure(error))
        {
            StatusMessage = $"Changes were not saved: {error.Message}";
            return false;
        }
    }

    private async Task ActivateProfileAsync()
    {
        if (SelectedProfile is null || !_runnerClient.IsConnected) return;
        var profile = SelectedProfile;
        if (!await SaveProfilesAsync()) return;
        var closeSummary = await CloseSelectedAppsAsync(profile);
        SystemTweaks.ReportTermination(closeSummary);
        try
        {
            await _runnerClient.ActivateProfileAsync(profile);
            StatusMessage = $"{closeSummary} Activating {profile.Name}…";
        }
        catch (Exception error) when (IsRunnerFailure(error))
        {
            StatusMessage = $"{closeSummary} Could not activate {profile.Name}: {error.Message}";
        }
    }

    /// <summary>
    /// Resolves the profile's selected apps to the instances running now and asks
    /// EngineSvc to close them. EngineSvc re-validates each PID before terminating.
    /// </summary>
    private async Task<string> CloseSelectedAppsAsync(ProfileWorkspace profile)
    {
        var selected = profile.Processes.Where(process => process.IsSelected).Select(process => process.Name).ToArray();
        if (selected.Length == 0) return "No apps are selected to close.";
        if (_processSource is null || _engineClient is null) return "Apps were not closed because the engine service is not available.";

        IReadOnlyList<ProcessTarget> targets;
        try
        {
            var source = _processSource;
            var snapshot = await Task.Run(() => source.Snapshot());
            targets = TerminationPlanner.Plan(selected, snapshot, source.CurrentSessionId, source.CurrentProcessId);
        }
        catch (Exception error) when (error is not OperationCanceledException)
        {
            return $"Apps were not closed because the running apps could not be read: {error.Message}";
        }
        if (targets.Count == 0) return "None of the selected apps are running.";

        try
        {
            var report = await _engineClient.TerminateAsync(targets);
            return report.Summary;
        }
        catch (EngineUnavailableException error)
        {
            return $"Apps were not closed: {error.Message}";
        }
    }

    private Task<bool> RequestCleanupAsync(string kind) =>
        SendToRunnerAsync(() => _runnerClient.RequestCleanupAsync(kind), "Cleanup request failed");

    private async Task<bool> SendToRunnerAsync(Func<Task> send, string failurePrefix)
    {
        if (!_runnerClient.IsConnected) return false;
        try
        {
            await send();
            return true;
        }
        catch (Exception error) when (IsRunnerFailure(error))
        {
            StatusMessage = $"{failurePrefix}: {error.Message}";
            return false;
        }
    }

    private static bool IsRunnerFailure(Exception error) =>
        error is IOException or InvalidOperationException or InvalidDataException or ObjectDisposedException or OperationCanceledException;

    private string GetUniqueProfileName(string root)
    {
        var candidate = root;
        var suffix = 2;
        while (Profiles.Any(profile => string.Equals(profile.Name, candidate, StringComparison.OrdinalIgnoreCase)))
            candidate = $"{root} {suffix++}";
        return candidate;
    }

    private void NotifyConnectionState()
    {
        OnPropertyChanged(nameof(ActivationEnabled));
        OnPropertyChanged(nameof(IsRunnerConnected));
        OnPropertyChanged(nameof(IsRunnerOffline));
        OnPropertyChanged(nameof(RunnerStatusLabel));
        OnPropertyChanged(nameof(EmptyStateTitle));
        OnPropertyChanged(nameof(EmptyStateMessage));
        NotifyCommandState();
    }

    private void NotifyCommandState()
    {
        ((RelayCommand)DuplicateProfileCommand).NotifyCanExecuteChanged();
        ((AsyncRelayCommand)DeleteProfileCommand).NotifyCanExecuteChanged();
        ((AsyncRelayCommand)ActivateProfileCommand).NotifyCanExecuteChanged();
        ((AsyncRelayCommand)ReconnectCommand).NotifyCanExecuteChanged();
    }
}

internal static class ProfileWorkspaceCopy
{
    public static ProfileWorkspace Create(ProfileWorkspace source, string name)
    {
        var copy = new ProfileWorkspace(name, false)
        {
            OverlayEnabled = source.OverlayEnabled,
            CrosshairImageName = source.CrosshairImageName,
            CrosshairImagePath = source.CrosshairImagePath,
            CrosshairXOffset = source.CrosshairXOffset,
            CrosshairYOffset = source.CrosshairYOffset,
            FanBoostEnabled = source.FanBoostEnabled,
            RecycleBinEnabled = source.RecycleBinEnabled,
            BrowserCacheEnabled = source.BrowserCacheEnabled,
        };
        foreach (var macro in source.Macros)
            copy.Macros.Add(new MacroDefinition(macro.Name, macro.Shortcut, macro.Steps.Select(step => new MacroStep(step.Action, step.Value))) { IsEnabled = macro.IsEnabled, RepeatMode = macro.RepeatMode, RepeatCount = macro.RepeatCount, StopKey = macro.StopKey });
        foreach (var process in source.Processes)
            copy.Processes.Add(new ProcessItem(process.Name, process.Cpu, process.Memory, process.IsSelected));
        return copy;
    }
}
