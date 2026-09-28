# Process termination safety

## Outcome or responsibility

Critical Windows processes cannot be selected, terminated, or lowered in priority through alternate spelling, casing, or extension forms, or through a PID that no longer refers to the validated process.

## Current verified status

**Status:** Partial

Protected-name normalization and regression tests cover extensionless, mixed-case, and whitespace forms. The protected list now also covers Edge Optimizer's own executables, security products, and a few more shell and session processes.

Targets are `ProcessTarget { pid, creation_time, image_name }`; a bare name or bare PID is never a target.

`process::terminate_target` runs these checks, in order:

1. Request checks: reserved PIDs, malformed or oversized names, protected names, and the caller's own PID.
2. It opens the PID with a handle that pins the process.
3. Through that handle it re-checks the creation time and the image name from `QueryFullProcessImageNameW`, rejects a protected actual image, and confirms the process is still running.
4. It requires the target to be in the requester's session.
5. It rejects critical processes, failing closed if `IsProcessCritical` cannot be read.
6. It terminates through the same handle.

These steps have pure-logic unit tests. The live OS path has not been run. No code changes process priority yet.

## Architecture dependencies

- [Process safety](../architecture.md#process-safety)
- [Privilege and identity](../architecture.md#privilege-and-identity)

## Local rules and implications

### Process safety

UI validation is advisory. The broker performs every termination and must repeat validation against the resolved PID immediately before the operation. Priority lowering additionally rejects the audio, input, and display processes that [FPS Boost](fps-boost.md) lists.

PIDs change every time a process starts, so they are never stored. Profiles store names. Clients resolve names to targets at the moment of use, and trackers hold process handles, never bare PIDs, to follow liveness.

### Privilege and identity

Runner and Settings send validated targets to the broker, never raw names or commands. The broker limits targets to the requester's session.

## Related blueprints

### Required

None.

### Impact checks

- [Privileged broker](privileged-broker.md) — check when final authorization, termination, or priority validation changes.
- [System Tweaks](system-tweaks.md) — check when process selection or activation results change.
- [FPS Boost](fps-boost.md) — check when priority-lowering targets or never-lower rules change.

## Relevant implementation and tests

- `crates/core/src/process.rs` — normalization, protected names, `ProcessTarget`, target resolution, identity verification, validated termination, `WatchedProcess`, and unit tests.
- `crates/core/src/engine_commands.rs` — request-level rejection before any OS call, with fake-operation tests.
- `apps/EdgeOptimizer.Settings.Core/Services/ProcessNames.cs` — C# copy of normalization and protected names (advisory).

## Acceptance or verification criteria

- [x] Block `.exe` and extensionless forms case-insensitively.
- [x] Ignore surrounding whitespace for safety comparison.
- [x] Resolve a requested target to a PID plus creation time and re-check identity, session, and Windows critical state through one handle before terminating (logic tests; Windows evidence pending).
- [ ] Apply the same validation to priority-lowering operations.
- [x] Return a structured reason for every skipped PID.
- [ ] Exercise PID reuse, elevated same-session targets, and critical processes in an isolated Windows environment.

## Remaining gaps and unknowns

The never-lower process list has not been defined. The Rust and C# protected lists are maintained by hand and must be kept identical; only the Rust list is authoritative. Protected Process Light targets fail with access denied, which is reported as a failure rather than a skip.
