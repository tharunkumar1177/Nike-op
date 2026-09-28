# System Tweaks

## Outcome or responsibility

A player can configure profile-scoped process termination and fan options, understand their safety and privilege requirements, preview the activation plan, and receive structured results when Runner applies explicitly authorized operations. Reversible performance adjustments belong to [FPS Boost](fps-boost.md), and cleanup belongs to [Disk cleanup](disk-cleanup.md).

## Current verified status

**Status:** Partial

Code inspection on 2026-09-05 confirmed:

- Rust profile storage for selected process names and a fan-speed flag;
- process-name normalization and protected-name tests;
- Runner-to-Engine command routing with fake-operation tests; and
- transitional Recycle Bin and browser-cache commands.

The WinUI page provides profile-scoped process selection, filtering, fan and cleanup toggles, selection totals, and restore-default behavior, with unit tests.

WinUI saves supported profile fields through Runner and requests a live read-only process snapshot. The per-profile cleanup toggles and run commands in `SystemTweaksViewModel` are transitional. Runner now executes cleanup requests in the user's context rather than in EngineSvc, but the controls stay disabled because the app-wide [Disk cleanup](disk-cleanup.md) page supersedes them. `fan_speed_max` is stored but not applied by Engine command dispatch. PID-level safety validation is not implemented.

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

Settings presents choices and validation feedback but never enumerates or terminates processes, changes fan policy, or opens durable state. Runner builds the activation plan, executes user-context work, and delegates only allowlisted machine-level operations to the broker.

### State ownership and persistence

Profile-scoped selections are durable only through Runner. Restoring a profile repopulates the UI but never terminates processes or applies fan policy until the user issues a new explicit activation command.

### IPC and protocol boundary

Process snapshots, activation previews, save commands, and structured per-operation results cross the WinUI/Runner contract. Runner/broker requests remain bounded, versioned, correlated, and independently authorized.

### Privilege and identity

Runner terminates processes owned by the interactive user itself. Only validated targets it cannot end go to the broker, which exists only in the Full edition. In the Store edition such targets are reported as skipped with the reason "requires Full edition".

### Process safety

UI filtering is advisory. Runner and the final execution boundary normalize targets and reject ambiguous, protected, critical, or changed process identities immediately before termination.

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
- `crates/core/src/process.rs` — process discovery, normalization, protected-name policy, termination, and safe unit tests.
- `crates/core/src/orchestration.rs` — activation messages and structured operation results.
- `crates/core/src/engine_commands.rs` — injectable Engine command routing and fake-operation tests.
- `crates/runner/src/main.rs` — activation, persistence, and Engine state transitions.
- `crates/engine_service/src/main.rs` — transitional process execution.
- `apps/EdgeOptimizer.Settings.Core/ViewModels/SystemTweaksViewModel.cs` — profile selections, live process presentation, and transitional cleanup toggles.
- `tests/EdgeOptimizer.Settings.Core.Tests/SystemTweaksViewModelTests.cs` — filtering, totals, safe defaults, and profile isolation tests.

## Acceptance or verification criteria

- [x] Store selected process names and the transitional fan flag per Rust profile.
- [x] Normalize protected process names across casing, whitespace, and `.exe` forms.
- [x] Route process intents through injectable Engine decision logic for safe hosted tests.
- [x] Keep WinUI process and toggle state independent between profiles, and restore safe defaults deterministically.
- [x] Replace fixture processes with a Runner-provided read-only snapshot.
- [ ] Add structured protected and ambiguous selection reasons.
- [ ] Terminate user-owned processes in Runner, and send only unreachable validated targets to the broker.
- [ ] Remove the per-profile cleanup toggles once [Disk cleanup](disk-cleanup.md) provides the app-wide page.
- [ ] Align WinUI 3 and Rust profile contracts for every supported tweak; never imply persistence for preview-only toggles.
- [ ] Define supported fan-policy hardware, authorization, apply, journaled revert, and unavailable behavior before enabling it.
- [ ] Validate PID identity and Windows critical or protected state immediately before termination.
- [ ] Preview the exact activation plan and return killed, missing, skipped, failed, and applied outcomes.
- [ ] Require explicit user intent for termination and machine changes; startup restoration performs no side effects.
- [ ] Verify privileged and destructive behavior only in an isolated Windows environment, never on hosted CI or a developer machine.

## Remaining gaps and unknowns

Runner-side termination of user-owned processes, a unified tweak schema, executable fan policy, PID validation, authenticated broker execution, and Windows integration coverage remain planned. Until those contracts exist, the WinUI 3 page must keep identifying unavailable actions as preview-only.
