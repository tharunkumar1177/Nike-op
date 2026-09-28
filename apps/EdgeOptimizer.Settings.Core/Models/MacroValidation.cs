using System.Text;

namespace EdgeOptimizer.Settings.Core.Models;

/// <summary>
/// Mirrors <c>MacroDefinition::validate</c> in <c>crates/core/src/macro_config.rs</c> and the
/// key set the Macro worker can register, so problems surface before Runner rejects a save.
/// </summary>
public static class MacroValidation
{
    public const int MaximumNameBytes = 50;
    public const string Unassigned = "Unassigned";

    private static readonly string[] Modifiers = { "Ctrl", "Alt", "Shift", "Win" };

    public static bool IsShortcutAssigned(string? shortcut) =>
        !string.IsNullOrWhiteSpace(shortcut)
        && !shortcut.Equals(Unassigned, StringComparison.OrdinalIgnoreCase)
        && !shortcut.Equals("Not set", StringComparison.OrdinalIgnoreCase);

    /// <returns>A user-facing problem description, or <c>null</c> when the macro is valid.</returns>
    public static string? Validate(MacroDefinition macro, IEnumerable<MacroDefinition> siblings)
    {
        if (string.IsNullOrWhiteSpace(macro.Name)) return "Give this macro a name.";
        if (Encoding.UTF8.GetByteCount(macro.Name) > MaximumNameBytes) return $"Macro names must be {MaximumNameBytes} characters or fewer.";
        if (siblings.Any(other => !ReferenceEquals(other, macro) && string.Equals(other.Name.Trim(), macro.Name.Trim(), StringComparison.OrdinalIgnoreCase)))
            return "Another macro in this profile already uses this name.";
        if (macro.Steps.Count == 0) return "Add at least one action.";
        if (macro.Steps.Any(step => !step.IsSupportedAction)) return "Choose an action type for every step.";
        if (macro.Steps.Any(step => IsKeyAction(step.Action) && string.IsNullOrWhiteSpace(step.Value)))
            return "Every key action needs a key.";
        return ValidateShortcut(macro.Shortcut);
    }

    private static bool IsKeyAction(string action) =>
        action.StartsWith("Key ", StringComparison.OrdinalIgnoreCase);

    public static string? ValidateShortcut(string? shortcut)
    {
        if (!IsShortcutAssigned(shortcut)) return null;
        var parts = shortcut!.Split('+', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries);
        var key = parts.LastOrDefault() ?? string.Empty;
        var hasModifier = parts.SkipLast(1).Any(part => Modifiers.Contains(part, StringComparer.OrdinalIgnoreCase));
        if (!hasModifier || Modifiers.Contains(key, StringComparer.OrdinalIgnoreCase))
            return "Shortcuts need a modifier and a key, for example Ctrl + F8.";
        if (!IsRegistrableKey(key)) return "Shortcut keys must be a letter, a digit, or F1–F12.";
        return null;
    }

    private static bool IsRegistrableKey(string key)
    {
        if (key.Length == 1) return char.IsAsciiLetterOrDigit(key[0]);
        return key.Length is 2 or 3
            && (key[0] == 'F' || key[0] == 'f')
            && int.TryParse(key.AsSpan(1), out var number)
            && number is >= 1 and <= 12;
    }
}
