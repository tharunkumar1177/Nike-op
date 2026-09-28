using System.Text;
using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.Services;

/// <summary>
/// Transitional Bincode codec for EngineSvc's <c>Envelope&lt;EngineCommand&gt;</c> and
/// <c>Envelope&lt;EngineEvent&gt;</c> (see <c>crates/core/src/orchestration.rs</c>). The Rust
/// tests assert the same byte vectors as <c>EngineProtocolTests</c>.
/// </summary>
public static class EngineProtocol
{
    public const ushort ProtocolVersion = 3;
    public const string PipeName = "EdgeOptimizerEngineIPC";
    public const int MaximumMessageBytes = 64 * 1024;
    public const int MaximumTargets = 128;
    public const int MaximumImageNameBytes = 255;
    public const string ClientId = "edge-settings-winui";

    private const uint TerminateTargetsTag = 0;
    private const uint PingTag = 2;

    public static byte[] EncodeTerminate(string clientId, string requestId, ulong timestampUnixMs, IReadOnlyList<ProcessTarget> targets)
    {
        if (targets.Count is 0 or > MaximumTargets)
            throw new ArgumentOutOfRangeException(nameof(targets), $"A request must contain between 1 and {MaximumTargets} targets.");
        return Encode(clientId, requestId, timestampUnixMs, writer =>
        {
            writer.Write(TerminateTargetsTag);
            writer.Write((ulong)targets.Count);
            foreach (var target in targets)
            {
                writer.Write(target.ProcessId);
                writer.Write(target.CreationTime);
                WriteString(writer, target.ImageName);
            }
        });
    }

    public static byte[] EncodePing(string clientId, string requestId, ulong timestampUnixMs) =>
        Encode(clientId, requestId, timestampUnixMs, writer => writer.Write(PingTag));

    public static EngineResponse Decode(byte[] message)
    {
        if (message.Length > MaximumMessageBytes) throw new InvalidDataException("The engine response exceeds the size limit.");
        using var reader = new BinaryReader(new MemoryStream(message, writable: false), Encoding.UTF8, leaveOpen: false);
        try
        {
            var version = reader.ReadUInt16();
            if (version != ProtocolVersion) throw new InvalidDataException($"Unsupported engine protocol version {version}.");
            _ = ReadString(reader);
            var requestId = ReadString(reader);
            _ = reader.ReadUInt64();
            _ = reader.ReadUInt32(); // serialized identity claim; never trusted
            return reader.ReadUInt32() switch
            {
                0 => new EngineResponse.Ack(requestId, ReadString(reader)),
                1 => new EngineResponse.Error(requestId, ReadString(reader), reader.ReadBoolean(), ReadString(reader)),
                2 => new EngineResponse.Capabilities(requestId, reader.ReadBoolean()),
                3 => new EngineResponse.Pong(requestId),
                4 => new EngineResponse.Termination(requestId, ReadReport(reader)),
                var tag => throw new InvalidDataException($"Unknown engine response {tag}."),
            };
        }
        catch (EndOfStreamException error)
        {
            throw new InvalidDataException("The engine response is truncated.", error);
        }
    }

    private static TerminationReport ReadReport(BinaryReader reader)
    {
        var count = reader.ReadUInt64();
        if (count > MaximumTargets) throw new InvalidDataException("The engine response lists too many targets.");
        var outcomes = new List<TargetOutcome>((int)count);
        for (var index = 0UL; index < count; index++)
        {
            var target = new ProcessTarget(reader.ReadUInt32(), reader.ReadUInt64(), ReadString(reader));
            var tag = reader.ReadUInt32();
            if (tag > (uint)TerminationOutcome.Failed) throw new InvalidDataException($"Unknown termination outcome {tag}.");
            var outcome = (TerminationOutcome)tag;
            var code = outcome == TerminationOutcome.Failed ? reader.ReadUInt32() : 0u;
            outcomes.Add(new TargetOutcome(target, outcome, code));
        }
        return new TerminationReport(outcomes);
    }

    private static byte[] Encode(string clientId, string requestId, ulong timestampUnixMs, Action<BinaryWriter> writePayload)
    {
        using var buffer = new MemoryStream();
        using (var writer = new BinaryWriter(buffer, Encoding.UTF8, leaveOpen: true))
        {
            writer.Write(ProtocolVersion);
            WriteString(writer, clientId);
            WriteString(writer, requestId);
            writer.Write(timestampUnixMs);
            writer.Write(0u); // AuthContext::InteractiveUser (claim only)
            writePayload(writer);
        }
        if (buffer.Length > MaximumMessageBytes) throw new InvalidDataException("The engine request exceeds the size limit.");
        return buffer.ToArray();
    }

    private static void WriteString(BinaryWriter writer, string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        writer.Write((ulong)bytes.Length);
        writer.Write(bytes);
    }

    private static string ReadString(BinaryReader reader)
    {
        var length = reader.ReadUInt64();
        if (length > (ulong)(reader.BaseStream.Length - reader.BaseStream.Position))
            throw new InvalidDataException("The engine response contains an invalid string length.");
        return Encoding.UTF8.GetString(reader.ReadBytes((int)length));
    }
}

public abstract record EngineResponse(string RequestId)
{
    public sealed record Ack(string RequestId, string Message) : EngineResponse(RequestId);
    public sealed record Error(string RequestId, string Code, bool Recoverable, string Message) : EngineResponse(RequestId);
    public sealed record Capabilities(string RequestId, bool SupportsProcessTermination) : EngineResponse(RequestId);
    public sealed record Pong(string RequestId) : EngineResponse(RequestId);
    public sealed record Termination(string RequestId, TerminationReport Report) : EngineResponse(RequestId);
}
