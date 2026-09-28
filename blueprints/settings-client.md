# Settings client

## Outcome or responsibility

An on-demand, unprivileged WinUI client edits profiles and presents Runner
state while consuming no memory when closed. It owns only the full Settings
experience; Runner owns the system tray and quick flyout.

## Current verified status

**Status:** Partial

WinUI 3 is the active Settings UI and is launched on demand by Runner from the packaged `EdgeOptimizer.Settings.WinUI.exe`. It restores the full profile-scoped Dashboard, Crosshair, Macros, and System Tweaks surfaces and uses a transitional Bincode compatibility client to hydrate Runner state, save profile collections, request cleanup, and activate profiles. It enumerates processes locally for System Tweaks, idling when that page is hidden or the window is inactive. On activation it sends the selected apps' running instances directly to EngineSvc. Runner's pipe accepts the client non-blockingly. A Windows CI job tests UI-independent logic, compiles WinUI XAML, and publishes a self-contained client artifact. Generated Protobuf bindings and runtime UI smoke automation remain planned.

Code inspection on 2026-09-28 (Core unit tests only; runtime UI behavior is not verified):

- The client shows no placeholder profiles, macros, or processes. A new `ProfileWorkspace` matches Runner's `create_profile` defaults, so nothing is selected for termination.
- Until Runner supplies a snapshot, the shell shows an offline, connecting, or no-profiles empty state, with **Retry** reconnecting the pipe.
- The sidebar marks the current page, and profiles can be duplicated or deleted; deletion asks for confirmation first.
- IPC send failures are reported in the status bar instead of escaping the command.
- Feature pages report "Not saved" when Runner does not accept a save.
- The Dashboard setup checklist is computed from profile state.
- The Macros editor checks Runner's name, action, and shortcut rules inline before saving.

A tray single-click now opens Runner's native flyout and no longer launches
Settings (see [system tray and quick flyout](tray-and-flyout.md)). A tray
double-click or an explicit **Open Settings** action starts or foregrounds this
client.

## Architecture dependencies

