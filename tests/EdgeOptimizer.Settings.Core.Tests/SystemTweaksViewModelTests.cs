using EdgeOptimizer.Settings.Core.Models;
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
        // Verifies the empty state distinguishes "nothing loaded" from "nothing matches the search".
        var viewModel = new SystemTweaksViewModel();
        viewModel.LoadProfile(new ProfileWorkspace("Empty", false));
        Assert.True(viewModel.IsProcessListEmpty);
        Assert.StartsWith("No apps listed yet", viewModel.ProcessListEmptyText);

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
    public void ProcessSnapshotPreservesSelectionsAndUpdatesMetrics()
    {
        // Verifies a Runner refresh keeps selections (case-insensitively), updates metrics, and keeps selected apps that are not running.
        var profile = Fixtures.ConfiguredProfile();
        var viewModel = new SystemTweaksViewModel();
        viewModel.LoadProfile(profile);

        viewModel.ApplyProcessSnapshot(new[]
        {
            new ProcessItem("discord.EXE", "2.0%", "200 MB", false),
            new ProcessItem("game.exe", "8.0%", "900 MB", false),
        });

        var discord = profile.Processes.Single(process => process.Name == "discord.EXE");
        Assert.True(discord.IsSelected);
        Assert.Equal("2.0%", discord.Cpu);
        Assert.False(profile.Processes.Single(process => process.Name == "game.exe").IsSelected);
        var retained = profile.Processes.Where(process => process.Cpu == SystemTweaksViewModel.NotRunningMetric).Select(process => process.Name);
        Assert.Equal(new[] { "chrome.exe", "Spotify.exe" }, retained);
        Assert.DoesNotContain(profile.Processes, process => process.Name == "Steam.exe");
        Assert.Equal("3 selected", viewModel.SelectionSummary);
    }

    [Fact]
    public async Task UnavailableRunnerIsReportedForRefreshAndCleanup()
    {
        // Verifies the page never claims a request was sent when Runner could not accept it.
        var viewModel = new SystemTweaksViewModel(cleanupAsync: _ => Task.FromResult(false), refreshProcessesAsync: () => Task.FromResult(false));
        viewModel.LoadProfile(new ProfileWorkspace("Test", false));

        await viewModel.RefreshCommand.ExecuteAsync(null);
        Assert.Equal("Could not refresh because Runner is unavailable.", viewModel.FeedbackText);

        await viewModel.RunRecycleBinCleanupCommand.ExecuteAsync(null);
        Assert.Equal("Could not start Recycle Bin cleanup because Runner is unavailable.", viewModel.FeedbackText);
    }
}
