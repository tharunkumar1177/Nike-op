using EdgeOptimizer.Settings.Core.Models;
using EdgeOptimizer.Settings.Core.Services;

namespace EdgeOptimizer.Settings.Core.Tests;

internal sealed class FakeFilePicker(string? path) : IFilePicker
{
    public Task<string?> PickPngAsync(CancellationToken cancellationToken = default) => Task.FromResult(path);
}

internal sealed class FakeRunnerClient(bool connected = false) : IRunnerClient
{
    public bool IsConnected { get; set; } = connected;
    public Exception? SendFailure { get; set; }
    public int StartCount { get; private set; }
    public List<ProfileWorkspace> SavedProfiles { get; } = new();
    public List<ProfileWorkspace> ActivatedProfiles { get; } = new();
    public List<string> CleanupRequests { get; } = new();
    public int ProcessSnapshotRequests { get; private set; }
    public event EventHandler<bool>? ConnectionChanged;
    public event EventHandler<RunnerSnapshot>? SnapshotReceived;
    public event EventHandler<string>? StatusReceived;
    public event EventHandler<IReadOnlyList<ProcessItem>>? ProcessSnapshotReceived;
    public event EventHandler<string?>? ActiveProfileChanged;
    public event EventHandler<RunnerWindowCommand>? WindowCommandReceived;

    public Task StartAsync(CancellationToken cancellationToken = default)
    {
        StartCount++;
        return Task.CompletedTask;
    }

    public Task SaveProfilesAsync(IReadOnlyList<ProfileWorkspace> profiles, CancellationToken cancellationToken = default)
    {
        ThrowIfFailing();
        SavedProfiles.AddRange(profiles);
        return Task.CompletedTask;
    }

    public Task SetActiveProfileAsync(string? profileName, CancellationToken cancellationToken = default) => Task.CompletedTask;
    public Task SetOverlayVisibilityAsync(bool visible, CancellationToken cancellationToken = default) => Task.CompletedTask;
    public Task ActivateProfileAsync(ProfileWorkspace profile, CancellationToken cancellationToken = default) { ThrowIfFailing(); ActivatedProfiles.Add(profile); return Task.CompletedTask; }
    public Task RequestCleanupAsync(string cleanupKind, CancellationToken cancellationToken = default) { ThrowIfFailing(); CleanupRequests.Add(cleanupKind); return Task.CompletedTask; }
    public Task RequestProcessSnapshotAsync(CancellationToken cancellationToken = default) { ThrowIfFailing(); ProcessSnapshotRequests++; return Task.CompletedTask; }

    public void RaiseSnapshot(RunnerSnapshot snapshot) => SnapshotReceived?.Invoke(this, snapshot);
    public void RaiseActiveProfile(string? name) => ActiveProfileChanged?.Invoke(this, name);
    public void RaiseConnection(bool isConnected) { IsConnected = isConnected; ConnectionChanged?.Invoke(this, isConnected); }

    private void ThrowIfFailing()
    {
        if (SendFailure is not null) throw SendFailure;
    }
}

internal static class Fixtures
{
    public static ProfileWorkspace ConfiguredProfile(string name = "Configured")
    {
        var profile = new ProfileWorkspace(name, false)
        {
            CrosshairImagePath = @"C:\fixtures\dot.png",
            CrosshairImageName = "dot.png",
        };
        profile.Macros.Add(new MacroDefinition("Build combo", "Ctrl + F8", new[]
        {
            new MacroStep("Key down", "W"),
            new MacroStep("Wait", "120 ms"),
            new MacroStep("Key press", "Space"),
            new MacroStep("Key up", "W"),
        }));
        profile.Macros.Add(new MacroDefinition("Quick heal", MacroValidation.Unassigned, new[] { new MacroStep("Key press", "H") }));
        profile.Processes.Add(new ProcessItem("Discord.exe", "0.4%", "156.2 MB", true));
        profile.Processes.Add(new ProcessItem("chrome.exe", "1.2%", "512.7 MB", true));
        profile.Processes.Add(new ProcessItem("Spotify.exe", "0.3%", "123.4 MB", true));
        profile.Processes.Add(new ProcessItem("Steam.exe", "0.6%", "287.9 MB", false));
        return profile;
    }

    public static RunnerSnapshot Snapshot(string? activeName, params ProfileWorkspace[] profiles) =>
        new(profiles, activeName, false);
}
