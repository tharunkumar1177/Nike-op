using System.Text;
using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.Services;

/// <summary>
/// Resolves a profile's selected app names to the process instances running now.
/// Mirrors <c>resolve_targets</c> in the Rust core; EngineSvc re-validates every target.
/// </summary>
public static class TerminationPlanner
{
    public static IReadOnlyList<ProcessTarget> Plan(IEnumerable<string> selectedNames, IReadOnlyList<RunningProcess> processes, int sessionId, uint selfProcessId)
    {
        var wanted = selectedNames
            .Select(ProcessNames.Normalize)
            .Where(name => name.Length > 0 && !ProcessNames.IsProtected(name))
            .ToHashSet(StringComparer.Ordinal);
        var targets = new List<ProcessTarget>();
        var seen = new HashSet<uint>();

        foreach (var process in processes)
        {
            if (targets.Count == EngineProtocol.MaximumTargets) break;
            if (process.SessionId != sessionId
                || process.CreationTime == 0
                || process.ProcessId is 0 or 4
                || process.ProcessId == selfProcessId
                || ProcessNames.IsProtected(process.ImageName)
                || Encoding.UTF8.GetByteCount(process.ImageName) > EngineProtocol.MaximumImageNameBytes
                || !wanted.Contains(ProcessNames.Normalize(process.ImageName))
                || !seen.Add(process.ProcessId))
                continue;
            targets.Add(new ProcessTarget(process.ProcessId, process.CreationTime, process.ImageName));
        }
        return targets;
    }
}
