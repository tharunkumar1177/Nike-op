using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.ViewModels;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class DashboardViewModelTests
{
    [Fact]
    public void ConfiguredProfileIsReady()
    {
        // Verifies readiness is derived from selected apps, a crosshair image, and an assigned macro shortcut.
        var viewModel = new DashboardViewModel(_ => { });
        viewModel.LoadProfile(Fixtures.ConfiguredProfile());

        Assert.True(viewModel.IsReady);
        Assert.Equal("Ready", viewModel.ReadinessLabel);
        Assert.Equal(3, viewModel.SelectedAppCount);
        Assert.Equal(100d, viewModel.SetupProgress);
        Assert.Equal("3 of 3 set up", viewModel.SetupProgressLabel);
        Assert.Equal("Ctrl + F8", viewModel.MacroShortcut);
    }

    [Fact]
    public void NewProfileReportsEveryStepAsIncomplete()
    {
        // Verifies the setup checklist is computed from profile state rather than a fixed placeholder value.
        var viewModel = new DashboardViewModel(_ => { });

        viewModel.LoadProfile(new ProfileWorkspace("Empty", false));

        Assert.False(viewModel.IsReady);
        Assert.Equal("Setup required", viewModel.ReadinessLabel);
        Assert.Equal(0, viewModel.CompletedSetupSteps);
        Assert.Equal(0d, viewModel.SetupProgress);
        Assert.Equal("No image", viewModel.CrosshairStatus);
        Assert.Equal(MacroValidation.Unassigned, viewModel.MacroShortcut);
    }

    [Fact]
    public void CrosshairRequiresAnImageAndAnEnabledOverlay()
    {
        // Verifies the crosshair step matches Runner, which starts the overlay only with an image and the overlay on.
        var profile = Fixtures.ConfiguredProfile();
        var viewModel = new DashboardViewModel(_ => { });

        profile.OverlayEnabled = false;
        viewModel.LoadProfile(profile);

        Assert.False(viewModel.HasCrosshair);
        Assert.Equal("Off", viewModel.CrosshairStatus);
        Assert.Equal("2 of 3 set up", viewModel.SetupProgressLabel);
    }

    [Fact]
    public void MacroShortcutIgnoresDisabledAndUnassignedMacros()
    {
        // Verifies the trigger summary shows the first macro Runner would actually register.
        var profile = new ProfileWorkspace("Macros", false);
        profile.Macros.Add(new MacroDefinition("Unassigned", MacroValidation.Unassigned, new[] { new MacroStep("Key press", "A") }));
        profile.Macros.Add(new MacroDefinition("Disabled", "Ctrl + F1", new[] { new MacroStep("Key press", "B") }) { IsEnabled = false });
        profile.Macros.Add(new MacroDefinition("Live", "Alt + F2", new[] { new MacroStep("Key press", "C") }));
        var viewModel = new DashboardViewModel(_ => { });

        viewModel.LoadProfile(profile);

        Assert.Equal("Alt + F2", viewModel.MacroShortcut);
        Assert.True(viewModel.HasMacroShortcut);
        Assert.Equal(3, viewModel.MacroCount);
    }
}
