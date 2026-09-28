using CommunityToolkit.Mvvm.Input;
using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.ViewModels;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class MacrosViewModelTests
{
    [Fact]
    public void SearchIsCaseInsensitiveAndKeepsAMatchingSelection()
    {
        // Verifies macro filtering ignores casing and selects a visible macro when the current one is filtered out.
        var viewModel = CreateViewModel();
        viewModel.MacroSearch = "QUICK";

        Assert.Equal("Quick heal", Assert.Single(viewModel.FilteredMacros).Name);
        Assert.Equal("Quick heal", viewModel.SelectedMacro?.Name);

        viewModel.MacroSearch = "heal";
        Assert.Equal("Quick heal", viewModel.SelectedMacro?.Name);
    }

    [Fact]
    public void NewDuplicateAndDeleteMaintainSelection()
    {
        // Verifies macro collection edits always leave a valid selected item when one remains.
        var viewModel = CreateViewModel();
        viewModel.NewMacroCommand.Execute(null);
        Assert.Equal("New macro", viewModel.SelectedMacro?.Name);
        Assert.Single(viewModel.SelectedMacro!.Steps);

        viewModel.DuplicateMacroCommand.Execute(null);
        Assert.Equal("New macro copy", viewModel.SelectedMacro?.Name);
        Assert.Equal(4, viewModel.Macros.Count);

        viewModel.DeleteMacroCommand.Execute(null);
        Assert.Equal(3, viewModel.Macros.Count);
        Assert.NotNull(viewModel.SelectedMacro);
    }

    [Fact]
    public void RepeatedDuplicationProducesUniqueNames()
    {
        // Verifies duplicates never collide, because Runner rejects case-insensitive duplicate macro names.
        var viewModel = CreateViewModel();
        var source = viewModel.SelectedMacro!;

        viewModel.DuplicateMacroCommand.Execute(null);
        viewModel.SelectedMacro = source;
        viewModel.DuplicateMacroCommand.Execute(null);

        Assert.Equal(new[] { "Build combo", "Quick heal", "Build combo copy", "Build combo copy 2" }, viewModel.Macros.Select(macro => macro.Name));
        Assert.Equal(MacroValidation.Unassigned, viewModel.SelectedMacro?.Shortcut);
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

    [Fact]
    public void ValidationTracksEditsToTheSelectedMacro()
    {
        // Verifies the inline warning appears for an empty key and clears once the key is set.
        var viewModel = CreateViewModel();
        viewModel.AddActionCommand.Execute(null);
        Assert.Equal("Every key action needs a key.", viewModel.ValidationMessage);

        viewModel.SelectedMacro!.Steps[^1].Value = "E";
        Assert.False(viewModel.HasValidationMessage);

        viewModel.SelectedMacro.Shortcut = "F8";
        Assert.True(viewModel.HasValidationMessage);
    }

    [Fact]
    public async Task SaveIsBlockedWhileAnyMacroIsInvalid()
    {
        // Verifies invalid macros are caught before a save Runner would reject, and the offending macro is selected.
        var saves = 0;
        var viewModel = CreateViewModel(() => { saves++; return Task.FromResult(true); });
        var invalid = viewModel.Macros[1];
        invalid.Name = "build COMBO";

        await ((IAsyncRelayCommand)viewModel.SaveMacroCommand).ExecuteAsync(null);

        Assert.Equal(0, saves);
        Assert.StartsWith("Not saved.", viewModel.FeedbackText);
        Assert.NotNull(viewModel.SelectedMacro);
    }

    [Fact]
    public async Task SaveReportsRunnerUnavailability()
    {
        // Verifies the editor does not claim success when Runner could not persist the change.
        var viewModel = CreateViewModel(() => Task.FromResult(false));

        await ((IAsyncRelayCommand)viewModel.SaveMacroCommand).ExecuteAsync(null);

        Assert.StartsWith("Not saved. Runner is unavailable", viewModel.FeedbackText);
    }

    [Fact]
    public void StepActionIndexRoundTripsSupportedActions()
    {
        // Verifies the action picker index maps onto the codec's supported action names and ignores deselection.
        var step = new MacroStep("key UP", "W");
        Assert.Equal(2, step.ActionIndex);

        step.ActionIndex = 3;
        Assert.Equal("Wait", step.Action);

        step.ActionIndex = -1;
        Assert.Equal("Wait", step.Action);
        Assert.False(new MacroStep("Jump", "Space").IsSupportedAction);
    }

    private static MacrosViewModel CreateViewModel(Func<Task<bool>>? save = null)
    {
        var viewModel = new MacrosViewModel(save);
        viewModel.LoadProfile(Fixtures.ConfiguredProfile("Test"));
        return viewModel;
    }
}
