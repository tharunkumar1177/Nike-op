# IPC contracts

## Outcome or responsibility

Runner, WinUI 3, workers, and the broker communicate through versioned, bounded, validated cross-language messages with OS-backed peer identity.

## Current verified status

**Status:** Partial

Current named-pipe communication uses Rust Serde/Bincode. A bounded C# compatibility codec now connects WinUI to Runner for state, saves, live process snapshots, cleanup intents, activation, and orchestration results. Runner accepts connections non-blockingly. Protobuf, authoritative generated bindings, explicit length framing, and authenticated broker identity are not implemented.

## Architecture dependencies

- [IPC and protocol boundary](../architecture.md#ipc-and-protocol-boundary)
- [Privilege and identity](../architecture.md#privilege-and-identity)

## Local rules and implications

The `.proto` schema will be authoritative for wire representation. Serialized auth fields are informational only; peer identity comes from the named-pipe token and ACL.

The current pipe names (`\\.\pipe\EdgeOptimizerIPC`, `EdgeOptimizerMacroIPC`, and `EdgeOptimizerEngineIPC` in `crates/core/src/ipc.rs` and `engine_ipc.rs`) are machine-global and use default security. Runner's Settings and worker pipes must become per-user, with names derived from the user's SID or logon session and a security descriptor that grants access only to that user. The broker pipe stays machine-wide and follows [Privileged broker](privileged-broker.md).

Planned message families:

- capability availability with reasons, so clients can present Store-edition limits;
- cleanup settings, preview, run, progress, and per-category results for [Disk cleanup](disk-cleanup.md); and
- activation plans with per-adjustment apply and revert results for [FPS Boost](fps-boost.md).

Operations are identified by enumerations, never by filesystem paths, service names, or commands.

The Runner-owned quick flyout is in-process and must not use the
Runner/Settings pipe. Launching a new Settings process also does not require an
IPC message. When Settings is already connected, Runner sends the explicit
`BringMainToFront` window command over the named pipe. The Settings transport
receives it on its listener thread and hands it to WinUI's `DispatcherQueue`;
`DispatcherQueue` is a Settings implementation detail, not part of the wire
protocol and not a Runner dependency.

Transitional `ShowFlyout` and `HideFlyout` messages are deprecated by the
Runner-owned flyout design. They remain present only until that flyout is
implemented and verified.

## Related blueprints

### Required

None.

### Impact checks

- [Settings client](settings-client.md) — C# bindings and client transport.
- [Privileged broker](privileged-broker.md) — authenticated privileged endpoint.
- [Crosshair overlay](crosshair-overlay.md) — WinUI/Runner state and Runner/worker lifecycle messages.
- [Macro automation](macro-automation.md) — edit, record, playback, cancellation, and worker events.
- [System Tweaks](system-tweaks.md) — snapshots, activation plans, commands, and structured results.
- [Disk cleanup](disk-cleanup.md) — check when cleanup settings, preview, run, or result messages change.
- [FPS Boost](fps-boost.md) — check when adjustment plans or apply and revert results change.
- [System tray and quick flyout](tray-and-flyout.md) — removal of transitional
  flyout messages and preservation of the full Settings window command.

## Relevant implementation and tests

- `crates/core/src/ipc.rs` — transitional Settings/Runner pipe.
- `crates/core/src/engine_ipc.rs` — transitional Runner/Engine pipe.
- `crates/core/src/orchestration.rs` — transitional envelope and operations.

## Acceptance or verification criteria

- [ ] Generate Rust and C# bindings from one Protobuf schema.
- [ ] Derive Runner's Settings and worker pipe names per user, and apply a security descriptor restricted to that user.
- [ ] Carry capability availability and reasons from Runner to Settings.
- [ ] Enforce protocol version and maximum frame length before decoding.
- [ ] Reject unknown or malformed privileged operations.
- [ ] Verify broker peer identity from Windows, not message claims.
- [ ] Bound idempotency retention by time and size.
- [ ] Define `BringMainToFront` as a Runner-to-Settings command distinct from
  all Runner-local flyout actions.
- [ ] Verify that tray single-click and Runner-owned flyout actions produce no
  Runner-to-Settings IPC.
- [ ] Verify that a connected Settings client receives one
  `BringMainToFront` command for double-click or **Open Settings** and dispatches
  activation through its UI thread.

## Remaining gaps and unknowns

The complete target contract remains to be implemented before the active WinUI Settings client can enable Runner-backed behavior. Transitional
`ShowFlyout`/`HideFlyout` messages remain in the current Bincode contract and
must be removed after the Runner-owned flyout is verified.
