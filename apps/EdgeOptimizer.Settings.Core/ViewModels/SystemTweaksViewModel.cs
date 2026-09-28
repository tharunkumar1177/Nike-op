using System.ComponentModel;
using System.Windows.Input;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.ViewModels;

public sealed class SystemTweaksViewModel : ObservableObject
{
    public const string RecycleBinCleanup = "recycle-bin";
    public const string BrowserCacheCleanup = "browser-cache";
    public const string NotRunningMetric = "—";

    private readonly Func<Task<bool>> _saveAsync;
    private readonly Func<string, Task<bool>> _cleanupAsync;
    private readonly Func<Task<bool>> _refreshProcessesAsync;
    private ProfileWorkspace? _profile;
    private string _processFilter = string.Empty;
    private string _feedbackText = "Choose the apps Runner should close when this profile is activated.";

    public SystemTweaksViewModel(Func<Task<bool>>? saveAsync = null, Func<string, Task<bool>>? cleanupAsync = null, Func<Task<bool>>? refreshProcessesAsync = null)
    {
        _saveAsync = saveAsync ?? (() => Task.FromResult(true));
        _cleanupAsync = cleanupAsync ?? (_ => Task.FromResult(true));
        _refreshProcessesAsync = refreshProcessesAsync ?? (() => Task.FromResult(true));
        RefreshCommand = new AsyncRelayCommand(RefreshProcessesAsync);
        RestoreDefaultsCommand = new RelayCommand(RestoreDefaults);
        ClearSelectionCommand = new RelayCommand(ClearSelection);
        SaveCommand = new AsyncRelayCommand(SaveAsync);
        RunRecycleBinCleanupCommand = new AsyncRelayCommand(() => RunCleanupAsync(RecycleBinCleanup, "Recycle Bin"));
        RunBrowserCacheCleanupCommand = new AsyncRelayCommand(() => RunCleanupAsync(BrowserCacheCleanup, "browser cache"));
    }

    public IAsyncRelayCommand RefreshCommand { get; }
    public ICommand RestoreDefaultsCommand { get; }
    public ICommand ClearSelectionCommand { get; }
    public ICommand SaveCommand { get; }
    public IAsyncRelayCommand RunRecycleBinCleanupCommand { get; }
    public IAsyncRelayCommand RunBrowserCacheCleanupCommand { get; }

    public IEnumerable<ProcessItem> FilteredProcesses =>
        (_profile?.Processes.AsEnumerable() ?? Enumerable.Empty<ProcessItem>()).Where(FilterProcess);
    public int SelectedCount => _profile?.Processes.Count(process => process.IsSelected) ?? 0;
    public string SelectionSummary => $"{SelectedCount} selected";
    public bool IsProcessListEmpty => !FilteredProcesses.Any();
    public string ProcessListEmptyText => string.IsNullOrWhiteSpace(ProcessFilter)
        ? "No apps listed yet. Select Refresh to load the apps that are running now."
        : $"No apps match \"{ProcessFilter}\".";

    public string ProcessFilter
    {
        get => _processFilter;
        set
        {
            if (!SetProperty(ref _processFilter, value)) return;
            NotifyProcessList();
        }
    }

    public bool FanBoostEnabled
    {
        get => _profile?.FanBoostEnabled ?? false;
        set { if (_profile is not null && _profile.FanBoostEnabled != value) { _profile.FanBoostEnabled = value; OnPropertyChanged(); } }
    }

    public bool RecycleBinEnabled
    {
        get => _profile?.RecycleBinEnabled ?? false;
        set { if (_profile is not null && _profile.RecycleBinEnabled != value) { _profile.RecycleBinEnabled = value; OnPropertyChanged(); } }
    }

    public bool BrowserCacheEnabled
    {
        get => _profile?.BrowserCacheEnabled ?? false;
        set { if (_profile is not null && _profile.BrowserCacheEnabled != value) { _profile.BrowserCacheEnabled = value; OnPropertyChanged(); } }
    }

