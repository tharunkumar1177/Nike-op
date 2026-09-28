using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Security.Principal;
using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.Services;
using Microsoft.Win32.SafeHandles;

namespace EdgeOptimizer.Settings.WinUI.Services;

/// <summary>
/// Transitional Bincode client for EngineSvc. The pipe is used only when SYSTEM owns
/// it, which a standard-user squatter cannot arrange; EngineSvc in turn accepts only
/// this executable and Runner from its own install directory.
/// </summary>
public sealed class EngineServiceClient : IEngineClient
{
    private const int ConnectTimeoutMilliseconds = 3000;
    private static readonly TimeSpan ResponseTimeout = TimeSpan.FromSeconds(15);

    public async Task<TerminationReport> TerminateAsync(IReadOnlyList<ProcessTarget> targets, CancellationToken cancellationToken = default)
    {
        var now = (ulong)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var requestId = $"winui-{now}-{Guid.NewGuid():N}";
        var request = EngineProtocol.EncodeTerminate(EngineProtocol.ClientId, requestId, now, targets);
        var response = await ExchangeAsync(request, cancellationToken);
        return response switch
        {
            EngineResponse.Termination termination => termination.Report,
            EngineResponse.Error error => throw new EngineUnavailableException($"The engine service rejected the request: {error.Message}"),
            _ => throw new EngineUnavailableException("The engine service sent an unexpected response."),
        };
    }

    private static async Task<EngineResponse> ExchangeAsync(byte[] request, CancellationToken cancellationToken)
    {
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        timeout.CancelAfter(ResponseTimeout);
        try
        {
            await using var pipe = new NamedPipeClientStream(".", EngineProtocol.PipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
            await pipe.ConnectAsync(ConnectTimeoutMilliseconds, timeout.Token);
            VerifySystemOwnedPipe(pipe);
            pipe.ReadMode = PipeTransmissionMode.Message;
            await pipe.WriteAsync(request, timeout.Token);
            await pipe.FlushAsync(timeout.Token);
            return EngineProtocol.Decode(await ReadMessageAsync(pipe, timeout.Token));
        }
        catch (TimeoutException error)
        {
            throw new EngineUnavailableException("The engine service is not running.", error);
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            throw new EngineUnavailableException("The engine service did not respond in time.", error);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or InvalidDataException)
        {
            throw new EngineUnavailableException($"The engine service could not be used: {error.Message}", error);
        }
    }

    private static async Task<byte[]> ReadMessageAsync(NamedPipeClientStream pipe, CancellationToken cancellationToken)
    {
        using var message = new MemoryStream();
        var chunk = new byte[8192];
        do
        {
            var count = await pipe.ReadAsync(chunk, cancellationToken);
            if (count == 0) throw new IOException("The engine service closed the connection without responding.");
            message.Write(chunk, 0, count);
            if (message.Length > EngineProtocol.MaximumMessageBytes) throw new InvalidDataException("The engine response exceeds the size limit.");
        } while (!pipe.IsMessageComplete);
        return message.ToArray();
    }

    private static void VerifySystemOwnedPipe(NamedPipeClientStream pipe)
    {
        const int SeKernelObject = 6;
        const uint OwnerSecurityInformation = 0x1;
        var status = GetSecurityInfo(pipe.SafePipeHandle, SeKernelObject, OwnerSecurityInformation, out var owner, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, out var descriptor);
        try
        {
            if (status != 0) throw new UnauthorizedAccessException($"The engine pipe owner could not be read (Win32 error {status}).");
            if (!new SecurityIdentifier(owner).IsWellKnown(WellKnownSidType.LocalSystemSid))
                throw new UnauthorizedAccessException("The engine pipe is not owned by SYSTEM; refusing to use it.");
        }
        finally
        {
            if (descriptor != IntPtr.Zero) LocalFree(descriptor);
        }
    }

    [DllImport("advapi32.dll")]
    private static extern uint GetSecurityInfo(SafePipeHandle handle, int objectType, uint securityInfo, out IntPtr owner, IntPtr group, IntPtr dacl, IntPtr sacl, out IntPtr securityDescriptor);

    [DllImport("kernel32.dll")]
    private static extern IntPtr LocalFree(IntPtr memory);
}
