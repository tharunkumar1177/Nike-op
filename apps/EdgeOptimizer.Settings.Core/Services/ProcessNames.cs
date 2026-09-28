namespace EdgeOptimizer.Settings.Core.Services;

/// <summary>
/// Process-name normalization and protected names. Must match
/// <c>normalize_process_name</c> and <c>PROTECTED_PROCESSES</c> in <c>crates/core/src/process.rs</c>;
/// EngineSvc re-applies the Rust rules, so this copy is advisory.
/// </summary>
public static class ProcessNames
{
    private static readonly HashSet<string> Protected = new(StringComparer.Ordinal)
    {
        "audiodg", "csrss", "ctfmon", "dwm", "edgeoptimizer.settings.winui", "edgeoptimizer_crosshair",
        "edgeoptimizer_enginesvc", "edgeoptimizer_macro", "edgeoptimizer_runner", "explorer", "fontdrvhost",
        "lsaiso", "lsass", "memory compression", "msmpeng", "mssense", "nissrv", "registry",
        "securityhealthservice", "services", "sihost", "smss", "svchost", "system", "wininit", "winlogon",
    };

    public static string Normalize(string name)
    {
        var lower = ToAsciiLower(name.Trim());
        if (lower.EndsWith(".exe", StringComparison.Ordinal)) lower = lower[..^4];
        return lower.Trim();
    }

    public static bool IsProtected(string name) => Protected.Contains(Normalize(name));

    public static bool Match(string left, string right)
    {
        var normalized = Normalize(left);
        return normalized.Length > 0 && normalized == Normalize(right);
    }

    private static string ToAsciiLower(string value) =>
        string.Create(value.Length, value, static (span, source) =>
        {
            for (var index = 0; index < source.Length; index++)
            {
                var c = source[index];
                span[index] = c is >= 'A' and <= 'Z' ? (char)(c + 32) : c;
            }
        });
}
