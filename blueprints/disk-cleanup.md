# Disk cleanup and scheduled maintenance

## Outcome or responsibility

A user chooses cleanup categories, previews how much space each would free, runs cleanup immediately, or sets a weekly schedule. They then see a per-category history of what was removed, skipped, or failed. Cleanup is an app-wide setting on its own Settings page, not part of a gaming profile.

## Current verified status

**Status:** Planned

Code inspection on 2026-09-28:

- `crates/engine_service/src/main.rs` implements two transitional cleanup kinds inside the SYSTEM-hosted EngineSvc:
  - Recycle Bin, by running `Clear-RecycleBin` through PowerShell.
  - Browser cache, by recreating Chrome and Edge `Default`-profile cache folders under the service's own `%LOCALAPPDATA%`.
- Both therefore act on the SYSTEM profile rather than the signed-in user's.
- `scripts/register-cleanup-task.ps1` schedules `EdgeOptimizer_EngineCtl.exe cleanup <kind>` as a daily SYSTEM task. `crates/engine_ctl/src/main.rs` forwards that command to the engine pipe with an `AuthContext::ScheduledTask` claim.
- The WinUI shell (`apps/EdgeOptimizer.Settings.WinUI/ShellPage.xaml`) has no Cleanup page.
- `SystemTweaksViewModel` holds per-profile Recycle Bin and browser-cache toggles and run commands. Those toggles do not exist in the Rust profile contract, and [System Tweaks](system-tweaks.md) keeps the controls disabled.

No schedule is stored, and no size preview or cleanup history exists.

## Architecture dependencies

