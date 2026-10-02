# Edge Optimizer automated test matrix

The hosted CI suite is deliberately logic-only. It must not terminate processes, clear data,
install or control services, send keyboard or mouse input, register global hooks, or open a real
crosshair overlay. Every runnable test contains a one-line `Verifies ...` scenario description.

| Area | Automated now | Deferred until its contract exists |
|---|---|---|
| Profiles and shell | Defaults, validation, uniqueness, creation, selection, profile-scoped navigation, Runner-connected activation state | Runner-backed CRUD and active-state hydration |
| Dashboard | Readiness, summaries, incomplete configuration, quick-action routing | Live system snapshot and optimize result handling |
| Crosshair | Movement, centering, bounds, enable/hide/reset, file-picker cancel/replace/remove, PNG fixtures | Managed asset identity/copy and overlay-worker protocol |
| Macros | Search, create, select, duplicate, delete, add/remove steps, shortcut display, validation | Generated worker contract, recording acknowledgements, playback cancellation |
| System tweaks | Process filtering/counts, independent profile state, safe defaults | Live process refresh and Runner cleanup commands |
| Process safety | Name normalization, protected forms, empty result mapping | PID identity and Windows critical-process checks |
| Persistence | Fresh schema, restart state, referential activation, deletion, v1-to-v2 migration, backup, newer-schema rejection, malformed JSON | Automated backup recovery, managed assets, import/export |
| IPC and orchestration | Correlation, version claims, idempotency, result mapping, Bincode round trips, fake Engine dispatch | Protobuf Rust/C# golden fixtures, bounded frames, peer authentication |
| WinUI 3 | View-model behavior, resource/view/window STA construction, window bounds, automation metadata | Interactive desktop automation and real Runner connectivity |
| Privileged broker | None on hosted CI | Named-pipe ACL/token, SCM lifecycle, authorization, recovery on an isolated self-hosted runner |

Coverage is collected and uploaded for review without a merge-blocking percentage threshold.
