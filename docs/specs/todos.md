# Maestro Zellij Plugin – MVP Checklist

Status legend: `[ ]` not started · `[~]` in progress · `[x]` complete

- [x] **Rust plugin scaffold**: set up `zellij-tile` plugin crate targeting `wasm32-wasi`, add dev layout for hot reload (`start-or-reload-plugin`).
- [ ] **Permissions/request flow**: request `ReadApplicationState`, `ChangeApplicationState`, `RunCommands`/`OpenTerminalsOrPlugins` (as needed) at load; handle denial UX.
- [x] **Agent persistence layer**: read/write `~/.config/maestro/agents.{json|toml}` (via `/host`) with validation (unique name, command non-empty); resolve concurrency (last-write wins + simple guard).
- [ ] **State model & resync**: in-memory maps for workspaces/sessions; subscribe to `TabUpdate`/`PaneUpdate`/`SessionUpdate`/`CommandPane*`/`PaneClosed`; reconcile on each event; optional `list_clients` repair path.
- [ ] **Launch pipeline**: implement `open_command_pane` (or `open_terminal`) with unique tab title (add UUID) and env injected via argv (`KEY=VAL cmd ...`); capture `pane_id`; track session identity.
- [ ] **Focus/Kill actions**: focus via `go_to_tab_name` fallback `focus_*_pane`; kill via `close_*_pane` fallback close tab; clear sessions on `PaneClosed`.
- [ ] **UI rendering**: build View with Workspaces/Sessions/Agents using built-in components; render status/hints; highlight selections.
- [ ] **Agent form flow**: add/edit/delete agents (form parsing for command/env/note); persist immediately and refresh UI.
- [ ] **New session wizard**: workspace prompt -> agent select/create; create inline agent path; trigger launch on completion.
- [ ] **Error/status handling**: bubble backend errors to status line; keep state intact on failure; retries consistent.
- [ ] **Tests**: unit tests for persistence parsing, event-to-state reconciliation, command construction (titles/env), UI logic helpers.
- [ ] **Packaging/docs**: build instructions, layout usage, version pin (v0.43.1), update README/specs as work completes.
