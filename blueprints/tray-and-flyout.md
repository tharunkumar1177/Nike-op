# System tray and quick flyout

## Outcome or responsibility

Runner owns the system-tray icon and a lightweight native quick flyout in the
same process. Opening the flyout does not start or load the WinUI Settings
process. The full Settings client starts only from a tray double-click or when
the user explicitly chooses **Open Settings**.

## Current verified status

**Status:** Partial

Runner currently owns the tray icon and its context menu. On a tray single
click, however, Runner sends `ShowFlyout` to an existing Settings client or
starts `EdgeOptimizer.Settings.WinUI.exe --flyout-only`. The WinUI client does
not implement a separate flyout mode and activates its main window instead.

The Runner-owned flyout described here is the approved target design and is
not yet implemented. Legacy `FlyoutWindow` and `TrayFlyoutManager` code exists
under `crates/core`, but it is not the active Runner tray path and must not be
treated as verification of this blueprint.

## Architecture dependencies

- [Component boundaries](../architecture.md#component-boundaries) — Runner is
  the per-user tray, orchestration, process, and durable-state authority.
- [State ownership and persistence](../architecture.md#state-ownership-and-persistence)
  — flyout state is a projection of Runner's current in-memory state and never
  a second durable store.
- [Failure and recovery](../architecture.md#failure-and-recovery) — a flyout
  rendering failure must not stop Runner or trigger a privileged fallback.

## Local rules and implications

### Ownership and lifetime

- Runner owns the tray icon, flyout HWND, flyout event loop, positioning,
  visibility, dismissal, and cleanup.
- The flyout runs unprivileged in Runner's interactive-user session.
- Showing or hiding the flyout must not launch, connect to, or initialize the
  WinUI Settings process.
- The flyout is created lazily or retained as a lightweight Runner-owned
  window. Either implementation must keep one logical flyout instance and
  prevent duplicate windows.
- Closing the WinUI Settings client has no effect on tray or flyout lifetime.
  Exiting Runner removes both tray and flyout.

### State and actions

- The flyout reads profile summaries and active-profile state from Runner's
  authoritative in-memory snapshot; it does not read `state.db` independently.
- Profile activation and deactivation use the same validated Runner
  orchestration paths used by other clients. The flyout must not bypass worker,
  persistence, process-safety, or result handling.
- The initial flyout scope is profile selection, active/inactive status,
  activate/deactivate, **Open Settings**, and **Exit Runner**. Editing profiles,
  crosshairs, macros, and System Tweaks remains in WinUI Settings.
- **Open Settings** is the only action inside the flyout that launches or
  foregrounds `EdgeOptimizer.Settings.WinUI.exe`; tray double-click and the
  context-menu **Open Settings** command provide the same full-window action.
- Runner refreshes a visible flyout after profile or engine state changes. A
  stale click is resolved against current Runner state before execution.

### Interaction behavior

- A single left-click toggles Runner's flyout near the taskbar work area. This
  path stays entirely inside Runner: it uses neither Settings IPC nor a WinUI
  `DispatcherQueue`.
- A double left-click cancels the pending single-click action and opens the
  full Settings window. Runner starts `EdgeOptimizer.Settings.WinUI.exe` when
  Settings is not running. Starting a new process does not require IPC.
- When Settings is already connected, a double left-click sends a named-pipe
  `BringMainToFront` command instead of starting another instance. Settings
  receives that command off the UI thread and uses its WinUI
  `DispatcherQueue` to activate the existing window safely.
- Right-click shows Runner's context menu with **Open Settings**,
  **Documentation**, **Report Bug**, and **Exit**.
- **Open Settings** follows the same launch-or-foreground behavior as a tray
  double-click.
- Clicking outside, pressing Escape, selecting an action, or receiving an
  explicit close request dismisses the flyout.
- The existing right-click context menu remains available independently of the
  quick flyout.
- Flyout work must not block Runner's message pump, worker supervision, IPC, or
  shutdown path.
- The window supports keyboard navigation, visible focus, accessible control
  names, high-contrast/theme resources, DPI scaling, and screen-edge-aware
  placement.

### Migration boundary

- Retire the `--flyout-only` Settings launch path after the Runner-owned flyout
  is verified.
- Retire Settings IPC commands whose only purpose is showing or hiding the
  quick flyout. Commands for opening or foregrounding the full Settings window
  remain distinct.
- Keep `BringMainToFront` as a Settings-window command. Its receiver must
  dispatch window activation through WinUI's `DispatcherQueue`; it is not a
  flyout command and Runner does not use a `DispatcherQueue`.
- Reuse or replace legacy native flyout code only after validating its thread
  ownership, DPI behavior, accessibility, resource lifetime, and compatibility
  with Runner's active tray loop.

## Related blueprints

### Required

- [Profile persistence](profile-persistence.md) — the flyout displays and
  activates Runner-owned profiles without becoming a persistence owner.

### Impact checks

- [Settings client](settings-client.md) — check when launch, foregrounding, or
  window-command behavior changes.
- [IPC contracts](ipc-contracts.md) — check when removing transitional
  `ShowFlyout`/`HideFlyout` messages or separating full-window commands.
- [Crosshair overlay](crosshair-overlay.md) — check when activation from the
  flyout changes worker orchestration.
- [Macro automation](macro-automation.md) — check when activation from the
  flyout changes worker orchestration.

## Relevant implementation and tests

- `crates/core/src/tray_icon.rs` — active lightweight tray icon and context
  menu used by Runner; currently documents that it does not own a flyout.
- `crates/runner/src/main.rs` — active tray event loop and current Settings
  launch/`ShowFlyout` behavior.
- `crates/core/src/flyout.rs` — legacy native flyout implementation; candidate
  reference only, not active evidence.
- `crates/core/src/tray_flyout.rs` — legacy combined tray/flyout manager; not
  used by the active Runner path.
- `apps/EdgeOptimizer.Settings.WinUI/App.xaml.cs` — currently maps the
  transitional flyout command to the full Settings window.

## Acceptance or verification criteria

- [ ] A tray primary-click shows the quick flyout while
  `EdgeOptimizer.Settings.WinUI.exe` is not running.
- [ ] A tray single-click never sends Settings IPC or initializes WinUI.
- [ ] A tray double-click cancels pending single-click flyout activation and
  opens the full Settings window.
- [ ] Double-click launches one Settings process when disconnected and sends
  `BringMainToFront` over the named pipe when Settings is already connected.
- [ ] Settings marshals `BringMainToFront` to its UI thread with
  `DispatcherQueue` before activating its existing window.
- [ ] Right-click exposes **Open Settings**, **Documentation**,
  **Report Bug**, and **Exit** without opening the quick flyout.
- [ ] Repeated tray activation toggles one flyout instance without duplicating
  windows or handlers.
- [ ] The flyout shows current profile summaries and active state from Runner.
- [ ] Activate/deactivate actions use Runner's normal validated orchestration
  and surface success or failure.
- [ ] **Open Settings** launches or foregrounds the full WinUI client; no other
  action inside the quick flyout loads it.
- [ ] Outside click, Escape, action completion, and Runner shutdown dismiss and
  release the flyout safely.
- [ ] Flyout failure leaves Runner, tray, workers, and active optimization state
  operational.
- [ ] Keyboard-only, screen-reader-name, high-contrast, DPI, multi-monitor, and
  taskbar-edge behavior are covered by Windows integration tests.
- [ ] Unit tests cover flyout state projection, stale-action validation,
  toggling, and action routing without creating a real window.
- [ ] CI verifies the Runner build and logic tests; interactive Windows tests
  provide separate evidence for HWND, focus, placement, and accessibility.
- [ ] Transitional `--flyout-only`, `ShowFlyout`, and `HideFlyout` paths are
  removed after the Runner-owned flyout is verified.

## Remaining gaps and unknowns

- The Runner-owned flyout UI and state adapter are not implemented.
- The native UI technology should be selected during implementation after
  evaluating the existing GDI+ flyout for DPI, accessibility, and maintenance;
  ownership is fixed by this blueprint, but the rendering library is not.
- Runtime memory, launch latency, multi-monitor positioning, and accessibility
  have not been measured.
- The current transitional IPC and `--flyout-only` behavior still loads the
  complete WinUI Settings process.
