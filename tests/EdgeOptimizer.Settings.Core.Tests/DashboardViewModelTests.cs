using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.ViewModels;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class DashboardViewModelTests
{
    [Fact]
    public void ConfiguredProfileIsReady()
    {
        // Verifies readiness is derived from apps, a loaded crosshair, and an assigned macro.
        var viewModel = new DashboardViewModel(_ => { });
        viewModel.LoadProfile(new ProfileWorkspace("Ready", false));

        Assert.True(viewModel.IsReady);
        Assert.Equal("Ready", viewModel.ReadinessLabel);
        Assert.Equal(3, viewModel.SelectedAppCount);
    }

    [Fact]
    public void MissingCrosshairOrMacroRequiresSetup()
    {
        // Verifies incomplete profile configuration cannot be reported as ready.
        var profile = new ProfileWorkspace("Incomplete", false) { CrosshairImageName = "No image selected" };
        profile.Macros.Clear();
        var viewModel = new DashboardViewModel(_ => { });

        viewModel.LoadProfile(profile);

        Assert.False(viewModel.IsReady);
        Assert.Equal("Setup required", viewModel.ReadinessLabel);
        Assert.Equal("Unassigned", viewModel.MacroShortcut);
    }
}
