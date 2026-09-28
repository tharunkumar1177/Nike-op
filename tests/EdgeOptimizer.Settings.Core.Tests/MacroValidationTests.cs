using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class MacroValidationTests
{
    [Theory]
    [InlineData("Unassigned")]
    [InlineData("  ")]
    [InlineData("Ctrl + F8")]
    [InlineData("ctrl+shift+a")]
    [InlineData(" Alt + 7 ")]
    [InlineData("Win + f12")]
    public void AcceptsShortcutsTheWorkerCanRegister(string shortcut)
    {
        // Verifies modifier-plus-key shortcuts pass regardless of casing and whitespace, and unassigned is allowed.
        Assert.Null(MacroValidation.ValidateShortcut(shortcut));
    }

    [Theory]
    [InlineData("F8")]
    [InlineData("Ctrl + Shift")]
    [InlineData("Ctrl +")]
    [InlineData("Hyper + A")]
    public void RejectsShortcutsWithoutAModifierAndKey(string shortcut)
    {
        // Verifies the Rust rule that a shortcut needs at least one modifier and a non-modifier key.
        Assert.Equal("Shortcuts need a modifier and a key, for example Ctrl + F8.", MacroValidation.ValidateShortcut(shortcut));
    }

    [Theory]
    [InlineData("Ctrl + Space")]
    [InlineData("Alt + F13")]
    [InlineData("Shift + F0")]
    [InlineData("Ctrl + é")]
    public void RejectsKeysTheWorkerCannotRegister(string shortcut)
    {
        // Verifies keys outside the worker's letter, digit, and F1–F12 table are flagged instead of silently ignored.
        Assert.Equal("Shortcut keys must be a letter, a digit, or F1–F12.", MacroValidation.ValidateShortcut(shortcut));
    }

    [Fact]
    public void RejectsNamesRunnerWouldRefuse()
    {
        // Verifies empty, over-long (by UTF-8 bytes), and case-insensitive duplicate names are reported.
        var steps = new[] { new MacroStep("Key press", "A") };
        var existing = new MacroDefinition("Heal", MacroValidation.Unassigned, steps);

        Assert.Equal("Give this macro a name.", MacroValidation.Validate(new MacroDefinition(" ", MacroValidation.Unassigned, steps), Array.Empty<MacroDefinition>()));
        Assert.NotNull(MacroValidation.Validate(new MacroDefinition(new string('é', 26), MacroValidation.Unassigned, steps), Array.Empty<MacroDefinition>()));
        Assert.Null(MacroValidation.Validate(new MacroDefinition(new string('a', 50), MacroValidation.Unassigned, steps), Array.Empty<MacroDefinition>()));

        var duplicate = new MacroDefinition(" heal ", MacroValidation.Unassigned, steps);
        Assert.Equal("Another macro in this profile already uses this name.", MacroValidation.Validate(duplicate, new[] { existing, duplicate }));
    }

    [Fact]
    public void RejectsEmptyOrMalformedActionLists()
    {
        // Verifies a macro needs at least one action, recognized action types, and keys for key actions.
        Assert.Equal("Add at least one action.", MacroValidation.Validate(new MacroDefinition("A", MacroValidation.Unassigned, Array.Empty<MacroStep>()), Array.Empty<MacroDefinition>()));
        Assert.Equal("Choose an action type for every step.", MacroValidation.Validate(new MacroDefinition("B", MacroValidation.Unassigned, new[] { new MacroStep("Jump", "Space") }), Array.Empty<MacroDefinition>()));
        Assert.Equal("Every key action needs a key.", MacroValidation.Validate(new MacroDefinition("C", MacroValidation.Unassigned, new[] { new MacroStep("Key down", " ") }), Array.Empty<MacroDefinition>()));
        Assert.Null(MacroValidation.Validate(new MacroDefinition("D", MacroValidation.Unassigned, new[] { new MacroStep("Wait", "") }), Array.Empty<MacroDefinition>()));
    }
}