    public string FeedbackText { get => _feedbackText; private set => SetProperty(ref _feedbackText, value); }

    public void LoadProfile(ProfileWorkspace profile)
    {
        if (_profile is not null)
        {
            foreach (var process in _profile.Processes) process.PropertyChanged -= ProcessPropertyChanged;
        }
        _profile = profile;
        foreach (var process in profile.Processes) process.PropertyChanged += ProcessPropertyChanged;
        NotifyProcessList();
        OnPropertyChanged(nameof(FanBoostEnabled));
        OnPropertyChanged(nameof(RecycleBinEnabled));
        OnPropertyChanged(nameof(BrowserCacheEnabled));
    }

    /// <summary>
    /// Replaces the listed processes with Runner's snapshot. Selected apps that are not
    /// running stay selected, so a refresh never silently shrinks the profile's close list.
    /// </summary>
    public void ApplyProcessSnapshot(IReadOnlyList<ProcessItem> processes)
    {
        if (_profile is null) return;
        var selected = _profile.Processes.Where(process => process.IsSelected).Select(process => process.Name).ToList();
        var running = processes.Select(process => process.Name).ToHashSet(StringComparer.OrdinalIgnoreCase);
        var selectedSet = selected.ToHashSet(StringComparer.OrdinalIgnoreCase);
        foreach (var process in _profile.Processes) process.PropertyChanged -= ProcessPropertyChanged;
        _profile.Processes.Clear();

        foreach (var name in selected.Where(name => !running.Contains(name)))
            AddProcess(new ProcessItem(name, NotRunningMetric, NotRunningMetric, true));
        foreach (var process in processes)
        {
            process.IsSelected = selectedSet.Contains(process.Name);
            AddProcess(process);
        }
        NotifyProcessList();
    }

    private void AddProcess(ProcessItem process)
    {
        process.PropertyChanged += ProcessPropertyChanged;
        _profile!.Processes.Add(process);
    }

    private bool FilterProcess(ProcessItem process) =>
        string.IsNullOrWhiteSpace(ProcessFilter) || process.Name.Contains(ProcessFilter, StringComparison.OrdinalIgnoreCase);

    private void ProcessPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(ProcessItem.IsSelected)) return;
        OnPropertyChanged(nameof(SelectedCount));
        OnPropertyChanged(nameof(SelectionSummary));
    }

    private void NotifyProcessList()
    {
        OnPropertyChanged(nameof(FilteredProcesses));
        OnPropertyChanged(nameof(SelectedCount));
        OnPropertyChanged(nameof(SelectionSummary));
        OnPropertyChanged(nameof(IsProcessListEmpty));
        OnPropertyChanged(nameof(ProcessListEmptyText));
    }

    private void ClearSelection()
    {
        if (_profile is null) return;
        foreach (var process in _profile.Processes) process.IsSelected = false;
        FeedbackText = "No apps will be closed on activation. Save to apply the change.";
    }

    private void RestoreDefaults()
    {
        if (_profile is null) return;
        FanBoostEnabled = false;
        RecycleBinEnabled = false;
        BrowserCacheEnabled = false;
        foreach (var process in _profile.Processes) process.IsSelected = false;
        FeedbackText = "Restored safe defaults. Save to apply them.";
    }

    private async Task SaveAsync()
    {
        FeedbackText = await _saveAsync()
            ? "System tweak settings saved to Runner."
            : "Not saved. Runner is unavailable; your edits are kept in this window.";
    }

    private async Task RunCleanupAsync(string kind, string label)
    {
        FeedbackText = await _cleanupAsync(kind)
            ? $"Asked Runner to clean the {label}. The result appears in the status bar."
            : $"Could not start {label} cleanup because Runner is unavailable.";
    }

    private async Task RefreshProcessesAsync()
    {
        FeedbackText = await _refreshProcessesAsync()
            ? "Requested the running apps from Runner."
            : "Could not refresh because Runner is unavailable.";
    }
}
