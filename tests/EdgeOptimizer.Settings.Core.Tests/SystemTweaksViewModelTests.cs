using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.Services;
using EdgeOptimizer.Settings.Core.ViewModels;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class SystemTweaksViewModelTests
{
    [Fact]
    public void ProcessSearchAndSelectionSummaryAreDeterministic()
    {
        // Verifies filtering is case-insensitive and selected-process totals reflect model changes.
        var profile = Fixtures.ConfiguredProfile();
        var viewModel = new SystemTweaksViewModel();
        viewModel.LoadProfile(profile);
        viewModel.ProcessFilter = "CHROME";
        Assert.Equal("chrome.exe", Assert.Single(viewModel.FilteredProcesses).Name);

        profile.Processes[0].IsSelected = false;
        Assert.Equal("2 selected", viewModel.SelectionSummary);
    }

    [Fact]
    public void EmptyListExplainsWhetherAFilterIsHidingApps()
    {
        // Verifies the empty state distinguishes "unavailable", "still loading", and "nothing matches the search".
        var viewModel = new SystemTweaksViewModel();
        viewModel.LoadProfile(new ProfileWorkspace("Empty", false));
        Assert.True(viewModel.IsProcessListEmpty);
        Assert.Equal("Running apps can't be listed on this system.", viewModel.ProcessListEmptyText);

        var withSource = new SystemTweaksViewModel(processSource: new FakeProcessSource());
        withSource.LoadProfile(new ProfileWorkspace("Empty", false));
        Assert.StartsWith("Loading", withSource.ProcessListEmptyText);

        viewModel.LoadProfile(Fixtures.ConfiguredProfile());
        viewModel.ProcessFilter = "notepad";
        Assert.True(viewModel.IsProcessListEmpty);
        Assert.Equal("No apps match \"notepad\".", viewModel.ProcessListEmptyText);
    }

    [Fact]
    public void NewProfileDefaultsAreSafe()
    {
        // Verifies a new profile selects nothing to close and enables no tweak or cleanup options.
        var viewModel = new SystemTweaksViewModel();
        viewModel.LoadProfile(new ProfileWorkspace("New", false));

        Assert.False(viewModel.FanBoostEnabled);
        Assert.False(viewModel.RecycleBinEnabled);
        Assert.False(viewModel.BrowserCacheEnabled);
        Assert.Equal(0, viewModel.SelectedCount);
    }

    [Fact]
    public void RestoreDefaultsClearsTogglesAndProcesses()
    {
        // Verifies restoring safe defaults disables all tweak options and process selections.
        var profile = Fixtures.ConfiguredProfile();
        profile.FanBoostEnabled = true;
        var viewModel = new SystemTweaksViewModel();
        viewModel.LoadProfile(profile);

        viewModel.RestoreDefaultsCommand.Execute(null);

        Assert.False(viewModel.FanBoostEnabled);
        Assert.False(viewModel.RecycleBinEnabled);
        Assert.False(viewModel.BrowserCacheEnabled);
        Assert.All(profile.Processes, process => Assert.False(process.IsSelected));
        Assert.Equal("0 selected", viewModel.SelectionSummary);
    }

    [Fact]
    public void SwitchingProfilesUsesIndependentTweakState()
    {
        // Verifies profile selection swaps tweak values without leaking changes between profiles.
        var first = new ProfileWorkspace("First", false) { FanBoostEnabled = true };
        var second = new ProfileWorkspace("Second", false);
        var viewModel = new SystemTweaksViewModel();
        viewModel.LoadProfile(first);
        Assert.True(viewModel.FanBoostEnabled);
        viewModel.LoadProfile(second);
        Assert.False(viewModel.FanBoostEnabled);
        Assert.True(first.FanBoostEnabled);
    }

    [Fact]
    public void ProcessSnapshotPreservesSelectionsAndUpdatesMetricsInPlace()
    {
        // Verifies a refresh keeps selections (case-insensitively), updates existing rows in place, and keeps selected apps that are not running.
        var profile = Fixtures.ConfiguredProfile();
        var discord = profile.Processes[0];
        var viewModel = new SystemTweaksViewModel();
        viewModel.LoadProfile(profile);

        viewModel.ApplyProcessSnapshot(new[]
        {
            new ProcessItem("discord.EXE", "2.0%", "200 MB", false),
            new ProcessItem("game.exe", "8.0%", "900 MB", false),
        });

        Assert.Same(discord, profile.Processes.Single(process => ProcessNames.Match(process.Name, "discord")));
        Assert.True(discord.IsSelected);
        Assert.Equal("2.0%", discord.Cpu);
        Assert.False(profile.Processes.Single(process => process.Name == "game.exe").IsSelected);
        var retained = profile.Processes.Where(process => process.Cpu == SystemTweaksViewModel.NotRunningMetric).Select(process => process.Name);
        Assert.Equal(new[] { "chrome.exe", "Spotify.exe" }, retained);
        Assert.DoesNotContain(profile.Processes, process => process.Name == "Steam.exe");
        Assert.Equal("3 selected", viewModel.SelectionSummary);
    }

    [Fact]
    public async Task RefreshListsOnlySameSessionUnprotectedAppsAndMarksStoppedSelections()
    {
        // Verifies the local fetcher hides protected and other-session processes and keeps a stopped selected app.
        var source = new FakeProcessSource(
            FakeProcessSource.Process(10, "Discord.exe"),
            FakeProcessSource.Process(11, "game.exe"),
            FakeProcessSource.Process(12, "svchost.exe"),
            FakeProcessSource.Process(13, "other.exe", session: 2));
        var profile = new ProfileWorkspace("Test", false);
        var viewModel = new SystemTweaksViewModel(processSource: source);
        viewModel.LoadProfile(profile);

        await viewModel.RefreshCommand.ExecuteAsync(null);
        Assert.Equal(new[] { "Discord.exe", "game.exe" }, profile.Processes.Select(process => process.Name));
        Assert.Equal("Updated the running apps.", viewModel.FeedbackText);

        var discord = profile.Processes[0];
        discord.IsSelected = true;
        source.Processes = new[] { FakeProcessSource.Process(11, "game.exe", cpuSeconds: 2) };
        await viewModel.RefreshCommand.ExecuteAsync(null);

        Assert.Same(discord, profile.Processes[0]);
        Assert.Equal(SystemTweaksViewModel.NotRunningMetric, discord.Cpu);
        Assert.Equal("1 selected", viewModel.SelectionSummary);
    }

    [Fact]
    public async Task FetcherRunsOnlyWhileThePageAndWindowAreActive()
    {
        // Verifies process enumeration idles when System Tweaks is not shown or the Settings window is inactive.
        var source = new FakeProcessSource(FakeProcessSource.Process(10, "game.exe"));
        var viewModel = new SystemTweaksViewModel(processSource: source, refreshInterval: TimeSpan.FromHours(1));
        viewModel.LoadProfile(new ProfileWorkspace("Test", false));
        Assert.False(viewModel.IsMonitoring);

        viewModel.SetPageActive(true);
        Assert.True(viewModel.IsMonitoring);
        await WaitUntilAsync(() => source.SnapshotCount >= 1);

        viewModel.SetWindowActive(false);
        Assert.False(viewModel.IsMonitoring);
        viewModel.SetWindowActive(true);
        Assert.True(viewModel.IsMonitoring);
        viewModel.SetPageActive(false);
        Assert.False(viewModel.IsMonitoring);
    }

    [Fact]
    public async Task MissingProcessSourceAndUnavailableRunnerAreReported()
    {
        // Verifies the page never claims a refresh or cleanup happened when its dependency is unavailable.
        var viewModel = new SystemTweaksViewModel(cleanupAsync: _ => Task.FromResult(false));
        viewModel.LoadProfile(new ProfileWorkspace("Test", false));

        viewModel.SetPageActive(true);
        Assert.False(viewModel.IsMonitoring);
        await viewModel.RefreshCommand.ExecuteAsync(null);
        Assert.Equal("Running apps can't be listed on this system.", viewModel.FeedbackText);

        await viewModel.RunRecycleBinCleanupCommand.ExecuteAsync(null);
        Assert.Equal("Could not start Recycle Bin cleanup because Runner is unavailable.", viewModel.FeedbackText);
    }

    private static async Task WaitUntilAsync(Func<bool> condition)
    {
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (!condition())
        {
            if (DateTime.UtcNow > deadline) throw new TimeoutException("The condition was not met in time.");
            await Task.Delay(10);
        }
    }
}