- [Component boundaries](../architecture.md#component-boundaries)
- [IPC and protocol boundary](../architecture.md#ipc-and-protocol-boundary)
- [Distribution editions and install layout](../architecture.md#distribution-editions-and-install-layout)

## Local rules and implications

WinUI 3 never owns durable state. Its only privileged request is EngineSvc's allowlisted `TerminateTargets`, sent after it verifies that SYSTEM owns the engine pipe. It requests a snapshot from Runner, submits validated commands, and exits completely when closed. Background work, currently the System Tweaks process fetcher, must stop when its page is hidden or the window is deactivated or minimized.

WinUI never infers the edition itself. It presents each capability as available or unavailable exactly as Runner reports it, shows the reason (for example "requires Full edition"), and disables the matching controls. Planned surfaces are an app-wide **Cleanup** page owned by [Disk cleanup](disk-cleanup.md) and profile-scoped FPS Boost controls owned by [FPS Boost](fps-boost.md).

For the Store edition the client is published framework-dependent on the Windows App SDK runtime package; the Full edition may stay self-contained (see [Packaging and distribution](packaging-and-distribution.md)). The legacy Iced settings executable and its `crates/core/src/gui` implementation have been deleted.

The client connects to Runner's per-session pipe, whose name comes from `RunnerPipeIdentity`. Before sending anything, it confirms through `GetNamedPipeServerProcessId` that the server is `EdgeOptimizer_Runner.exe` in its own install directory and session, and it disconnects otherwise.

Starting a new Settings process is a Runner process-launch operation and does
not require named-pipe IPC. Once Settings is connected, Runner uses the named
pipe for `BringMainToFront`; the Settings client marshals that command from its
IPC listener to the WinUI thread with `DispatcherQueue` before activating the
existing window. Runner never uses WinUI `DispatcherQueue`.

## Related blueprints

### Required

- [IPC contracts](ipc-contracts.md) — generated C# types are required before retiring the transitional compatibility transport.
- [System tray and quick flyout](tray-and-flyout.md) — defines the launch and
  window-ownership boundary between Runner and the full Settings client.

### Impact checks

- [Profile persistence](profile-persistence.md) — profile CRUD and startup hydration must remain Runner-owned.
- [Crosshair overlay](crosshair-overlay.md) — WinUI owns preview presentation but not overlay lifecycle or assets.
- [Macro automation](macro-automation.md) — WinUI owns editing presentation but not hooks or input execution.
- [System Tweaks](system-tweaks.md) — WinUI 3 presents choices without performing cleanup, termination, or machine changes.
- [Disk cleanup](disk-cleanup.md) — check when adding or changing the Cleanup page, schedule editor, preview, or history.
- [FPS Boost](fps-boost.md) — check when adding FPS Boost controls or result presentation.
- [Packaging and distribution](packaging-and-distribution.md) — check when publish mode, Windows App SDK deployment, or launch identity changes.

## Relevant implementation and tests

- `apps/EdgeOptimizer.Settings.Core` — UI-independent models, contracts, and view-model logic.
- `apps/EdgeOptimizer.Settings.WinUI` — active WinUI presentation client.
- `tests/EdgeOptimizer.Settings.Core.Tests/MainWindowViewModelTests.cs` — hydration, empty and offline states, reconnect, profile CRUD, and IPC failure reporting.
- `apps/EdgeOptimizer.Settings.Core/Services/RunnerPipeIdentity.cs` and `tests/EdgeOptimizer.Settings.Core.Tests/RunnerPipeIdentityTests.cs` — pipe naming and Runner identity rules.
- `crates/runner/src/main.rs` — launches the packaged WinUI client.

## Acceptance or verification criteria

- [ ] Build on .NET 10 LTS.
- [x] Hydrate profiles and active state from Runner through the transitional compatibility transport.
- [x] Perform profile collection saves through Runner; generated versioned bindings remain planned.
- [ ] Exit fully when the window closes.
- [x] Runner launches the packaged WinUI Settings client.
- [x] Tray flyout activation does not launch or initialize the WinUI Settings
  client (code path; Windows evidence pending).
- [ ] A tray double-click or explicit **Open Settings** action launches the
  full client when it is not running.
- [ ] When the client is already connected, those actions send
  `BringMainToFront` instead of starting a second Settings process.
- [ ] The client handles `BringMainToFront` through its WinUI
  `DispatcherQueue` and activates the existing window.
- [x] Save supported profile state, activate profiles, and request cleanup through Runner's transitional transport.
- [x] Enumerate processes locally and idle the fetcher when System Tweaks is hidden or the window is inactive.
- [x] Send activation termination targets directly to EngineSvc, and report when it is unavailable without blocking activation.
- [x] Include WinUI 3 build and logic tests in CI.
- [x] Never present placeholder profile, macro, or process data as Runner state; show empty and offline states instead.
- [x] Surface Runner save and send failures without crashing and without reporting success.
- [ ] Present Runner-reported capability availability and reasons, and disable unavailable controls.
- [ ] Publish framework-dependent on the Windows App SDK runtime for the Store edition.
- [x] Delete the legacy Iced settings executable and `crates/core/src/gui`.
- [x] Connect only to Runner's per-session pipe, and verify that the server is the sibling Runner before sending messages.

## Remaining gaps and unknowns

Generated Protobuf bindings, broader golden cross-language fixtures, profile rename validation, safe macro recording/test playback, and interactive Windows UI automation remain planned. `App.xaml.cs` still maps the deprecated flyout commands to the main window; Runner no longer sends them. The Bincode compatibility client is transitional and must be removed after the shared generated contract lands. GitHub Actions is the build/test authority because the local .NET 10 SDK is unavailable.
