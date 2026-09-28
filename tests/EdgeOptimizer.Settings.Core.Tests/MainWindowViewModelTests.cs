using EdgeOptimizer.Settings.Core.ViewModels;
using EdgeOptimizer.Settings.Core.Models;
using CommunityToolkit.Mvvm.Input;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class MainWindowViewModelTests
{
    [Fact]
    public void StartsWithoutPlaceholderProfiles()
    {
        // Verifies the shell never shows invented profiles before Runner supplies its authoritative state.
        var viewModel = CreateViewModel();

        Assert.Empty(viewModel.Profiles);
        Assert.Null(viewModel.SelectedProfile);
        Assert.True(viewModel.ShowEmptyState);
        Assert.Equal("Runner is not available", viewModel.EmptyStateTitle);
    }

    [Fact]
    public void NavigationPreservesSelectedProfile()
    {
        // Verifies switching feature pages never loses the user's selected profile context.
        var (viewModel, _) = CreateHydrated(new ProfileWorkspace("First", false), new ProfileWorkspace("Second", false));
        var selected = viewModel.Profiles[1];
        viewModel.SelectedProfile = selected;

        foreach (var page in new[] { "Dashboard", "Crosshair", "Macros", "SystemTweaks" })
        {
            viewModel.NavigateTo(page);
            Assert.Same(selected, viewModel.SelectedProfile);
        }
        Assert.True(viewModel.IsSystemTweaksSelected);
    }

    [Fact]
    public void SelectingProfileLoadsEveryWorkspace()
    {
        // Verifies Crosshair, Macros, System Tweaks, and Dashboard receive the newly selected profile state.
        var (viewModel, _) = CreateHydrated(new ProfileWorkspace("First", false), Fixtures.ConfiguredProfile("Second"));
        var selected = viewModel.Profiles[1];
        selected.CrosshairXOffset = 17;
        selected.FanBoostEnabled = true;

        viewModel.SelectedProfile = selected;

        Assert.Same(selected, viewModel.Dashboard.SelectedProfile);
        Assert.Equal(17, viewModel.Crosshair.XOffset);
        Assert.Same(selected.Macros[0], viewModel.Macros.SelectedMacro);
        Assert.True(viewModel.SystemTweaks.FanBoostEnabled);
    }

    [Fact]
    public void DashboardQuickActionNavigatesToRequestedWorkspace()
    {
        // Verifies a Dashboard quick action routes through the shell navigation contract.
        var viewModel = CreateViewModel();

        viewModel.Dashboard.NavigateCommand.Execute("Macros");

        Assert.Equal("Macros", viewModel.CurrentPageLabel);
        Assert.Same(viewModel.Macros, viewModel.CurrentPage);
    }

    [Fact]
    public void NewProfileIsSelectedAndSelectsNothingForTermination()
    {
        // Verifies a new profile matches Runner defaults: no apps to close, no macros, and no image.
        var (viewModel, _) = CreateHydrated(Fixtures.ConfiguredProfile());

        viewModel.NewProfileCommand.Execute(null);

        var created = Assert.IsType<ProfileWorkspace>(viewModel.SelectedProfile);
        Assert.Same(viewModel.Profiles[^1], created);
        Assert.Equal("New profile", created.Name);
        Assert.Empty(created.Processes);
        Assert.Empty(created.Macros);
        Assert.False(created.HasCrosshairImage);
        Assert.False(created.FanBoostEnabled);
        Assert.Same(created, viewModel.Dashboard.SelectedProfile);
    }

    [Fact]
    public void NewProfileNamesAreUniqueIgnoringCase()
    {
        // Verifies repeated creation never produces a name Runner would reject as a duplicate.
        var (viewModel, _) = CreateHydrated(new ProfileWorkspace("new PROFILE", false));

        viewModel.NewProfileCommand.Execute(null);

        Assert.Equal("New profile 2", viewModel.SelectedProfile?.Name);
    }

    [Fact]
    public void ActivationReflectsRunnerAvailability()
    {
        // Verifies profile activation stays disabled until a Runner client reports a connection.
        Assert.False(CreateViewModel().ActivationEnabled);
        Assert.True(new MainWindowViewModel(new FakeFilePicker(null), new FakeRunnerClient(true)).ActivationEnabled);
    }

    [Fact]
    public void RunnerSnapshotSelectsTheActiveProfile()
    {
        // Verifies Runner hydration marks the authoritative active profile and selects it.
        var (viewModel, _) = CreateHydratedWithActive("Active", new ProfileWorkspace("Hydrated", false), new ProfileWorkspace("Active", false));

        Assert.Equal(2, viewModel.Profiles.Count);
        Assert.Equal("Active", viewModel.SelectedProfile?.Name);
        Assert.True(viewModel.SelectedProfile?.IsActive);
        Assert.True(viewModel.IsSelectedProfileActive);
    }

    [Fact]
    public void RunnerSnapshotKeepsTheCurrentSelection()
    {
        // Verifies a later Runner snapshot does not jump away from the profile the user is editing.
        var (viewModel, runner) = CreateHydratedWithActive("Active", new ProfileWorkspace("Other", false), new ProfileWorkspace("Active", false));
        viewModel.SelectedProfile = viewModel.Profiles[0];

        runner.RaiseSnapshot(Fixtures.Snapshot("Active", new ProfileWorkspace("Other", false), new ProfileWorkspace("Active", false)));

        Assert.Equal("Other", viewModel.SelectedProfile?.Name);
    }

    [Fact]
    public void EmptyRunnerSnapshotDoesNotInventAProfile()
    {
        // Verifies an empty Runner store shows the create-profile empty state instead of a fabricated profile.
        var (viewModel, _) = CreateHydrated();

        Assert.Empty(viewModel.Profiles);
        Assert.True(viewModel.ShowEmptyState);
        Assert.Equal("No profiles yet", viewModel.EmptyStateTitle);
    }

    [Fact]
    public async Task ActivationSavesThenRequestsRunnerOrchestration()
    {
        // Verifies activation persists the current collection before asking Runner to start workers and optimization.
        var (viewModel, runner) = CreateHydrated(Fixtures.ConfiguredProfile());

        await ((IAsyncRelayCommand)viewModel.ActivateProfileCommand).ExecuteAsync(null);

        Assert.Equal(viewModel.Profiles.Count, runner.SavedProfiles.Count);
        Assert.Same(viewModel.SelectedProfile, Assert.Single(runner.ActivatedProfiles));
    }

    [Fact]
    public async Task RunnerSendFailureIsReportedInsteadOfThrown()
    {
        // Verifies a broken pipe during save or activation surfaces a status message rather than crashing the client.
        var (viewModel, runner) = CreateHydrated(Fixtures.ConfiguredProfile());
        runner.SendFailure = new IOException("Pipe is broken.");

        await ((IAsyncRelayCommand)viewModel.ActivateProfileCommand).ExecuteAsync(null);
        await ((IAsyncRelayCommand)viewModel.Crosshair.SaveCommand).ExecuteAsync(null);

        Assert.Empty(runner.ActivatedProfiles);
        Assert.Contains("Pipe is broken.", viewModel.StatusMessage);
        Assert.StartsWith("Not saved", viewModel.Crosshair.FeedbackText);
    }

    [Fact]
    public async Task DeleteRemovesTheProfileAndSavesThroughRunner()
    {
        // Verifies deletion keeps a valid selection and persists the reduced collection.
        var (viewModel, runner) = CreateHydrated(new ProfileWorkspace("First", false), new ProfileWorkspace("Second", false));
        viewModel.SelectedProfile = viewModel.Profiles[0];

        await viewModel.DeleteProfileCommand.ExecuteAsync(null);

        Assert.Equal("Second", Assert.Single(viewModel.Profiles).Name);
        Assert.Equal("Second", viewModel.SelectedProfile?.Name);
        Assert.Equal("Second", Assert.Single(runner.SavedProfiles).Name);
    }

    [Fact]
    public async Task ReconnectStartsTheRunnerClientOnlyWhileOffline()
    {
        // Verifies Retry attempts a new connection when Runner is offline and is disabled once connected.
        var runner = new FakeRunnerClient();
        var viewModel = new MainWindowViewModel(new FakeFilePicker(null), runner);

        await ((IAsyncRelayCommand)viewModel.ReconnectCommand).ExecuteAsync(null);
        Assert.Equal(1, runner.StartCount);

        runner.RaiseConnection(true);
        Assert.False(viewModel.ReconnectCommand.CanExecute(null));
        Assert.True(viewModel.IsRunnerConnected);
        Assert.False(viewModel.IsRunnerOffline);
    }

    [Fact]
    public async Task CleanupAndRefreshAreForwardedToRunner()
    {
        // Verifies System Tweaks cleanup and process refresh reach Runner with the transitional cleanup kinds.
        var (viewModel, runner) = CreateHydrated(new ProfileWorkspace("Test", false));

        await viewModel.SystemTweaks.RunRecycleBinCleanupCommand.ExecuteAsync(null);
        await viewModel.SystemTweaks.RunBrowserCacheCleanupCommand.ExecuteAsync(null);
        await viewModel.SystemTweaks.RefreshCommand.ExecuteAsync(null);

        Assert.Equal(new[] { SystemTweaksViewModel.RecycleBinCleanup, SystemTweaksViewModel.BrowserCacheCleanup }, runner.CleanupRequests);
        Assert.Equal(1, runner.ProcessSnapshotRequests);
    }

    private static MainWindowViewModel CreateViewModel() =>
        new(new FakeFilePicker(null), new FakeRunnerClient());

    private static (MainWindowViewModel ViewModel, FakeRunnerClient Runner) CreateHydrated(params ProfileWorkspace[] profiles) =>
        CreateHydratedWithActive(null, profiles);

    private static (MainWindowViewModel ViewModel, FakeRunnerClient Runner) CreateHydratedWithActive(string? activeName, params ProfileWorkspace[] profiles)
    {
        var runner = new FakeRunnerClient(true);
        var viewModel = new MainWindowViewModel(new FakeFilePicker(null), runner);
        runner.RaiseSnapshot(Fixtures.Snapshot(activeName, profiles));
        return (viewModel, runner);
    }
}
