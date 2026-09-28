# Privileged broker and cleanup

## Outcome

A least-privilege Windows service performs a small allowlist of machine-level operations for an authenticated Runner, while user cleanup remains in the interactive user session.

## Current verified status

**Status:** Planned

The current EngineSvc is installed as a SYSTEM scheduled task, uses default named-pipe security, and computes browser paths from the SYSTEM environment.

## Architecture dependencies

- [Privilege and identity](../architecture.md#privilege-and-identity)
- [Failure and recovery](../architecture.md#failure-and-recovery)

## Feature-specific implications

The migration must preserve Runner-facing results while replacing the host with an SCM service. Browser cache and per-user recycle-bin cleanup move to Runner or use verified impersonation only when required.

## Related blueprints

### Required

- [IPC contracts](ipc-contracts.md) — broker transport and identity.
- [Process safety](process-safety.md) — final termination validation.

### Impact checks

- [System Tweaks](system-tweaks.md) — operation classification determines which requests may reach the broker.

## Relevant implementation and tests

- `crates/engine_service/src/main.rs` — transitional privileged worker.
- `crates/core/src/engine_ipc.rs` — transitional pipe.
- `scripts/install-engine-service.ps1` — scheduled-task installer to replace.

## Acceptance criteria

- [ ] Install and run under Windows Service Control Manager.
- [ ] Apply an explicit pipe ACL and verify the connecting user/token.
- [ ] Enforce a minimal operation allowlist and per-operation authorization.
- [ ] Run user-specific cleanup in the verified interactive-user context.
- [ ] Support service recovery, controlled shutdown, and operational logging.

## Remaining gaps

This iteration is intentionally deferred until the cross-language protocol and threat boundary are finalized.
