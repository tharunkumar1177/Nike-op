# Process termination safety

## Outcome

Critical Windows processes cannot be selected or terminated through alternate spelling, casing, or extension forms.

## Current verified status

**Status:** Partial

Protected-name normalization and regression tests now cover extensionless, mixed-case, and whitespace forms. PID-level critical/protected-process validation is not implemented.

## Architecture dependencies

- [Process safety](../architecture.md#process-safety)
- [Privilege and identity](../architecture.md#privilege-and-identity)

## Feature-specific implications

UI validation is advisory. The final privileged execution boundary must repeat validation against the resolved PID immediately before termination.

## Related blueprints

### Required

None.

### Impact checks

- [Privileged broker](privileged-broker.md) — owns final authorization and termination.
- [System Tweaks](system-tweaks.md) — process selection and activation results must preserve safety decisions.

## Relevant implementation and tests

- `crates/core/src/process.rs` — current name matching, process enumeration, termination, and unit tests.

## Acceptance criteria

- [x] Block `.exe` and extensionless forms case-insensitively.
- [x] Ignore surrounding whitespace for safety comparison.
- [ ] Resolve a requested target to PID and re-check Windows critical/protected state.
- [ ] Return a structured reason for every skipped PID.

## Remaining gaps

Current termination remains name-based and executes in EngineSvc. Final PID validation belongs in the privileged broker iteration.
