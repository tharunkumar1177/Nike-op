# Codex Project Instructions

## Project scope

Edge Optimizer is a Windows desktop application for gaming profiles, process
cleanup, crosshair overlays, and input macros. The repository is in an
architecture migration: the WinUI 3 settings client is the active presentation
client, while Runner IPC and a hardened privileged broker are being introduced.

Treat current code and tests as the source of truth for implemented behavior.
Treat `architecture.md` and `blueprints/` as the source of truth for
intended boundaries and migration status. Never present planned behavior as
already implemented.

<system_runtime> Do not run Gradle locally; GitHub Actions is the build and test authority for this repository. </system_runtime>


## context

<context> Use <progressive_disclosure> for feature task and architecture work: </context>

1. Read `architecture.md` for cross-cutting contracts.
2. Use the router in `blueprints/README.md` to select the primary
   blueprint for the task.
3. Read only that blueprint, its linked architecture sections, and any impact
   checks relevant to the change.
4. Inspect the current implementation and tests before editing or changing a
   blueprint status.

For changes to a shared contract, read the complete architecture document and
all affected consumer blueprints.

<progressive_disclosure>

Do not load every blueprint for an isolated feature task. Read all of `.github/architecture.md` and every affected blueprint only for structural, cross-cutting, persistence-wide, coordinate-contract, or ambiguous changes.

<progressive_disclosure>


## Repository map

- `crates/core`: shared domain types, persistence, process safety, IPC, and UI
  support.
- `crates/settings`: current Rust/Iced settings executable.
- `crates/runner`: per-user tray agent, orchestration authority, worker owner,
  and durable-state owner.
- `crates/crosshair`: unprivileged crosshair worker.
- `crates/macro`: unprivileged macro worker.
- `crates/engine_service`: transitional elevated Windows engine service.
- `crates/engine_ctl`: engine-service control utility.
- `apps/EdgeOptimizer.Settings.Core`: UI-independent .NET 10 settings logic.
- `apps/EdgeOptimizer.Settings.WinUI`: active .NET 10 WinUI 3 settings client.
- `scripts`: Windows service and scheduled-task administration scripts.
- `blueprints`: feature-specific design, status, and acceptance criteria.

## Architectural constraints

- Settings clients communicate with Runner; they do not open or mutate
  Runner's database directly.
- Runner is the sole owner of `%LOCALAPPDATA%\EdgeOptimizer\state.db` and the
  authority for profiles, active-profile state, orchestration, and workers.
- Keep crosshair and macro workers unprivileged.
- Keep machine-level operations behind a minimal, allowlisted privileged
  boundary. Do not expand service privileges or add privileged fallbacks.
- Treat serialized identity fields and `AuthContext` values as claims, not as
  authentication evidence.
- Preserve explicit protocol versioning, bounded frames/messages, validation,
  and compatibility across Rust and C# consumers.
- Do not make Serde/Bincode the WinUI 3 contract; it is transitional. Protobuf over
  per-user Windows named pipes is the target contract.
- Normalize protected process names consistently. Any change that can terminate
  a process must fail safely for ambiguous, critical, or protected targets.
- Persistence changes must preserve referential integrity, use transactional
  updates, and reject schemas newer than the binary supports. Startup restore
  must not replay optimization side effects.
- Surface persistence and IPC failures. Do not silently reset valid state or
  bypass authorization when a dependency is unavailable.

## Implementation conventions

- Keep changes minimal and within the owning component. Prefer shared domain or
  protocol logic in `crates/core` over duplicating it in executables.
- Follow Rust 2021 idioms, existing error types, and existing logging patterns.
  Avoid `unwrap`/`expect` in runtime paths unless an invariant is both local and
  documented.
- Keep Windows-specific APIs behind `cfg(windows)` or target-specific
  dependencies where practical.
- In C#, keep nullable reference types enabled and preserve the WinUI client's
  presentation-only role.
- Add or update tests with behavior changes. Favor unit tests for pure state,
  normalization, framing, and validation logic; use Windows integration tests
  for named pipes, service identity, GUI, and OS behavior.
- Use the blueprint status vocabulary exactly: `Implemented`, `Partial`,
  `Planned`, `Deprecated`, or `Unknown`.
- Update the relevant blueprint when an implementation changes an acceptance
  criterion, dependency, contract, or verified status. Update
  `architecture.md` only for cross-cutting contracts.
- Do not edit generated artifacts such as `target/`, `bin/`, or `obj/`, and do
  not commit local logs or user-specific state.
- Preserve unrelated work in the tree. Do not rewrite user changes merely to
  satisfy formatting.


## Build and validation

Run the narrowest useful checks during iteration, then the applicable broader
checks before handoff.

```powershell
# Rust formatting
cargo fmt --all -- --check

# Rust compilation and tests
cargo check --workspace
cargo test --workspace

# Rust linting
cargo clippy --workspace --all-targets -- -D warnings

# WinUI client (requires the .NET 10 SDK on Windows)
dotnet build .\apps\EdgeOptimizer.Settings.WinUI\EdgeOptimizer.Settings.WinUI.csproj
```

For a focused Rust change, use `cargo test -p <package>` first. Relevant package
names include `edge_optimizer_core`, `edge_optimizer_settings`,
`edge_optimizer_runner`, `edge_optimizer_crosshair`, `edge_optimizer_macro`,
`edge_optimizer_engine_service`, and `edge_optimizer_engine_ctl`.

Release builds use:

```powershell
cargo build --workspace --release
```

If a required SDK, Windows service context, interactive desktop, or elevation is
unavailable, run all checks that are possible and state exactly what remains
unverified. Never claim GUI, service, named-pipe authorization, or elevated
behavior was validated by compilation alone.

## Safety-sensitive work

Changes involving process termination, input hooks/simulation, named-pipe
authentication, scheduled tasks, Windows services, or elevated operations are
high risk. For these changes:

- trace the full caller-to-privileged-operation path;
- validate untrusted input at the boundary and again before the operation;
- add regression tests for malformed, unauthorized, mixed-case, extensionless,
  and whitespace-padded inputs as applicable;
- avoid testing against real critical processes or modifying the developer's
  machine configuration unless the user explicitly requests it; and
- clearly separate compile/unit-test evidence from privileged Windows
  integration evidence.

## Handoff expectations

Summarize changed behavior and affected components, list the checks actually
run, and call out remaining Windows-only or privileged validation. Mention any
blueprint status change explicitly.


# Don'ts
- Do not read prompts.md
