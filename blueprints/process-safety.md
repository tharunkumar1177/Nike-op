# Process termination safety

## Outcome or responsibility

Critical Windows processes cannot be selected, terminated, or lowered in priority through alternate spelling, casing, or extension forms, or through a PID that no longer refers to the validated process.

## Current verified status

**Status:** Partial

Protected-name normalization and regression tests cover extensionless, mixed-case, and whitespace forms. PID-level critical and protected-process validation is not implemented, and no code changes process priority yet.

## Architecture dependencies

- [Process safety](../architecture.md#process-safety)
- [Privilege and identity](../architecture.md#privilege-and-identity)

## Local rules and implications

### Process safety

UI validation is advisory. Whichever component performs a termination or priority change must repeat validation against the resolved PID immediately before the operation. That component is Runner for user-owned processes and the broker for all others. Priority lowering additionally rejects the audio, input, and display processes that [FPS Boost](fps-boost.md) lists.

### Privilege and identity

Runner can end or re-prioritize only processes the interactive user may already control. Anything else goes to the broker as a validated target, never as a raw name or command.

## Related blueprints

### Required

None.

### Impact checks

- [Privileged broker](privileged-broker.md) — check when final authorization, termination, or priority validation changes.
- [System Tweaks](system-tweaks.md) — check when process selection or activation results change.
- [FPS Boost](fps-boost.md) — check when priority-lowering targets or never-lower rules change.

## Relevant implementation and tests

- `crates/core/src/process.rs` — current name matching, process enumeration, termination, and unit tests.

## Acceptance or verification criteria

- [x] Block `.exe` and extensionless forms case-insensitively.
- [x] Ignore surrounding whitespace for safety comparison.
- [ ] Resolve a requested target to a PID and re-check Windows critical and protected state.
- [ ] Apply the same validation to priority-lowering operations.
- [ ] Return a structured reason for every skipped PID.

## Remaining gaps and unknowns

Current termination is still name-based and executes in EngineSvc. Final PID validation belongs to the Runner and broker execution paths. The never-lower process list has not been defined.
