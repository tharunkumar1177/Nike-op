using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.Services;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class ProcessTrackingTests
{
    private static RunningProcess Running(uint pid, string name, ulong creationTime, double cpuSeconds, long workingSetMb = 100, int session = 1) =>
        new(pid, creationTime, name, session, workingSetMb * 1024 * 1024, TimeSpan.FromSeconds(cpuSeconds));

    [Fact]
    public void NamesNormalizeLikeTheRustCore()
    {
        // Verifies casing, padding, and extension forms compare equal and protected names cannot be bypassed.
        Assert.Equal("notepad", ProcessNames.Normalize("  Notepad.ExE  "));
        Assert.Equal("notepad", ProcessNames.Normalize(" notepad .exe"));
        Assert.True(ProcessNames.Match("Discord", " discord.EXE "));
        Assert.False(ProcessNames.Match("", ".exe"));
        foreach (var name in new[] { "csrss", "CSRSS.EXE", " svchost.exe ", "MsMpEng.exe", "EdgeOptimizer_EngineSvc.exe", "Memory Compression" })
            Assert.True(ProcessNames.IsProtected(name), name);
        Assert.False(ProcessNames.IsProtected("chrome.exe"));
    }

    [Fact]
    public void SamplerMeasuresCpuBetweenSamplesPerExecutable()
    {
        // Verifies CPU appears after a second sample and instances of one executable are summed.
        var sampler = new ProcessSampler(processorCount: 1);
        var start = DateTimeOffset.UnixEpoch;
        var first = sampler.Sample(new[] { Running(1, "chrome.exe", 10, 1, 100), Running(2, "Chrome.exe", 20, 1, 50) }, 1, start);
        var chrome = Assert.Single(first);
        Assert.Equal(ProcessSampler.UnmeasuredMetric, chrome.Cpu);
        Assert.Equal("150.0 MB", chrome.Memory);

        var second = sampler.Sample(new[] { Running(1, "chrome.exe", 10, 2, 100), Running(2, "Chrome.exe", 20, 1.5, 50) }, 1, start.AddSeconds(10));
        Assert.Equal("15.0%", Assert.Single(second).Cpu);
    }

    [Fact]
    public void SamplerNeverCarriesCpuHistoryAcrossPidReuse()
    {
        // Verifies a recycled PID with a new creation time starts unmeasured instead of inheriting old CPU time.
        var sampler = new ProcessSampler(processorCount: 1);
        var start = DateTimeOffset.UnixEpoch;
        sampler.Sample(new[] { Running(7, "game.exe", 100, 50) }, 1, start);
        var reused = sampler.Sample(new[] { Running(7, "game.exe", 200, 1) }, 1, start.AddSeconds(5));
        Assert.Equal(ProcessSampler.UnmeasuredMetric, Assert.Single(reused).Cpu);
    }

    [Fact]
    public void SamplerHidesProtectedOtherSessionAndUnidentifiedProcesses()
    {
        // Verifies only processes that could be targeted safely are offered for selection.
        var sampler = new ProcessSampler(processorCount: 1);
        var rows = sampler.Sample(new[]
        {
            Running(1, "game.exe", 10, 1),
            Running(2, "explorer.exe", 20, 1),
            Running(3, "other.exe", 30, 1, session: 2),
            Running(4, "unknown.exe", 0, 1),
        }, 1, DateTimeOffset.UnixEpoch);
        Assert.Equal("game.exe", Assert.Single(rows).Name);
    }

    [Fact]
    public void PlannerTargetsEveryMatchingInstanceByIdentity()
    {
        // Verifies selection names expand to same-session instances and exclude protected, reserved, self, and unidentified processes.
        var processes = new[]
        {
            Running(100, "chrome.exe", 1_100, 1),
            Running(101, "Chrome.exe", 1_101, 1),
            Running(102, "chrome.exe", 1_102, 1, session: 2),
            Running(103, "svchost.exe", 1_103, 1),
            Running(104, "chrome.exe", 0, 1),
            Running(4, "chrome.exe", 1_004, 1),
            Running(999, "chrome.exe", 1_999, 1),
            Running(100, "chrome.exe", 1_100, 1),
        };
        var targets = TerminationPlanner.Plan(new[] { " CHROME ", "svchost", "" }, processes, 1, 999);
        Assert.Equal(
            new[] { new ProcessTarget(100, 1_100, "chrome.exe"), new ProcessTarget(101, 1_101, "Chrome.exe") },
            targets);
    }

    [Fact]
    public void PlannerCapsTargetsAtTheEngineLimit()
    {
        // Verifies a single activation never produces a request EngineSvc would reject for size.
        var processes = Enumerable.Range(10, EngineProtocol.MaximumTargets + 20)
            .Select(pid => Running((uint)pid, "worker.exe", (ulong)pid, 1))
            .ToArray();
        Assert.Equal(EngineProtocol.MaximumTargets, TerminationPlanner.Plan(new[] { "worker" }, processes, 1, 1).Count);
    }
}
