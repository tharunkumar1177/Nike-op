# Privileged broker and cleanup

## Outcome or responsibility

In the Full edition, a least-privilege Windows service (EngineSvc) performs a small allowlist of named operations for two authenticated clients, the installed Runner and the installed Settings, and journals every reversible change. It is the only component that terminates processes, and it will own scheduled cleanup. The Store edition ships no broker.

## Current verified status

**Status:** Partial

Code inspection on 2026-09-28. Unit tests were written, but CI has not run them yet, and nothing was exercised under a real service or pipe.

- `crates/engine_service/src/main.rs` still runs as a console process with no Service Control Manager integration. `scripts/install-engine-service.ps1` registers it as a SYSTEM scheduled task.
- **Pipe.** The pipe uses `ENGINE_PIPE_SDDL`: SYSTEM owner, network logons denied, SYSTEM and Administrators full access, interactive users read/write. It also sets `FILE_FLAG_FIRST_PIPE_INSTANCE` and `PIPE_REJECT_REMOTE_CLIENTS`, and a single instance is held for the service lifetime.
- **Client checks.** The Rust client (Runner) and the C# client (Settings) both refuse the pipe unless SYSTEM owns it.
- **Server checks.** For each connection, EngineSvc reads the client's PID and session from the pipe. It requires the client image to be `EdgeOptimizer_Runner.exe` or `EdgeOptimizer.Settings.WinUI.exe` in its own install directory (`install_layout::is_sibling_image`), and disconnects anyone else.
- **Operations.** The allowlist is `TerminateTargets` (PID plus creation time plus image name, 1 to 128 targets), `Ping`, and `GetCapabilities`. Dispatch checks the protocol version and bounds, rejects duplicate PIDs, the client's own PID, and reserved PIDs, and passes the pipe-derived session, not a message claim, to termination.
- **Malformed frames.** A malformed or oversized frame now drops that connection instead of ending the service.
- `AuthContext` values are claims only.
- Recycle Bin and browser-cache cleanup currently run in Runner (`crates/core/src/user_cleanup.rs`). Moving scheduled cleanup to EngineSvc is planned.

## Architecture dependencies

- [Privilege and identity](../architecture.md#privilege-and-identity)
- [Reversible system changes](../architecture.md#reversible-system-changes)
- [Distribution editions and install layout](../architecture.md#distribution-editions-and-install-layout)
- [Process safety](../architecture.md#process-safety)
- [Failure and recovery](../architecture.md#failure-and-recovery)

## Local rules and implications

### Privilege and identity

The broker runs as a Windows service installed only by the Full edition installer.

**Pipe security.** The pipe's security descriptor grants connect access to interactive users and denies network logons. SYSTEM owns the pipe, and clients refuse it otherwise. For each connection, the broker:

- reads the client process ID and session from the pipe;
- confirms the client image path is the installed Runner or Settings beside the broker; and
- records the client's token SID (planned).

Each request is then authorized against the operation allowlist. The broker never trusts `AuthContext` or other serialized identity.

**Allowlist:**

- termination of validated targets in the client's session, for both Runner and Settings (implemented);
- scheduled cleanup, both user-specific categories (resolved for the requesting user's token, never from SYSTEM's environment) and machine-level categories from [Disk cleanup](disk-cleanup.md) (planned);
- session-only pause and resume of the FPS Boost service identifiers;
- creation, activation, and removal of the dedicated core-parking power plan;
- priority changes for validated processes outside the caller's session; and
- a standby memory list purge.

On-demand Recycle Bin and browser-cache cleanup run in Runner until scheduled cleanup moves here.

### Reversible system changes

The broker keeps its machine journal under `%ProgramData%`, with an ACL that allows access only to SYSTEM and Administrators. On service start it reverts outstanding entries before accepting new requests. On controlled shutdown it reverts entries for sessions that have ended.

### Distribution editions and install layout

The WiX installer registers the service, its recovery actions, and its security descriptor, and removes all three on uninstall. The scheduled-task installer is retired once the service host exists.

### Process safety

Before termination or a priority change, the broker is the final boundary: it repeats name normalization, opens the PID, re-checks creation time, image name, session, and critical status through that handle, and terminates through the same handle (`process::terminate_target`).

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

- `crates/engine_service/src/main.rs` — transitional privileged worker with client authentication.
- `crates/core/src/engine_ipc.rs` — transitional pipe, client identity, and SYSTEM-owner check.
- `crates/core/src/pipe_security.rs` — `ENGINE_PIPE_SDDL`, pipe client identity, and owner lookup.
- `crates/core/src/install_layout.rs` — `is_sibling_image` client-image rule, with unit tests.
- `crates/core/src/engine_commands.rs` — injectable command dispatch with fake-operation tests.
- `apps/EdgeOptimizer.Settings.WinUI/Services/EngineServiceClient.cs` — Settings client and SYSTEM-owner check.
- `scripts/install-engine-service.ps1` — scheduled-task installer to replace.
- `scripts/uninstall-engine-service.ps1` — scheduled-task removal to replace.

## Acceptance or verification criteria

- [ ] Install and run under the Windows Service Control Manager from the Full edition installer.
- [x] Apply an explicit, SYSTEM-owned pipe ACL and verify that the client image is the installed Runner or Settings (code and unit tests; Windows evidence pending).
- [ ] Record and authorize the connecting token SID per operation.
- [x] Enforce the named-operation allowlist with bounds, version, and per-target validation; reject everything else.
- [x] Terminate only PID-validated, same-session targets for both clients.
- [x] Move on-demand user-specific cleanup to Runner in the interactive-user context and remove it from the engine protocol.
- [ ] Run scheduled cleanup in the broker, resolving user categories for the requesting user's token.
- [ ] Journal machine-level reversible changes and revert outstanding entries on service start.
- [ ] Support service recovery, controlled shutdown, and Event Log operational logging.
- [x] Remove `engine_ctl` and `register-cleanup-task.ps1`.
- [ ] Remove the scheduled-task installer once the service host exists.
- [ ] Integration tests cover standard-user access, unauthorized clients, malformed frames, and service restart, in isolated Windows environments only.

## Remaining gaps and unknowns

- The service host depends on the cross-language protocol and final threat boundary described in [IPC contracts](ipc-contracts.md).
- Client-image trust is only as strong as the install directory's ACL. The unpackaged CI bundle is user-writable, so an attacker who can replace `EdgeOptimizer_Runner.exe` there is trusted. The Full edition's read-only, per-machine install directory is required.
- Standard-user clients check the pipe owner without opening the SYSTEM process. Whether any Windows configuration assigns the pipe a different owner is unverified.
- How scheduled cleanup obtains the user's token when no client is connected is undecided.
- The exact identifiers for Windows Update and Delivery Optimization cleanup are not yet specified.
- Whether per-session revert should also run when the user signs out while Runner keeps running is undecided.
