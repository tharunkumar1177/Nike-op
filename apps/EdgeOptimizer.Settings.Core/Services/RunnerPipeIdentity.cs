namespace EdgeOptimizer.Settings.Core.Services;

/// <summary>
/// Naming and server-identity rules for Runner's per-session Settings pipe.
/// The pipe name must match <c>SETTINGS_PIPE_BASE</c> and <c>session_pipe_name</c> in the Rust core.
/// </summary>
public static class RunnerPipeIdentity
{
    public const string PipeBaseName = "EdgeOptimizerIPC";
    public const string RunnerExecutableName = "EdgeOptimizer_Runner.exe";

    public static string PipeName(int sessionId) => $"{PipeBaseName}-{sessionId}";

    /// <summary>
    /// A pipe server is trusted only when it runs in the client's session from the
    /// Runner executable installed in the client's own directory.
    /// </summary>
    public static bool IsExpectedRunner(string? serverImagePath, int serverSessionId, string clientDirectory, int clientSessionId)
    {
        if (serverSessionId != clientSessionId || string.IsNullOrWhiteSpace(serverImagePath) || string.IsNullOrWhiteSpace(clientDirectory))
            return false;

        string serverPath;
        string clientPath;
        try
        {
            serverPath = Path.GetFullPath(serverImagePath);
            clientPath = Path.GetFullPath(clientDirectory);
        }
        catch (Exception error) when (error is ArgumentException or NotSupportedException or PathTooLongException)
        {
            return false;
        }

        var serverDirectory = Path.GetDirectoryName(serverPath);
        return serverDirectory is not null
            && string.Equals(Path.GetFileName(serverPath), RunnerExecutableName, StringComparison.OrdinalIgnoreCase)
            && string.Equals(Path.TrimEndingDirectorySeparator(serverDirectory), Path.TrimEndingDirectorySeparator(clientPath), StringComparison.OrdinalIgnoreCase);
    }
}
