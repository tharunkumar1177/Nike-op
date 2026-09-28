using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.Services;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class EngineProtocolTests
{
    private static readonly ProcessTarget Target = new(0x01020304, 0x1122334455667788, "a.exe");

    // Same vectors as the Rust tests engine_terminate_request_matches_the_winui_wire_layout and
    // engine_termination_response_matches_the_winui_wire_layout in crates/core/src/orchestration.rs.
    private static byte[] Header() =>
    [
        3, 0,
        1, 0, 0, 0, 0, 0, 0, 0, (byte)'c',
        1, 0, 0, 0, 0, 0, 0, 0, (byte)'r',
        5, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ];

    private static byte[] TargetBytes() =>
    [
        0x04, 0x03, 0x02, 0x01,
        0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11,
        5, 0, 0, 0, 0, 0, 0, 0, (byte)'a', (byte)'.', (byte)'e', (byte)'x', (byte)'e',
    ];

    private static byte[] TerminationResponse(params byte[] outcome) =>
        [.. Header(), 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, .. TargetBytes(), .. outcome];

    [Fact]
    public void TerminateRequestMatchesTheRustWireLayout()
    {
        // Verifies the exact Bincode bytes EngineSvc decodes as Envelope<EngineCommand::TerminateTargets>.
        byte[] expected = [.. Header(), 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, .. TargetBytes()];
        Assert.Equal(expected, EngineProtocol.EncodeTerminate("c", "r", 5, new[] { Target }));
    }

    [Fact]
    public void TerminationResponseDecodesPerTargetOutcomes()
    {
        // Verifies a tagged outcome with a payload decodes and marks the report as failed.
        var response = Assert.IsType<EngineResponse.Termination>(EngineProtocol.Decode(TerminationResponse(8, 0, 0, 0, 5, 0, 0, 0)));
        var outcome = Assert.Single(response.Report.Outcomes);
        Assert.Equal(Target, outcome.Target);
        Assert.Equal(TerminationOutcome.Failed, outcome.Outcome);
        Assert.Equal(5u, outcome.FailureCode);
        Assert.False(response.Report.IsSuccess);
        Assert.Equal("r", response.RequestId);

        var closed = Assert.IsType<EngineResponse.Termination>(EngineProtocol.Decode(TerminationResponse(0, 0, 0, 0)));
        Assert.Equal(TerminationOutcome.Terminated, Assert.Single(closed.Report.Outcomes).Outcome);
        Assert.True(closed.Report.IsSuccess);
    }

    [Fact]
    public void ErrorResponsesDecodeInFieldOrder()
    {
        // Verifies EngineEvent::Error decodes code, recoverable, and message in Rust declaration order.
        byte[] message =
        [
            .. Header(), 1, 0, 0, 0,
            1, 0, 0, 0, 0, 0, 0, 0, (byte)'x',
            1,
            2, 0, 0, 0, 0, 0, 0, 0, (byte)'n', (byte)'o',
        ];
        var error = Assert.IsType<EngineResponse.Error>(EngineProtocol.Decode(message));
        Assert.Equal(("x", true, "no"), (error.Code, error.Recoverable, error.Message));
    }

    [Fact]
    public void MalformedResponsesAreRejected()
    {
        // Verifies wrong versions, truncation, unknown tags, oversized counts, and bad string lengths fail closed.
        var wrongVersion = TerminationResponse(0, 0, 0, 0);
        wrongVersion[0] = 2;
        var unknownOutcome = TerminationResponse(9, 0, 0, 0);
        byte[] tooManyTargets = [.. Header(), 4, 0, 0, 0, 129, 0, 0, 0, 0, 0, 0, 0];
        byte[] badString = [.. Header(), 0, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0];
        byte[] unknownEvent = [.. Header(), 7, 0, 0, 0];
        var truncated = TerminationResponse(8, 0, 0, 0)[..^2];

        foreach (var message in new[] { wrongVersion, unknownOutcome, tooManyTargets, badString, unknownEvent, truncated })
            Assert.Throws<InvalidDataException>(() => EngineProtocol.Decode(message));
    }

    [Fact]
    public void RequestsOutsideTheTargetBoundsAreRefused()
    {
        // Verifies Settings never sends an empty or oversized termination request.
        Assert.Throws<ArgumentOutOfRangeException>(() => EngineProtocol.EncodeTerminate("c", "r", 5, Array.Empty<ProcessTarget>()));
        var tooMany = Enumerable.Range(1, EngineProtocol.MaximumTargets + 1).Select(pid => Target with { ProcessId = (uint)pid }).ToArray();
        Assert.Throws<ArgumentOutOfRangeException>(() => EngineProtocol.EncodeTerminate("c", "r", 5, tooMany));
    }
}
