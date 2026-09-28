using System.ComponentModel;
using System.Windows.Input;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.Services;

namespace EdgeOptimizer.Settings.Core.ViewModels;

public sealed class SystemTweaksViewModel : ObservableObject
{
    public const string RecycleBinCleanup = "recycle-bin";
    public const string BrowserCacheCleanup = "browser-cache";
    public const string NotRunningMetric = "—";
    public static readonly TimeSpan DefaultRefreshInterval = TimeSpan.FromSeconds(3);

    private readonly Func<Task<bool>> _saveAsync;
    private readonly Func<string, Task<bool>> _cleanupAsync;
    private readonly IProcessSource? _processSource;
    private readonly TimeSpan _refreshInterval;
    private readonly ProcessSampler _sampler = new(Environment.ProcessorCount);
    private ProfileWorkspace? _profile;
    private string _processFilter = string.Empty;
    private string _feedbackText = "Choose the apps to close when this profile is activated.";
    private bool _isPageActive;
    private bool _isWindowActive = true;
    private bool _isRefreshing;
    private CancellationTokenSource? _monitoring;

    public SystemTweaksViewModel(
        Func<Task<bool>>? saveAsync = null,
        Func<string, Task<bool>>? cleanupAsync = null,
        IProcessSource? processSource = null,
        TimeSpan? refreshInterval = null)
    {
        _saveAsync = saveAsync ?? (() => Task.FromResult(true));
        _cleanupAsync = cleanupAsync ?? (_ => Task.FromResult(true));
        _processSource = processSource;
        _refreshInterval = refreshInterval ?? DefaultRefreshInterval;
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
        ? _processSource is null ? "Running apps can't be listed on this system." : "Loading the apps that are running now…"
        : $"No apps match \"{ProcessFilter}\".";

    /// <summary>True while the running-app list refreshes periodically.</summary>
    public bool IsMonitoring => _monitoring is not null;

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
        if (IsMonitoring) _ = RefreshOnceAsync(CancellationToken.None);
    }

    /// <summary>Call when the System Tweaks page is shown or navigated away from.</summary>
    public void SetPageActive(bool active)
    {
        _isPageActive = active;
        UpdateMonitoring();
    }

    /// <summary>Call when the Settings window is activated, deactivated, or minimized.</summary>
    public void SetWindowActive(bool active)
    {
        _isWindowActive = active;
        UpdateMonitoring();
    }

    public void ReportTermination(string summary) => FeedbackText = summary;

    /// <summary>
    /// Merges a snapshot into the listed processes. Existing rows update in place;
    /// selected apps that are not running stay selected, so a refresh never silently
    /// shrinks the profile's close list.
    /// </summary>
    public void ApplyProcessSnapshot(IReadOnlyList<ProcessItem> processes)
    {
        if (_profile is null) return;
        var incoming = new Dictionary<string, ProcessItem>(StringComparer.Ordinal);
        foreach (var process in processes) incoming.TryAdd(ProcessNames.Normalize(process.Name), process);

        var membershipChanged = false;
        foreach (var existing in _profile.Processes.ToList())
        {
            var key = ProcessNames.Normalize(existing.Name);
            if (incoming.Remove(key, out var fresh))
            {
                existing.UpdateMetrics(fresh.Cpu, fresh.Memory);
            }
            else if (existing.IsSelected)
            {
                existing.UpdateMetrics(NotRunningMetric, NotRunningMetric);
            }
            else
            {
                existing.PropertyChanged -= ProcessPropertyChanged;
                _profile.Processes.Remove(existing);
                membershipChanged = true;
            }
        }
        foreach (var process in processes)
        {
            if (!incoming.Remove(ProcessNames.Normalize(process.Name))) continue;
            process.IsSelected = false;
            process.PropertyChanged += ProcessPropertyChanged;
            _profile.Processes.Add(process);
            membershipChanged = true;
        }
        if (membershipChanged) NotifyProcessList();
    }

    private void UpdateMonitoring()
    {
        var shouldRun = _isPageActive && _isWindowActive && _processSource is not null;
        if (shouldRun == IsMonitoring) return;
        if (shouldRun)
        {
            _monitoring = new CancellationTokenSource();
            _ = MonitorAsync(_monitoring.Token);
        }
        else
        {
            _monitoring!.Cancel();
            _monitoring.Dispose();
            _monitoring = null;
        }
        OnPropertyChanged(nameof(IsMonitoring));
    }

    private async Task MonitorAsync(CancellationToken cancellationToken)
    {
        try
        {
            await RefreshOnceAsync(cancellationToken);
            using var timer = new PeriodicTimer(_refreshInterval);
            while (await timer.WaitForNextTickAsync(cancellationToken))
                await RefreshOnceAsync(cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested) { }
    }

    private async Task<bool> RefreshOnceAsync(CancellationToken cancellationToken)
    {
        if (_processSource is null || _profile is null || _isRefreshing) return false;
        _isRefreshing = true;
        try
        {
            var source = _processSource;
            var snapshot = await Task.Run(() => source.Snapshot(), cancellationToken);
            if (cancellationToken.IsCancellationRequested) return false;
            ApplyProcessSnapshot(_sampler.Sample(snapshot, source.CurrentSessionId, DateTimeOffset.UtcNow));
            return true;
        }
        catch (Exception error) when (error is not OperationCanceledException)
        {
            FeedbackText = $"Could not read the running apps: {error.Message}";
            return false;
        }
        finally
        {
            _isRefreshing = false;
        }
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
        if (_processSource is null)
        {
            FeedbackText = "Running apps can't be listed on this system.";
            return;
        }
        if (await RefreshOnceAsync(CancellationToken.None)) FeedbackText = "Updated the running apps.";
    }
}
