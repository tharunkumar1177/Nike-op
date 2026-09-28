# System Tweaks

## Outcome or responsibility

A player can configure profile-scoped process termination and fan options, understand their safety and privilege requirements, and receive structured per-process results when EngineSvc closes the selected apps on activation or when the player ends them from Runner's quick flyout. Reversible performance adjustments belong to [FPS Boost](fps-boost.md), and cleanup belongs to [Disk cleanup](disk-cleanup.md).

## Current verified status

**Status:** Partial

Code inspection on 2026-09-28. Unit tests were written, but CI has not run them yet. No termination has been exercised on Windows.

- Profiles store selected process *names*. A name is resolved to specific instances only at the moment of use. Each instance is identified by PID plus creation time (`ProcessTarget`), so a recycled PID never matches.
- **Settings enumerates processes itself.** `WindowsProcessSource` reads processes read-only in the user's own session. `SystemTweaksViewModel` refreshes them every 3 seconds, and only while the System Tweaks page is shown and the Settings window is active; otherwise the fetcher is idle. Rows update in place. Protected, other-session, and unidentifiable processes are not listed. Selected apps that are not running stay selected.
- **Activation from Settings.** Settings saves the profile through Runner, resolves the selected names to live targets (`TerminationPlanner`), and sends them directly to EngineSvc (`EngineServiceClient`). It then asks Runner to start the profile's workers. If EngineSvc is unavailable, activation still proceeds and the page reports that apps were not closed.
- **Flyout.** Runner's quick flyout lists the active profile's running apps and offers **End** and **End all**. Runner tracks those processes with open handles only while the flyout is visible, and sends the targets to EngineSvc from a worker thread.
- **EngineSvc** validates each target and re-checks its live PID before terminating (see [Process safety](process-safety.md)). It returns one outcome per target: closed, already closed, identity changed, protected, critical, other session, access denied, invalid, or failed.
- Runner no longer sends process snapshots to Settings, and Settings-driven activation no longer asks Runner to terminate anything.

The fan toggle is disabled to match its unavailable state. The per-profile cleanup toggles and run commands in `SystemTweaksViewModel` are transitional and are not shown. `fan_speed_max` is stored but not applied.

## Architecture dependencies

