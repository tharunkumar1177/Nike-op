using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.ViewModels;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class CrosshairViewModelTests
{
    [Fact]
    public void DirectionalMovementAndCenterUpdateCoordinates()
    {
        // Verifies movement changes one axis at a time and centering resets both axes.
        var viewModel = CreateViewModel(new FakeFilePicker(null));
        viewModel.MoveCommand.Execute("Right");
        viewModel.MoveCommand.Execute("Down");
        Assert.Equal((1, 1), (viewModel.XOffset, viewModel.YOffset));

        viewModel.CenterCommand.Execute(null);
        Assert.Equal((0, 0), (viewModel.XOffset, viewModel.YOffset));
    }

    [Fact]
    public void CoordinatesAreClampedToSupportedRange()
    {
        // Verifies manual offsets cannot exceed the preview's safe coordinate limits.
        var viewModel = CreateViewModel(new FakeFilePicker(null));
        viewModel.XOffset = 900;
        viewModel.YOffset = -900;
        Assert.Equal((500, -500), (viewModel.XOffset, viewModel.YOffset));
    }

    [Fact]
    public async Task FilePickerCancellationLeavesImageUnchangedAsync()
    {
        // Verifies cancelling image selection preserves the current crosshair asset.
        var viewModel = CreateViewModel(new FakeFilePicker(null));
        await viewModel.ReplaceImageCommand.ExecuteAsync(null);
        Assert.Equal(ProfileWorkspace.NoImageName, viewModel.ImageName);
        Assert.False(viewModel.HasImage);
        Assert.False(viewModel.RemoveImageCommand.CanExecute(null));
    }

    [Fact]
    public async Task ReplaceRemoveAndResetUpdatePreviewStateAsync()
    {
        // Verifies image replacement, removal, hiding, and reset mutate only the selected profile.
        var viewModel = CreateViewModel(new FakeFilePicker(@"C:\fixtures\precision.png"));
        await viewModel.ReplaceImageCommand.ExecuteAsync(null);
        Assert.Equal("precision.png", viewModel.ImageName);
        Assert.True(viewModel.HasImage);
        Assert.True(viewModel.RemoveImageCommand.CanExecute(null));

        viewModel.RemoveImageCommand.Execute(null);
        Assert.Equal(ProfileWorkspace.NoImageName, viewModel.ImageName);
        viewModel.HidePreviewCommand.Execute(null);
        Assert.False(viewModel.OverlayEnabled);

        viewModel.XOffset = 12;
        viewModel.ResetCommand.Execute(null);
        Assert.True(viewModel.OverlayEnabled);
        Assert.Equal(ProfileWorkspace.NoImageName, viewModel.ImageName);
        Assert.Null(viewModel.ImagePath);
        Assert.Equal(0, viewModel.XOffset);
    }

    [Fact]
    public async Task SaveFeedbackReflectsTheRunnerResult()
    {
        // Verifies the page only reports a successful save when Runner accepted it.
        var viewModel = new CrosshairViewModel(new FakeFilePicker(null), () => Task.FromResult(false));
        viewModel.LoadProfile(new ProfileWorkspace("Test", false));

        await ((CommunityToolkit.Mvvm.Input.IAsyncRelayCommand)viewModel.SaveCommand).ExecuteAsync(null);

        Assert.StartsWith("Not saved", viewModel.FeedbackText);
    }

    private static CrosshairViewModel CreateViewModel(FakeFilePicker picker)
    {
        var viewModel = new CrosshairViewModel(picker);
        viewModel.LoadProfile(new ProfileWorkspace("Test", false));
        return viewModel;
    }
}
