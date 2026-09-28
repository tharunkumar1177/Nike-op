# System tray and quick flyout

## Outcome or responsibility

Runner owns the system-tray icon and a lightweight native quick flyout in the
same process. Opening the flyout does not start or load the WinUI Settings
process. The full Settings client starts only from a tray double-click or when
the user explicitly chooses **Open Settings**.

## Current verified status

**Status:** Partial

Code inspection on 2026-09-28. Unit tests for the model were written, but CI has not run them yet. No interactive Windows run has happened.

Runner owns the tray icon, context menu, and a native quick flyout, `crates/runner/src/flyout`, drawn with Direct2D and DirectWrite:

- A tray single-click toggles the flyout after the double-click window. It sends no Settings IPC and does not start Settings. `--flyout-only` is no longer passed.
- The flyout shows the profiles, the active profile, and the active profile's running apps. Its actions are activate, deactivate, **End** (one app), **End all**, **Open Settings**, and **Exit**.
- The window is a per-monitor-DPI-aware tool window. It is placed against the taskbar edge in the work area and dismissed by Escape, by losing activation, or by selecting activate, deactivate, open, or exit. It has Tab and arrow-key focus with a visible focus ring, and it follows the high-contrast and light/dark taskbar themes.
- Runner's loop now waits with `MsgWaitForMultipleObjects`, so flyout input is handled immediately. EngineSvc calls run on a worker thread, so the tray and flyout never block on EngineSvc.
- Stale clicks are re-validated against current state before they run.
- If the flyout cannot be created, tray clicks open Settings instead.

The legacy GDI+ `crates/core/src/flyout.rs` has been deleted. Screen-reader support (a UI Automation provider) is not implemented.

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
- The flyout scope is profile selection, active/inactive status,
  activate/deactivate, ending the active profile's running apps, **Open
  Settings**, and **Exit Runner**. Editing profiles, crosshairs, macros, and
  System Tweaks remains in WinUI Settings.
- Ending apps sends PID-plus-creation-time targets to EngineSvc; the flyout
  never terminates processes itself. While visible, it pins the listed
  processes with handles, checks them every second, and re-enumerates every
  three seconds. It releases every handle when hidden.
- Activation from the flyout persists the active profile, starts workers,
  notifies a connected Settings client, and asks EngineSvc to close the
  profile's running apps.
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
- Clicking outside, pressing Escape, selecting activate, deactivate, **Open
  Settings**, or **Exit**, or receiving an explicit close request dismisses the
  flyout. **End** and **End all** keep it open so the result is visible.
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

- `crates/core/src/tray_icon.rs` — tray icon and context menu used by Runner.
- `crates/runner/src/flyout/model.rs` — flyout projection, layout, hit
  testing, keyboard focus, stale-action validation, and taskbar-edge placement,
  with unit tests.
- `crates/runner/src/flyout/render.rs` — Direct2D/DirectWrite drawing and
  theme and high-contrast palettes.
- `crates/runner/src/flyout/window.rs` — HWND, DPI, input, dismissal, and
  timer handling.
- `crates/runner/src/tracker.rs` — handle-based tracking of the active
  profile's processes while the flyout is visible.
- `crates/runner/src/main.rs` — tray loop, flyout actions, and Settings
  launch/`BringMainToFront`.
- `apps/EdgeOptimizer.Settings.WinUI/App.xaml.cs` — still maps the deprecated
  flyout commands to the full Settings window.

## Acceptance or verification criteria

- [ ] A tray primary-click shows the quick flyout while
  `EdgeOptimizer.Settings.WinUI.exe` is not running (implemented; Windows
  evidence pending).
- [x] A tray single-click never sends Settings IPC or initializes WinUI (code
  path).
- [x] The flyout renders with Direct2D and DirectWrite, not GDI or GDI+.
- [ ] Ending an app from the flyout goes to EngineSvc and shows its per-target
  result (implemented; Windows evidence pending).
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
- [x] Unit tests cover flyout state projection, layout, focus order, stale-action
  validation, and placement without creating a real window. Toggling and
  action routing in `main.rs` are not unit-tested.
- [ ] CI verifies the Runner build and logic tests; interactive Windows tests
  provide separate evidence for HWND, focus, placement, and accessibility.
- [ ] Transitional `--flyout-only`, `ShowFlyout`, and `HideFlyout` paths are
  removed after the Runner-owned flyout is verified.

## Remaining gaps and unknowns

- The flyout has no UI Automation provider, so screen readers get only the
  window name. Adding one is required before the accessibility criterion can
  be met.
- The flyout does not scroll. It shows up to six profiles and six apps, with
  a count of the rest.
- Runtime memory, launch latency, multi-monitor positioning, and rendering on
  real hardware have not been measured.
- `ShowFlyout`/`HideFlyout` remain in the Bincode enum only to keep variant
  tags stable; Runner no longer sends them.
