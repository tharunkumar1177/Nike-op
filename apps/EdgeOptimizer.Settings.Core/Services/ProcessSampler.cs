using System.Globalization;
using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.Services;

/// <summary>
/// Turns successive process snapshots into one row per executable, with CPU usage
/// measured between samples. Instances are tracked by PID and creation time, so a
/// reused PID never inherits another process's CPU history.
/// </summary>
public sealed class ProcessSampler
{
    public const string UnmeasuredMetric = "—";

    private readonly int _processorCount;
    private Dictionary<(uint ProcessId, ulong CreationTime), TimeSpan> _previousCpu = new();
    private DateTimeOffset? _previousAt;

    public ProcessSampler(int processorCount) => _processorCount = Math.Max(1, processorCount);

    public IReadOnlyList<ProcessItem> Sample(IReadOnlyList<RunningProcess> processes, int sessionId, DateTimeOffset now)
    {
        var elapsed = _previousAt is { } previousAt ? now - previousAt : TimeSpan.Zero;
        var currentCpu = new Dictionary<(uint ProcessId, ulong CreationTime), TimeSpan>();
        var groups = new Dictionary<string, (string Name, long Memory, double CpuTicks, bool Measured)>(StringComparer.Ordinal);

        foreach (var process in processes)
        {
            if (process.SessionId != sessionId || process.CreationTime == 0 || ProcessNames.IsProtected(process.ImageName)) continue;
            var key = ProcessNames.Normalize(process.ImageName);
            if (key.Length == 0) continue;

            var identity = (process.ProcessId, process.CreationTime);
            currentCpu[identity] = process.ProcessorTime;
            var measured = _previousCpu.TryGetValue(identity, out var before) && elapsed > TimeSpan.Zero;
            var delta = measured ? Math.Max(0, (process.ProcessorTime - before).Ticks) : 0;

            groups[key] = groups.TryGetValue(key, out var group)
                ? (group.Name, group.Memory + process.WorkingSetBytes, group.CpuTicks + delta, group.Measured || measured)
                : (process.ImageName, process.WorkingSetBytes, delta, measured);
        }

        _previousCpu = currentCpu;
        _previousAt = now;

        return groups.Values
            .OrderBy(group => group.Name, StringComparer.OrdinalIgnoreCase)
            .Select(group => new ProcessItem(
                group.Name,
                group.Measured ? FormatCpu(group.CpuTicks / elapsed.Ticks / _processorCount * 100) : UnmeasuredMetric,
                FormatMemory(group.Memory),
                false))
            .ToList();
    }

    private static string FormatCpu(double percent) =>
        string.Create(CultureInfo.InvariantCulture, $"{Math.Clamp(percent, 0, 100):F1}%");

    private static string FormatMemory(long bytes) =>
        string.Create(CultureInfo.InvariantCulture, $"{bytes / (1024d * 1024d):F1} MB");
}
