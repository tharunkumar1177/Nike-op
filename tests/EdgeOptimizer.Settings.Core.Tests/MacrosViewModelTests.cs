using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.ViewModels;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class MacrosViewModelTests
{
    [Fact]
    public void SearchIsCaseInsensitive()
    {
        // Verifies macro filtering ignores character casing.
        var viewModel = CreateViewModel();
        viewModel.MacroSearch = "QUICK";
        Assert.Equal("Quick heal", Assert.Single(viewModel.FilteredMacros).Name);
    }

    [Fact]
    public void NewDuplicateAndDeleteMaintainSelection()
    {
        // Verifies macro collection edits always leave a valid selected item when one remains.
        var viewModel = CreateViewModel();
        viewModel.NewMacroCommand.Execute(null);
        Assert.Equal("New macro 3", viewModel.SelectedMacro?.Name);

        viewModel.DuplicateMacroCommand.Execute(null);
        Assert.Equal("New macro 3 copy", viewModel.SelectedMacro?.Name);
        Assert.Equal(4, viewModel.Macros.Count);

        viewModel.DeleteMacroCommand.Execute(null);
        Assert.Equal(3, viewModel.Macros.Count);
        Assert.NotNull(viewModel.SelectedMacro);
    }

    [Fact]
    public void AddAndDeleteStepUpdateSelectedSequence()
    {
        // Verifies sequence actions are added to and removed from only the selected macro.
        var viewModel = CreateViewModel();
        var initialCount = viewModel.SelectedMacro!.Steps.Count;
        viewModel.AddActionCommand.Execute(null);
        var added = viewModel.SelectedMacro.Steps[^1];
        Assert.Equal(initialCount + 1, viewModel.SelectedMacro.Steps.Count);

        viewModel.DeleteStepCommand.Execute(added);
        Assert.Equal(initialCount, viewModel.SelectedMacro.Steps.Count);
    }

    [Fact]
    public void DuplicateCopiesStepsWithoutSharingCollection()
    {
        // Verifies duplicated macros preserve actions without sharing a mutable steps collection.
        var viewModel = CreateViewModel();
        var source = viewModel.SelectedMacro!;
        viewModel.DuplicateMacroCommand.Execute(null);
        var duplicate = viewModel.SelectedMacro!;
        duplicate.Steps.Add(new MacroStep("Wait", "5 ms"));
        Assert.NotEqual(source.Steps.Count, duplicate.Steps.Count);
    }

    private static MacrosViewModel CreateViewModel()
    {
        var viewModel = new MacrosViewModel();
        viewModel.LoadProfile(new ProfileWorkspace("Test", false));
        return viewModel;
    }
}