- [Component boundaries](../architecture.md#component-boundaries)
- [State ownership and persistence](../architecture.md#state-ownership-and-persistence)
- [IPC and protocol boundary](../architecture.md#ipc-and-protocol-boundary)
- [Privilege and identity](../architecture.md#privilege-and-identity)
- [Process safety](../architecture.md#process-safety)
- [Reversible system changes](../architecture.md#reversible-system-changes)
- [Failure and recovery](../architecture.md#failure-and-recovery)

## Local rules and implications

### Component boundaries

Settings enumerates processes read-only, presents choices, and on activation sends specific targets to EngineSvc. It never terminates processes itself, changes fan policy, or opens durable state. Runner owns workers, persistence, and the flyout; flyout termination also goes to EngineSvc.

The Settings fetcher must idle when the System Tweaks page is not shown or the Settings window is inactive or minimized. The flyout's tracker must idle and release its handles when the flyout is hidden.

### State ownership and persistence

Profile-scoped selections are durable only through Runner. Restoring a profile repopulates the UI but never terminates processes or applies fan policy until the user issues a new explicit activation command. Tracked PIDs and handles are never persisted.

### IPC and protocol boundary

Save and activation commands cross the WinUI/Runner contract. Termination requests (`EngineCommand::TerminateTargets`) and per-target results cross the Settings/EngineSvc and Runner/EngineSvc contracts. Each request is bounded (at most 128 targets and 64 KiB), versioned, and correlated.

### Privilege and identity

EngineSvc performs every termination for both clients. It exists only in the Full edition. In the Store edition, activation still starts workers, and the result states that apps were not closed.

### Process safety

UI filtering is advisory. Settings and Runner exclude protected, other-session, and unidentifiable processes before sending. EngineSvc repeats all request checks and then re-validates the live PID immediately before termination.

### Reversible system changes

Process termination is irreversible and requires explicit activation. Fan policy, if enabled later, is a reversible change and must be journaled.

### Failure and recovery

Partial results remain visible per operation. A failed termination or fan-policy change is never reported as full activation success, silently retried with broader privilege, or allowed to corrupt the last valid profile state.

## Related blueprints

### Required

- [Profile persistence](profile-persistence.md) — owns durable profile selections and startup restoration semantics.
- [IPC contracts](ipc-contracts.md) — carries process snapshots, activation plans, commands, and results.
- [Process safety](process-safety.md) — defines normalization and final process validation.
- [Privileged broker](privileged-broker.md) — owns allowlisted machine-level execution and verified identity.

### Impact checks

- [Settings client](settings-client.md) — check when WinUI presentation, accessibility, navigation, or unavailable states change.
- [FPS Boost](fps-boost.md) — check when activation ordering or process selection is shared with priority adjustments.
- [Disk cleanup](disk-cleanup.md) — check when removing the transitional per-profile cleanup toggles.

## Relevant implementation and tests

- `crates/core/src/profile.rs` — selected processes and transitional fan-speed profile flag.
- `crates/core/src/process.rs` — `ProcessTarget` identity, enumeration, normalization, protected names, target resolution, validated termination, `WatchedProcess`, and unit tests.
- `crates/core/src/orchestration.rs` — `EngineCommand`/`EngineEvent` and byte-layout tests shared with C#.
- `crates/core/src/engine_commands.rs` — EngineSvc dispatch with request bounds and fake-operation tests.
- `crates/engine_service/src/main.rs` — client authentication and termination.
- `crates/runner/src/tracker.rs`, `crates/runner/src/engine_client.rs`, `crates/runner/src/main.rs` — flyout tracking, off-thread EngineSvc calls, activation, and persistence.
- `apps/EdgeOptimizer.Settings.Core/Services/ProcessSampler.cs`, `TerminationPlanner.cs`, `ProcessNames.cs`, `EngineProtocol.cs` — Settings-side listing, target planning, and codec.
- `apps/EdgeOptimizer.Settings.WinUI/Services/WindowsProcessSource.cs`, `EngineServiceClient.cs` — Windows enumeration and the EngineSvc pipe client.
- `apps/EdgeOptimizer.Settings.Core/ViewModels/SystemTweaksViewModel.cs`, `MainWindowViewModel.cs` — idle-aware fetcher and activation flow.
- `tests/EdgeOptimizer.Settings.Core.Tests/SystemTweaksViewModelTests.cs`, `ProcessTrackingTests.cs`, `EngineProtocolTests.cs`, `MainWindowViewModelTests.cs` — merge, idling, sampling, planning, codec, and activation tests.

## Acceptance or verification criteria

- [x] Store selected process names and the transitional fan flag per Rust profile.
- [x] Normalize protected process names across casing, whitespace, and `.exe` forms.
- [x] Route process intents through injectable Engine decision logic for safe hosted tests.
- [x] Keep WinUI process and toggle state independent between profiles, and restore safe defaults deterministically.
- [x] List running processes in Settings, read-only and same-session, without Runner snapshots.
- [x] Idle the Settings fetcher when the System Tweaks page is hidden or the window is inactive.
- [x] Resolve selected names to PID-plus-creation-time targets at activation, and send them from Settings directly to EngineSvc.
- [x] End the active profile's running apps from Runner's flyout through EngineSvc.
- [ ] Add structured protected and ambiguous selection reasons in the UI.
- [ ] Remove the per-profile cleanup toggles once [Disk cleanup](disk-cleanup.md) provides the app-wide page.
- [ ] Align WinUI 3 and Rust profile contracts for every supported tweak; never imply persistence for preview-only toggles.
- [ ] Define supported fan-policy hardware, authorization, apply, journaled revert, and unavailable behavior before enabling it.
- [x] Validate PID identity and Windows critical or protected state immediately before termination (logic and unit tests; Windows evidence pending).
- [x] Return per-target closed, already closed, skipped, and failed outcomes.
- [ ] Preview the exact activation plan before activation.
- [ ] Require explicit user intent for termination and machine changes; startup restoration performs no side effects.
- [ ] Verify privileged and destructive behavior only in an isolated Windows environment, never on hosted CI or a developer machine.

## Remaining gaps and unknowns

A unified tweak schema, executable fan policy, an activation preview, and Windows integration coverage remain planned. Termination has been checked only by code inspection and fake-operation unit tests. Real enumeration, PID reuse, session and critical checks, and the pipe's SYSTEM-ownership check need an isolated Windows test environment. Whether a standard-user Settings process can read `PROCESS_QUERY_LIMITED_INFORMATION` for every same-session elevated app is unverified; such apps are omitted from the list when it cannot.
