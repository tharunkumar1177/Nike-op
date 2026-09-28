using EdgeOptimizer.Settings.Core.Services;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class RunnerPipeIdentityTests
{
    private const string InstallDirectory = @"C:\Program Files\Edge Optimizer";

    [Fact]
    public void PipeNameMatchesTheRustSessionNamingContract()
    {
        // Verifies the C# client derives the same per-session name as the Rust core's session_pipe_name.
        Assert.Equal("EdgeOptimizerIPC-3", RunnerPipeIdentity.PipeName(3));
        Assert.NotEqual(RunnerPipeIdentity.PipeName(1), RunnerPipeIdentity.PipeName(2));
    }

    [Theory]
    [InlineData(@"C:\Program Files\Edge Optimizer\EdgeOptimizer_Runner.exe", InstallDirectory)]
    [InlineData(@"C:\Program Files\Edge Optimizer\edgeoptimizer_runner.EXE", InstallDirectory)]
    [InlineData(@"C:\Program Files\Edge Optimizer\EdgeOptimizer_Runner.exe", InstallDirectory + @"\")]
    public void AcceptsTheSiblingRunnerInTheSameSession(string serverImage, string clientDirectory)
    {
        // Verifies casing and a trailing separator do not reject the genuine Runner beside the client.
        Assert.True(RunnerPipeIdentity.IsExpectedRunner(serverImage, 2, clientDirectory, 2));
    }

    [Theory]
    [InlineData(@"C:\Users\Public\EdgeOptimizer_Runner.exe", 2)]
    [InlineData(@"C:\Program Files\Edge Optimizer\Other.exe", 2)]
    [InlineData(@"C:\Program Files\Edge Optimizer\sub\EdgeOptimizer_Runner.exe", 2)]
    [InlineData(@"C:\Program Files\Edge Optimizer\EdgeOptimizer_Runner.exe", 5)]
    [InlineData("", 2)]
    [InlineData(null, 2)]
    public void RejectsServersOutsideTheInstallDirectoryOrSession(string? serverImage, int serverSession)
    {
        // Verifies a pipe squatter from another folder, executable, or session is never trusted.
        Assert.False(RunnerPipeIdentity.IsExpectedRunner(serverImage, serverSession, InstallDirectory, 2));
    }
}