- [Privilege and identity](../architecture.md#privilege-and-identity)
- [State ownership and persistence](../architecture.md#state-ownership-and-persistence)
- [Reversible system changes](../architecture.md#reversible-system-changes)
- [Distribution editions and install layout](../architecture.md#distribution-editions-and-install-layout)
- [IPC and protocol boundary](../architecture.md#ipc-and-protocol-boundary)
- [Failure and recovery](../architecture.md#failure-and-recovery)

## Local rules and implications

### Categories and execution context

| Category | Context | Locations (resolved for the signed-in user) |
|---|---|---|
| Browser caches | Interactive-user | Cache and code-cache folders for every profile of Chrome, Edge, Brave, Opera GX, and Firefox |
| User temp | Interactive-user | The user's `%TEMP%` |
| Recycle Bin | Interactive-user | The signed-in user's Recycle Bin, through the shell API |
| Thumbnail and icon cache | Interactive-user | `thumbcache_*.db` and `iconcache_*.db` under the user's Explorer cache folder |
| GPU shader caches | Interactive-user | DirectX shader cache and the NVIDIA and AMD per-user shader cache folders |
| Windows temp | Machine-level | `%SystemRoot%\Temp` |
| Windows Update download cache | Machine-level | The Windows Update download folder |
| Delivery Optimization cache | Machine-level | Cleared through the Delivery Optimization API, not by deleting files |

Each category is a named enumeration value, and its locations are resolved by the executing component. Neither Settings nor any IPC message carries a filesystem path.

### Privilege and identity

Runner executes interactive-user categories itself. Machine-level categories are sent to the Privileged Broker only as allowlisted operation identifiers. In the Store edition they are reported as unavailable with the reason "requires Full edition" and cannot be selected.

### Safety rules

- Delete only files inside a resolved category root. Never follow symbolic links or junctions out of that root.
- Skip files that are locked or in use, and count them as skipped.
- Skip a browser's category while that browser is running, and report the reason.
- Never touch Prefetch, user documents, downloads, or browser profile data other than caches.
- Recreate a category root if a browser or Windows expects it to exist.

### State ownership and persistence

Runner stores the selected categories, the weekly schedule (one or more weekdays and a local time), the last-run marker, and a bounded run history in `state.db`, through a `PRAGMA user_version` migration. Settings edits them only through Runner.

### Scheduling

- Runner is the only scheduler; no Windows scheduled task is created.
- Occurrences use local wall-clock time. A time that does not exist on a daylight-saving transition day runs at the next valid minute, and a repeated time runs once.
- If one or more occurrences were missed while the user was signed out or the machine was off, Runner runs cleanup once after the next sign-in, after a short delay, and shows a notification. Missed occurrences never stack.
- Scheduled and immediate runs use the same validation, classification, and per-category results.

### Reversible system changes

Cleanup deletes files and is irreversible. Each run therefore needs explicit intent: either **Run now** or a schedule the user created. The page offers a size preview before either one.

### IPC and protocol boundary

The Settings/Runner contract carries:

- cleanup settings reads and saves;
- preview requests, with per-category estimated bytes and item counts;
- run requests;
- progress events; and
- structured per-category results (deleted bytes and items, skipped items with reasons, failures, or "unavailable in this edition").

### Failure and recovery

A failure in one category never stops the others and is never reported as overall success. A failed scheduled run is recorded in history and is not retried with broader privilege.

## Related blueprints

### Required

- [Privileged broker](privileged-broker.md) — executes the machine-level categories.
- [Profile persistence](profile-persistence.md) — owns `state.db` migrations that the schedule and history tables must follow.
- [IPC contracts](ipc-contracts.md) — carries settings, preview, run, progress, and result messages.

### Impact checks

- [Settings client](settings-client.md) — check when Cleanup page states, availability reasons, or notifications change.
- [System Tweaks](system-tweaks.md) — check when removing its transitional per-profile cleanup toggles.
- [Packaging and distribution](packaging-and-distribution.md) — check when edition availability of a category changes.

## Relevant implementation and tests

- `crates/engine_service/src/main.rs` — transitional SYSTEM-context Recycle Bin and browser-cache cleanup, to be replaced.
- `crates/core/src/orchestration.rs` — transitional `CleanupKind` and result types.
- `crates/core/src/engine_commands.rs` — injectable cleanup routing with fake-operation tests.
- `crates/engine_ctl/src/main.rs` and `scripts/register-cleanup-task.ps1` — transitional scheduled cleanup path, to be removed.
- `apps/EdgeOptimizer.Settings.Core/ViewModels/SystemTweaksViewModel.cs` — transitional per-profile cleanup toggles.

## Acceptance or verification criteria

- [ ] Resolve every interactive-user category for the signed-in user inside Runner; never from a SYSTEM environment.
- [ ] Send machine-level categories to the broker only as allowlisted identifiers, and report them as unavailable in the Store edition.
- [ ] Preview estimated bytes and item counts per category without deleting anything.
- [ ] Confine deletion to category roots, reject links and junctions that escape them, and skip locked files and running browsers with structured reasons.
- [ ] Persist categories, the weekly schedule, and bounded history through a transactional `state.db` migration.
- [ ] Run a due occurrence once and a missed occurrence once after sign-in, with a notification.
- [ ] Return per-category deleted, skipped, failed, and unavailable results for both immediate and scheduled runs.
- [ ] Provide a WinUI Cleanup page with category selection, weekday and time schedule, preview, **Run now**, and history.
- [ ] Remove `engine_ctl`, `register-cleanup-task.ps1`, and the transitional per-profile cleanup toggles.
- [ ] Unit-test path resolution, root confinement, schedule calculation (including daylight-saving transitions and missed runs), and result aggregation with fake filesystems and clocks.
- [ ] Verify real deletion only in isolated Windows integration environments, never on hosted CI or a developer machine.

## Remaining gaps and unknowns

- The exact per-vendor shader cache folders and per-browser cache folder names must be confirmed against current browser and driver releases before implementation.
- Thumbnail and icon caches are usually held open by Explorer, so most runs will report them as skipped. Whether to offer an Explorer restart is undecided.
- Whether Windows Update cache cleanup should wait while an update download is in progress needs evaluation.
