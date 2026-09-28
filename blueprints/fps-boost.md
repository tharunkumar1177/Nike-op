# FPS Boost

## Outcome or responsibility

While a profile is active, the player can apply a set of temporary, reversible system adjustments aimed at game performance. Each adjustment reports its own result. Every adjustment is undone when the profile is deactivated, when Runner exits, or on recovery after a crash or reboot.

## Current verified status

**Status:** Planned

Code inspection on 2026-09-28 found no code that changes process priority, service state, power configuration, or memory lists. `crates/core/src/profile.rs` has no field that identifies a profile's game executable or FPS Boost selections. Process termination, which is a separate irreversible action, is owned by [System Tweaks](system-tweaks.md).

## Architecture dependencies

- [Reversible system changes](../architecture.md#reversible-system-changes)
- [Privilege and identity](../architecture.md#privilege-and-identity)
- [Process safety](../architecture.md#process-safety)
- [Distribution editions and install layout](../architecture.md#distribution-editions-and-install-layout)
- [State ownership and persistence](../architecture.md#state-ownership-and-persistence)
- [Failure and recovery](../architecture.md#failure-and-recovery)

## Local rules and implications

### Adjustments and execution context

| Adjustment | Context | Store edition | Full edition |
|---|---|---|---|
| Raise the profile's game process to above-normal priority | Interactive-user | Available | Available |
| Lower priority of the user's own selected background processes | Interactive-user | Available | Available |
| Switch to a high-performance power plan | Interactive-user | Available | Available |
| Lower priority of selected processes outside the user's session | Machine-level | Unavailable | Available |
| Pause SysMain and Windows Search for the session | Machine-level | Unavailable | Available |
| Disable CPU core parking through a dedicated power plan | Machine-level | Unavailable | Available |
| Purge the standby memory list (RAM cleaner) | Machine-level | Unavailable | Available |
| Open Windows Security for Defender guidance | Presentation-only | Available | Available |

Machine-level adjustments reach the Privileged Broker only as allowlisted operation identifiers. Service pausing is limited to the fixed service identifiers in this table. Unavailable adjustments are shown with their reason and cannot be selected.

### Reversible system changes

Every adjustment except the standby-list purge is journaled before it is applied and reverted as the shared contract requires:

- The power plan is restored to the plan active before activation.
- A dedicated core-parking plan created by the broker is deleted on revert.
- Paused services are started again.
- Processes that still exist get their prior priority back; processes that have exited are recorded as "no longer running".

The standby-list purge is a one-time action with no persistent state to revert.

### Privilege and identity

Microsoft Defender is never disabled, reconfigured, or given exclusions. The only Defender-related feature is a presentation-only link to Windows Security with guidance.

### Process safety

The game target and every process chosen for priority lowering go through the protected and critical-process checks. Audio, input, display, and Windows-critical processes are never lowered. Before changing a priority, Runner or the broker re-resolves the PID and confirms the image identity.

### State ownership and persistence

A profile's game executable and its FPS Boost selections are profile-scoped Runner state. Restoring the active profile at startup does not re-apply adjustments; it only reverts journal entries left behind.

### Failure and recovery

Each adjustment reports applied, skipped (with a reason), unavailable, failed, or reverted. A failed revert stays visible until it is resolved or the user dismisses it. Activation is never reported as fully successful when an adjustment failed.

## Related blueprints

### Required

- [Privileged broker](privileged-broker.md) — executes and journals the machine-level adjustments.
- [Process safety](process-safety.md) — protected and critical-process rules and PID validation for priority changes.
- [Profile persistence](profile-persistence.md) — owns the profile schema change for the game executable and selections.

### Impact checks

- [System Tweaks](system-tweaks.md) — check when activation ordering, results, or process selection is shared.
- [Settings client](settings-client.md) — check when FPS Boost controls, availability reasons, or result presentation change.
- [IPC contracts](ipc-contracts.md) — check when adding activation-plan or adjustment-result messages.

## Relevant implementation and tests

- `crates/core/src/profile.rs` — profile schema that must gain the game executable and FPS Boost selections.
- `crates/core/src/process.rs` — protected-name normalization reused for priority targets.
- `crates/runner/src/main.rs` — profile activation and deactivation orchestration.

## Acceptance or verification criteria

- [ ] Store the game executable and FPS Boost selections per profile through a transactional migration.
- [ ] Apply interactive-user adjustments in Runner and machine-level adjustments only through allowlisted broker operations.
- [ ] Journal every reversible adjustment before applying it, and revert on deactivation, Runner exit, and post-crash or post-reboot start.
- [ ] Never change a service's startup type, and never touch Defender or other security products.
- [ ] Reject protected, critical, audio, input, and display processes for priority lowering, with structured reasons.
- [ ] Report per-adjustment applied, skipped, unavailable, failed, and reverted outcomes.
- [ ] Show machine-level adjustments as unavailable in the Store edition.
- [ ] Unit-test apply and revert planning, journal recovery, and target validation with fake system interfaces.
- [ ] Verify real service, power, priority, and memory changes only in isolated Windows integration environments.

## Remaining gaps and unknowns

- The measurable performance benefit of each adjustment on current Windows builds has not been evaluated. The Store listing must not claim specific FPS gains.
- Whether the power-plan switch needs a user-selectable plan, or always uses the built-in high-performance plan, is undecided.
- The exact list of never-lower processes beyond the existing protected set must be defined and tested.
- Game detection is limited to a user-selected executable; automatic detection is out of scope.
