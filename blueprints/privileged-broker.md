# Privileged broker and cleanup

## Outcome or responsibility

In the Full edition, a least-privilege Windows service performs a small allowlist of named machine-level operations for an authenticated Runner and journals every reversible change. User cleanup and all interactive-user work stay in Runner. The Store edition ships no broker.

## Current verified status

**Status:** Planned

Code inspection on 2026-09-28:

- `crates/engine_service/src/main.rs` runs as a console process with no Service Control Manager integration.
- `scripts/install-engine-service.ps1` registers that process as a SYSTEM scheduled task.
- `crates/core/src/engine_ipc.rs` creates the pipe with default security.
- The engine supports name-based process termination, Recycle Bin cleanup, and browser-cache cleanup. Its browser-cache paths come from the SYSTEM environment.
- `AuthContext` values are claims only.
- `crates/engine_ctl` and `scripts/register-cleanup-task.ps1` provide a transitional scheduled cleanup path into this engine.

## Architecture dependencies

- [Privilege and identity](../architecture.md#privilege-and-identity)
- [Reversible system changes](../architecture.md#reversible-system-changes)
- [Distribution editions and install layout](../architecture.md#distribution-editions-and-install-layout)
- [Process safety](../architecture.md#process-safety)
- [Failure and recovery](../architecture.md#failure-and-recovery)

## Local rules and implications

### Privilege and identity

The broker runs as a Windows service installed only by the Full edition installer.

**Pipe security.** The pipe's security descriptor grants connect access to interactive users and denies network logons. For each connection, the broker:

- reads the client process ID from the pipe;
- confirms the client image path is the installed Runner beside the broker; and
- records the client's token SID and session.

Each request is then authorized against the operation allowlist. The broker never trusts `AuthContext` or other serialized identity.

**Planned allowlist:**

- process termination of validated targets Runner cannot end itself;
- machine-level cleanup categories from [Disk cleanup](disk-cleanup.md);
- session-only pause and resume of the FPS Boost service identifiers;
- creation, activation, and removal of the dedicated core-parking power plan;
- priority changes for validated processes outside the caller's session; and
- a standby memory list purge.

Recycle Bin and browser-cache cleanup move out of the broker into Runner.

### Reversible system changes

The broker keeps its machine journal under `%ProgramData%`, with an ACL that allows access only to SYSTEM and Administrators. On service start it reverts outstanding entries before accepting new requests. On controlled shutdown it reverts entries for sessions that have ended.

### Distribution editions and install layout

The WiX installer registers the service, its recovery actions, and its security descriptor, and removes all three on uninstall. The scheduled-task installer and `engine_ctl` are retired once the service host exists.

### Process safety

Before termination or a priority change, the broker is the final boundary: it repeats name normalization and resolves and re-checks the target PID's image identity and critical or protected status.

### Failure and recovery

The service reports readiness through the Service Control Manager, uses SCM recovery actions for unexpected exits, logs operations to the Windows Event Log, and returns structured per-operation errors. It never broadens an operation to recover from a failure.

## Related blueprints

### Required

- [IPC contracts](ipc-contracts.md) — broker transport, framing, and peer identity.
- [Process safety](process-safety.md) — final termination and priority validation.

### Impact checks

- [System Tweaks](system-tweaks.md) — check when process-termination classification changes.
- [Disk cleanup](disk-cleanup.md) — check when machine-level cleanup categories change.
- [FPS Boost](fps-boost.md) — check when service, power, priority, or memory operations change.
- [Packaging and distribution](packaging-and-distribution.md) — check when service registration, recovery, or install layout changes.

## Relevant implementation and tests

- `crates/engine_service/src/main.rs` — transitional privileged worker.
- `crates/core/src/engine_ipc.rs` — transitional pipe.
- `crates/core/src/engine_commands.rs` — injectable command dispatch with fake-operation tests.
- `crates/engine_ctl/src/main.rs` — transitional control client, to be removed.
- `scripts/install-engine-service.ps1` — scheduled-task installer to replace.
- `scripts/uninstall-engine-service.ps1` — scheduled-task removal to replace.

## Acceptance or verification criteria

- [ ] Install and run under the Windows Service Control Manager from the Full edition installer.
- [ ] Apply an explicit pipe ACL and verify the connecting token and that the client image is the installed Runner.
- [ ] Enforce the named-operation allowlist and per-operation authorization; reject everything else.
- [ ] Move user-specific cleanup to Runner in the verified interactive-user context.
- [ ] Journal machine-level reversible changes and revert outstanding entries on service start.
- [ ] Support service recovery, controlled shutdown, and Event Log operational logging.
- [ ] Remove the scheduled-task installer, `engine_ctl`, and `register-cleanup-task.ps1`.
- [ ] Integration tests cover standard-user access, unauthorized clients, malformed frames, and service restart, in isolated Windows environments only.

## Remaining gaps and unknowns

- The service host depends on the cross-language protocol and final threat boundary described in [IPC contracts](ipc-contracts.md).
- The exact identifiers for Windows Update and Delivery Optimization cleanup are not yet specified.
- Whether per-session revert should also run when the user signs out while Runner keeps running is undecided.
