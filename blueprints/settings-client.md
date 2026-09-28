# Settings client

## Outcome or responsibility

An on-demand, unprivileged WinUI client edits profiles and presents Runner
state while consuming no memory when closed. It owns only the full Settings
experience; Runner owns the system tray and quick flyout.

## Current verified status

**Status:** Partial

WinUI 3 is the active Settings UI and is launched on demand by Runner from the packaged `EdgeOptimizer.Settings.WinUI.exe`. It restores the full profile-scoped Dashboard, Crosshair, Macros, and System Tweaks surfaces and uses a transitional Bincode compatibility client to hydrate Runner state, save profile collections, request live processes and cleanup, and activate profiles. Runner's pipe accepts the client non-blockingly. A Windows CI job tests UI-independent logic, compiles WinUI XAML, and publishes a self-contained client artifact. Generated Protobuf bindings and runtime UI smoke automation remain planned.

The current tray single-click path is an exception to the intended on-demand
boundary: Runner launches Settings with `--flyout-only`, but WinUI ignores that
mode and loads the full Settings window. The approved target is documented in
[system tray and quick flyout](tray-and-flyout.md): Runner will own the native
flyout. A tray double-click or an explicit **Open Settings** action will start
or foreground this client.

## Architecture dependencies

- [Component boundaries](../architecture.md#component-boundaries)
- [IPC and protocol boundary](../architecture.md#ipc-and-protocol-boundary)
- [Distribution editions and install layout](../architecture.md#distribution-editions-and-install-layout)

## Local rules and implications

WinUI 3 never owns durable state or privileged operations. It requests a snapshot from Runner, submits validated commands, and exits completely when closed.

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
- `apps/EdgeOptimizer.Settings.Core/Services/RunnerPipeIdentity.cs` and `tests/EdgeOptimizer.Settings.Core.Tests/RunnerPipeIdentityTests.cs` — pipe naming and Runner identity rules.
- `crates/runner/src/main.rs` — launches the packaged WinUI client.

## Acceptance or verification criteria

- [ ] Build on .NET 10 LTS.
- [x] Hydrate profiles and active state from Runner through the transitional compatibility transport.
- [x] Perform profile collection saves through Runner; generated versioned bindings remain planned.
- [ ] Exit fully when the window closes.
- [x] Runner launches the packaged WinUI Settings client.
- [ ] Tray flyout activation does not launch or initialize the WinUI Settings
  client.
- [ ] A tray double-click or explicit **Open Settings** action launches the
  full client when it is not running.
- [ ] When the client is already connected, those actions send
  `BringMainToFront` instead of starting a second Settings process.
- [ ] The client handles `BringMainToFront` through its WinUI
  `DispatcherQueue` and activates the existing window.
- [x] Save supported profile state, activate profiles, and request live process/cleanup operations through Runner's transitional transport.
- [x] Include WinUI 3 build and logic tests in CI.
- [ ] Present Runner-reported capability availability and reasons, and disable unavailable controls.
- [ ] Publish framework-dependent on the Windows App SDK runtime for the Store edition.
- [x] Delete the legacy Iced settings executable and `crates/core/src/gui`.
- [x] Connect only to Runner's per-session pipe, and verify that the server is the sibling Runner before sending messages.

## Remaining gaps and unknowns

Generated Protobuf bindings, golden cross-language fixtures, profile rename validation, safe macro recording/test playback, and interactive Windows UI automation remain planned. The current `--flyout-only` path still loads this client and must be removed after Runner's native flyout is verified. The Bincode compatibility client is transitional and must be removed after the shared generated contract lands. GitHub Actions is the build/test authority because the local .NET 10 SDK is unavailable.
