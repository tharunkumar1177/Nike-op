# Edge Optimizer Architecture

## Purpose and authority

This document defines cross-cutting contracts for the iterative architecture migration. Current code and tests are authoritative for implemented behavior; blueprints distinguish verified behavior from planned work.

## Component boundaries

- **Settings UI:** an unprivileged, on-demand WinUI presentation client. It is packaged beside Runner as `EdgeOptimizer.Settings.WinUI.exe`. Runner starts it on demand; a transitional Bincode compatibility client currently restores hydration, profile saves, cleanup intents, and activation while the generated Protobuf contract remains planned. Settings enumerates processes itself, read-only, for the System Tweaks page, and only while that page is shown in an active window. On profile activation it resolves the profile's selected apps to specific process instances and sends them directly to the Privileged Broker.
- **Runner:** the per-user startup agent, tray and quick-flyout owner, orchestration authority, worker owner, and sole durable-state owner. Termination requested from the flyout goes to the Privileged Broker.
- **Crosshair and Macro workers:** unprivileged native workers started and stopped by Runner.
- **Privileged Broker (EngineSvc):** a minimal Windows service that performs only allowlisted operations for two authenticated clients, the installed Runner and the installed Settings. It is the only component that terminates processes, and it will own scheduled cleanup. It exists only in the Full edition (see [Distribution editions and install layout](#distribution-editions-and-install-layout)). The current scheduled-task EngineSvc is transitional.

Dependencies point from clients and workers toward versioned contracts. Presentation clients never open Runner's database. Settings' only privileged call is the allowlisted process-termination operation.

## Distribution editions and install layout

This contract is planned. The current CI `bundle` job produces only an unpackaged folder of executables; no installer, package manifest, or startup registration exists yet.

- **Editions.** Two editions are built from the same binaries:
  - The **Store edition** is a single per-user MSIX package distributed through the Microsoft Store. It contains no Privileged Broker and declares no restricted capability other than `runFullTrust`.
  - The **Full edition** is a signed, per-machine installer distributed outside the Store. It installs the Privileged Broker as an SCM service with one UAC consent at install time.
- **Capability discovery.** Runner determines machine-level capability at runtime from an authenticated broker connection, never from a build flag, file presence, or edition claim. When the broker is absent or unavailable, Runner reports each affected operation to clients as unavailable with a reason. This is a normal capability state, not a failure, and it never triggers elevation or a privileged fallback.
- **Layout.** Runner, Settings, and the workers are installed in one directory; the Full edition adds the broker to it. Every component resolves sibling executables only relative to Runner's own image path, never from the current directory or `PATH`. The install directory is read-only at runtime. Mutable state lives only in the per-user locations defined in [State ownership and persistence](#state-ownership-and-persistence) and, in the Full edition, in the broker's machine journal under `%ProgramData%`.
- **Store data location.** Inside the MSIX, Windows redirects `%LOCALAPPDATA%` writes to a package-private, per-user location that is deleted on uninstall. Runner and Settings share that view, so the state ownership rules are unchanged.
- **Exclusivity.** Only one edition may be installed on a machine. Each installer detects the other edition and refuses to proceed until it is removed.
- **Startup.** Runner starts at user sign-in through the edition's own mechanism: a package startup task for the Store edition and a per-user registration for the Full edition. The user can disable it.

## State ownership and persistence

Runner exclusively owns `%LOCALAPPDATA%\EdgeOptimizer\state.db`. SQLite schema changes use `PRAGMA user_version` migrations. Profile-list replacement and active-profile changes are transactional. An active profile must reference an existing profile, and deleting that profile clears activation.

At startup Runner restores profiles, active-profile identity, and UI state. Restoration does not replay optimization side effects: process termination, cleanup, and privileged changes require a new explicit command. Two startup actions are not replays and are permitted:

- reverting journaled changes, as required by [Reversible system changes](#reversible-system-changes); and
- running a user-scheduled operation that became due or was missed. The schedule itself is the explicit, previously recorded intent.

When the database is empty, Runner may import the legacy `%APPDATA%\GamingOptimizer` JSON files once.

Crosshair files will be copied into an application-managed assets directory and referenced by asset identity. Portable import/export will use versioned JSON. These asset and export rules are planned, not yet implemented.

## IPC and protocol boundary

Settings communicates with Runner for state and orchestration, and with the Privileged Broker only to request process termination. Runner communicates with workers and the Privileged Broker. Every protocol has an explicit version, bounded message size, validation, and generated Rust/C# types. Protobuf over per-user Windows named pipes is the target contract.

The current Rust endpoints still use Serde/Bincode. This is transitional and must not become the WinUI 3 contract. Serialized identity or `AuthContext` fields are claims, not authentication evidence. Until generated bindings exist, the hand-written C# EngineSvc codec is pinned to the Rust encoding by identical byte-vector tests on both sides.

## Privilege and identity

Every operation is classified by execution context before it is implemented:

- **Presentation-only:** Settings, plus read-only process enumeration in the user's session.
- **Interactive-user:** Runner, in the signed-in user's context, with no elevation.
- **Machine-level:** the Privileged Broker. Process termination always runs here, whether or not the interactive user could end the process, so every termination passes the same final validation.

The Privileged Broker exposes a minimal allowlist of named, typed operations. Parameters are enumerations or validated identifiers, never caller-supplied commands, scripts, arbitrary paths, or service names. Anything outside the allowlist is rejected. The allowlist may grow only by adding a named operation with its own validation, authorization, and, where applicable, reversal under [Reversible system changes](#reversible-system-changes).

Its named pipe has an explicit security descriptor and is owned by SYSTEM. Clients refuse the pipe unless SYSTEM owns it. The broker reads the client's process ID and session from the pipe, accepts only the installed Runner or Settings image from its own install directory, and authorizes each operation independently. Termination is limited to processes in the requesting client's session. User-specific cleanup must resolve locations for the requesting user and never from the broker's SYSTEM environment. Until scheduled cleanup moves to the broker, the existing on-demand kinds run in Runner.

No component disables, reconfigures, or adds exclusions to Microsoft Defender or other security products. Clients may only direct the user to Windows Security.

EngineSvc now applies the pipe descriptor, SYSTEM ownership, client-image and session checks, and PID validation. It is still a SYSTEM scheduled task rather than an SCM service, and the install directory it trusts is not yet guaranteed read-only.

## Process safety

Protected process names are normalized identically for configuration input and discovered executables before comparison. Extensionless, mixed-case, and surrounding-whitespace forms must be blocked.

Windows reuses PIDs, so a termination target is a PID together with its creation time and image name, never a name or a bare PID. Immediately before terminating, the broker opens the PID and, through that same handle, re-checks:

- the creation time and image name;
- the protected-name list;
- the target's session;
- Windows' critical-process flag.

It then terminates through that handle, so the process cannot be swapped between check and use. Name normalization and this PID-level validation are implemented; Windows integration evidence is pending. Runner's flyout also holds handles on the processes it lists, so it sees exits without polling and a listed PID cannot be recycled. The same protected and critical-process rules apply to any operation that lowers another process's priority or otherwise degrades it, not only to termination.

## Reversible system changes

This contract is planned; no current code changes reversible system state.

Any operation that changes persistent or session-wide system state records the prior value durably before applying the change. This includes process priority, service run state, power plans and core-parking settings, and similar configuration; deleting cache or temporary files is excluded. The component that makes the change owns its journal: Runner stores interactive-user entries in `state.db`, and the Privileged Broker keeps machine-level entries in its machine journal.

Journaled changes are reverted on profile deactivation, on Runner exit, and at the next start of the owning component after a crash or reboot. A revert failure is surfaced per operation, and its journal entry is retained until the revert succeeds or the user explicitly dismisses it. Services are paused by stopping them for the active session only; their configured startup type is never changed.

File deletion is not reversible. It therefore requires explicit user intent, either an immediate command or a user-created schedule, and a preview of what will be removed.

## Failure and recovery

Persistence failures are surfaced and must not silently reset valid state. A schema newer than the binary supports is rejected. Database updates use transactions, and Runner checkpoints/copies the last valid database before each state mutation. Automated recovery from that backup is planned.

IPC failures degrade the affected capability and never authorize a privileged fallback. Duplicate request tracking must be bounded by time and size; the current in-memory unbounded cache is transitional.

## Verification boundaries

- State-store unit tests verify schema creation, restart restoration, referential activation, and invalid activation.
- Process-safety unit tests verify all accepted name forms.
- IPC contract tests must round-trip generated messages in both Rust and C# before WinUI 3 enables Runner-backed behavior.
- Broker integration tests must cover standard-user access, unauthorized clients (including a non-sibling image and a squatted pipe), malformed frames, PID reuse, and service restart.
- Windows UI and service behavior require Windows integration tests; documentation alone never marks them implemented.
- Reversible changes require unit tests that apply and revert through fake system interfaces, plus isolated Windows integration tests for crash and reboot recovery. They are never exercised against a developer machine.
- Building an MSIX or installer in CI is compile evidence only. Install, upgrade, repair, uninstall, startup registration, and edition exclusivity require clean Windows virtual machines.

## Feature blueprints

See [the blueprint manifest](blueprints/README.md).
