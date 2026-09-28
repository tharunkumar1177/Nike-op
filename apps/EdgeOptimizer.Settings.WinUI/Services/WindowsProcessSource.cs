using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.Services;
using Microsoft.Win32.SafeHandles;

namespace EdgeOptimizer.Settings.WinUI.Services;

/// <summary>
/// Enumerates processes in the signed-in user's session, read-only. Processes whose
/// creation time cannot be read are omitted because they cannot be targeted safely.
/// </summary>
public sealed class WindowsProcessSource : IProcessSource
{
    private const uint ProcessQueryLimitedInformation = 0x1000;

    public WindowsProcessSource()
    {
        using var current = Process.GetCurrentProcess();
        CurrentSessionId = current.SessionId;
        CurrentProcessId = checked((uint)current.Id);
    }

    public int CurrentSessionId { get; }
    public uint CurrentProcessId { get; }

    public IReadOnlyList<RunningProcess> Snapshot()
    {
        var processes = Process.GetProcesses();
        var result = new List<RunningProcess>(processes.Length);
        foreach (var process in processes)
        {
            using (process)
            {
                try
                {
                    if (process.Id is 0 or 4 || process.SessionId != CurrentSessionId) continue;
                    var processId = checked((uint)process.Id);
                    using var handle = OpenProcess(ProcessQueryLimitedInformation, false, processId);
                    if (handle.IsInvalid) continue;
                    if (!GetProcessTimes(handle, out var created, out _, out var kernel, out var user) || created == 0) continue;
                    var imageName = QueryImageName(handle) ?? $"{process.ProcessName}.exe";
                    result.Add(new RunningProcess(
                        processId,
                        created,
                        imageName,
                        process.SessionId,
                        process.WorkingSet64,
                        TimeSpan.FromTicks(checked((long)(kernel + user)))));
                }
                catch (Exception error) when (error is InvalidOperationException or Win32Exception or OverflowException)
                {
                    // The process exited or became inaccessible during enumeration.
                }
            }
        }
        return result;
    }

    private static string? QueryImageName(SafeProcessHandle handle)
    {
        var buffer = new char[1024];
        var length = (uint)buffer.Length;
        return QueryFullProcessImageName(handle, 0, buffer, ref length)
            ? Path.GetFileName(new string(buffer, 0, checked((int)length)))
            : null;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern SafeProcessHandle OpenProcess(uint desiredAccess, [MarshalAs(UnmanagedType.Bool)] bool inheritHandle, uint processId);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetProcessTimes(SafeProcessHandle process, out ulong creationTime, out ulong exitTime, out ulong kernelTime, out ulong userTime);

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode, EntryPoint = "QueryFullProcessImageNameW")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool QueryFullProcessImageName(SafeProcessHandle process, uint flags, [Out] char[] exeName, ref uint size);
}
