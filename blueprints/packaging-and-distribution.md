# Packaging and distribution

## Outcome or responsibility

Each edition installs, starts, updates, and uninstalls as one unit. The Store edition is a single MSIX package in the Microsoft Store. The Full edition is one signed setup executable that also installs the Privileged Broker. Both are built reproducibly by GitHub Actions from the same Rust and .NET outputs.

## Current verified status

**Status:** Planned

Code inspection on 2026-09-28:

- The `bundle` job in `.github/workflows/buildntest.yml` copies the release Rust executables and the self-contained WinUI publish output into one unpackaged artifact folder. No MSIX manifest, WiX project, signing step, or installer exists.
- Runner resolves `EdgeOptimizer.Settings.WinUI.exe` beside its own image (`spawn_settings_window` in `crates/runner/src/main.rs`), and the Macro worker is resolved the same way (`crates/core/src/macro_worker.rs`).
- `crates/core/src/crosshair_overlay.rs` also searches the current directory and other fallback paths, and stops the overlay with `taskkill /F /IM EdgeOptimizer_Crosshair.exe`.
- No code registers Runner to start at sign-in.
- The broker is installed only by `scripts/install-engine-service.ps1` as a SYSTEM scheduled task.
- `apps/EdgeOptimizer.Settings.WinUI` is built with `WindowsPackageType=None` and a self-contained Windows App SDK.

## Architecture dependencies

- [Distribution editions and install layout](../architecture.md#distribution-editions-and-install-layout)
- [Component boundaries](../architecture.md#component-boundaries)
- [Privilege and identity](../architecture.md#privilege-and-identity)
- [State ownership and persistence](../architecture.md#state-ownership-and-persistence)
- [Verification boundaries](../architecture.md#verification-boundaries)

## Local rules and implications

### Distribution editions and install layout

The Store package manifest:

- declares Runner as the entry application, with a `desktop:StartupTask`;
- declares Settings as a second application with no Start menu entry;
- includes the Crosshair and Macro workers as package files;
- declares only `runFullTrust`; and
- takes a framework dependency on the Windows App SDK runtime package, so the WinUI client is published framework-dependent for this edition.

The package contains no broker executable, no `desktop6:Service`, and no `packagedServices` or `localSystemServices` capability. The legacy Iced settings executable and `EdgeOptimizer_EngineCtl.exe` are not packaged in either edition.

The Full edition is a WiX v5 per-machine MSI wrapped in a WiX Burn bundle that produces one setup executable.

- The MSI installs all binaries, including `EdgeOptimizer_EngineSvc.exe`, under `%ProgramFiles%`.
- It registers the broker with the Service Control Manager, including recovery actions and an explicit service security descriptor, and removes the service on uninstall.
- It registers Runner for per-user start at sign-in.
- The bundle installs any runtime prerequisites itself; it does not download them at install time.
- The WinUI client stays self-contained for this edition unless the bundle chains the Windows App SDK runtime installer.

Each installer detects the other edition (the MSIX package family or the MSI upgrade code) and blocks installation, with an actionable message, until the other edition is removed.

### Component boundaries

Packaging never changes component ownership. Settings is still launched only by Runner or by the user, and workers are still started and stopped only by Runner.

### Privilege and identity

The Store edition never elevates. The Full edition asks for UAC consent once, for the per-machine MSI transaction. After that, privileged work happens only through the broker's allowlist; neither edition adds an elevated helper, scheduled task, or `requireAdministrator` manifest.

### State ownership and persistence

Installers never create, migrate, or delete `state.db`. Runner owns migrations on first start. A Full edition uninstall leaves per-user state in place. Removing the MSIX deletes the package-private copy, as Windows does for all MSIX packages.

### Verification boundaries

CI builds and validates package structure, signatures, and manifest schema. Install, upgrade, uninstall, startup registration, and edition exclusivity are verified only on clean Windows virtual machines.

## Related blueprints

### Required

- [Privileged broker](privileged-broker.md) — the Full edition installs, recovers, and removes the broker service it defines.
- [Settings client](settings-client.md) — owns the WinUI publish settings that differ between editions.

### Impact checks

- [System tray and quick flyout](tray-and-flyout.md) — check when startup registration or Runner launch arguments change.
- [Crosshair overlay](crosshair-overlay.md) — check when worker path resolution or the stop mechanism changes.
- [Macro automation](macro-automation.md) — check when worker path resolution or packaging of input-hook components changes.
- [Profile persistence](profile-persistence.md) — check when the data location, uninstall behavior, or legacy import path changes.

## Relevant implementation and tests

- `.github/workflows/buildntest.yml` — current `bundle` job; future MSIX and WiX packaging jobs.
- `scripts/publish-winui-settings.ps1` — current WinUI publish beside Runner.
- `scripts/install-engine-service.ps1` — transitional scheduled-task installer to be replaced by the WiX service registration.
- `apps/EdgeOptimizer.Settings.WinUI/EdgeOptimizer.Settings.WinUI.csproj` — packaging type and Windows App SDK deployment mode.
- `crates/runner/src/main.rs` — Settings path resolution and single-instance mutex.
- `crates/core/src/crosshair_overlay.rs` — crosshair worker path resolution and stop behavior.
- `crates/core/src/macro_worker.rs` — Macro worker path resolution.

## Acceptance or verification criteria

- [ ] CI produces an unsigned `.msixupload` for Store submission and a test-signed `.msix` for sideload validation.
- [ ] The Store manifest declares only `runFullTrust`, a Runner startup task, a Settings application without a Start menu entry, and a Windows App SDK framework dependency.
- [ ] CI produces a signed Full edition setup executable containing the per-machine MSI.
- [ ] The Full edition installs, repairs, upgrades, and uninstalls the broker service with recovery actions and leaves no orphaned service or task.
- [ ] Every component resolves sibling executables only beside Runner's image.
- [ ] Runner stops workers by the process handle it owns, never by image name.
- [ ] Runner starts at sign-in in both editions, and the user can disable it.
- [ ] Each installer blocks while the other edition is present.
- [ ] Neither edition ships the legacy Iced settings executable or `EdgeOptimizer_EngineCtl.exe`.
- [ ] The Store package passes the Windows App Certification Kit.
- [ ] Install, upgrade, uninstall, startup, and exclusivity are verified on clean Windows virtual machines.

## Remaining gaps and unknowns

- Partner Center identity values (package name, publisher, and publisher display name) are not yet available and must be supplied before a Store build can be submitted.
- A code-signing certificate for the Full edition, and hosting and update delivery for it, are not yet selected.
- Whether the Full edition's WinUI client stays self-contained or chains the Windows App SDK runtime is undecided.
- Store policy review of the macro input-simulation feature and the optimization claims in the listing has not happened.
