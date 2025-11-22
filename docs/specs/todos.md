# Maestro Zellij Plugin – MVP Checklist (ordered toward a working prototype)

Status legend: `[ ]` not started · `[~]` in progress · `[x]` complete

## Next actions (do in order)
- [ ] Implement the View/NewSession/AgentForm state machine: drive transitions per spec (workspace prompt -> agent select/create; agent add/edit/delete with confirmations), keep state when errors occur.
- [ ] Agent CRUD + persistence: parse command/env/note inputs, validate (unique name, non-empty command), call `save_agents` on add/edit/delete, and reload list immediately.
- [ ] Session persistence/resync: rebuild sessions from events after reload (or via `list_clients` repair) so sessions survive plugin reload; keep workspace tab mapping intact.
- [ ] New session wizard + launch: workspace prompt defaulting to caller cwd, tab selection (existing or new), agent pick/create inline, build command with env, set cwd/title context, call `open_command_pane`, and stash session entry.
- [ ] Session actions: hook Enter to focus and `x` to kill selected session, handle `CommandPaneExited`/`CommandPaneReRun`/`PaneClosed` to update status, and fall back to tab-name focus/close when pane_id is missing.
- [ ] Resync robustness: reconcile sessions on `TabUpdate`/`PaneUpdate` deltas, drop stale entries, repair workspace list, and add optional `list_clients` repair path if drift is detected.
- [ ] Permissions + config path polish: confirm `/host` config resolution for agents file, and surface a blocking retry prompt when permissions are denied.
- [ ] Tests: cover form parsing/validation, state-machine transitions, command construction (titles/env/cwd), and session reconciliation.
- [ ] Docs: refresh README/spec to describe controls, config path, build/reload steps, and current limitations once the prototype works.

## Done
- [x] Wire input handling and selections: subscribe to key events (arrows, Tab, Enter, Esc, n/a/e/d/x) to move between sections, change selected items, and surface key hints in the status line; initial wizard/forms scaffolding.
- [x] Rust plugin scaffold targeting `wasm32-wasi` with dev layout for hot reload.
- [x] Permissions/request flow: request needed permissions at load and show basic denied/pending messaging.
- [x] Agent persistence layer: read/write `~/.config/maestro/agents.toml` with validation and atomic replace.
- [x] Initial state model & basic resync: track sessions/workspaces/agents; subscribe to core events.
- [x] Launch pipeline: `open_command_pane` with unique tab title and env injected via argv; capture `pane_id`.
- [x] Focus/kill helpers: focus by tab name with pane fallback; close terminal pane when a pane_id is known.
- [x] Baseline UI rendering: tables for workspaces/sessions and ribbon for agents with status line.
