namespace EdgeOptimizer.Settings.Core.Models;

/// <summary>
/// A process seen by enumeration. <see cref="CreationTime"/> is a Windows FILETIME;
/// together with <see cref="ProcessId"/> it identifies one process instance even
/// after Windows reuses the PID.
/// </summary>
public sealed record RunningProcess(
    uint ProcessId,
    ulong CreationTime,
    string ImageName,
    int SessionId,
    long WorkingSetBytes,
    TimeSpan ProcessorTime);

/// <summary>One specific process instance requested for termination.</summary>
public sealed record ProcessTarget(uint ProcessId, ulong CreationTime, string ImageName);

/// <summary>Per-target result from EngineSvc. Order matches the Rust enum's Bincode tags.</summary>
public enum TerminationOutcome
{
    Terminated,
    NotRunning,
    IdentityChanged,
    Protected,
    Critical,
    OutsideSession,
    AccessDenied,
    Invalid,
    Failed,
}

public sealed record TargetOutcome(ProcessTarget Target, TerminationOutcome Outcome, uint FailureCode = 0);

public sealed record TerminationReport(IReadOnlyList<TargetOutcome> Outcomes)
{
    public static bool IsFailure(TerminationOutcome outcome) =>
        outcome is TerminationOutcome.AccessDenied or TerminationOutcome.Invalid or TerminationOutcome.Failed;

    public bool IsSuccess => !Outcomes.Any(entry => IsFailure(entry.Outcome));

    public string Summary
    {
        get
        {
            if (Outcomes.Count == 0) return "No selected apps were running.";
            var closed = Outcomes.Count(entry => entry.Outcome == TerminationOutcome.Terminated);
            var gone = Outcomes.Count(entry => entry.Outcome is TerminationOutcome.NotRunning or TerminationOutcome.IdentityChanged);
            var skipped = Outcomes.Count(entry => entry.Outcome is TerminationOutcome.Protected or TerminationOutcome.Critical or TerminationOutcome.OutsideSession);
            var failed = Outcomes.Count(entry => IsFailure(entry.Outcome));
            return $"Closed {closed}, already closed {gone}, skipped {skipped}, failed {failed}.";
        }
    }
}
