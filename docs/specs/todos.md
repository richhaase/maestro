# Maestro Zellij Plugin – MVP Checklist (ordered toward a working prototype)

Status legend: `[ ]` not started · `[~]` in progress · `[x]` complete

## Next actions (do in order)
- [~] Agent pane persistence/resync: rebuild agent panes from events after reload (or via `list_clients` repair) so panes survive plugin reload; handle `CommandPaneExited/ReRun` to keep statuses current.
- [ ] Resync robustness: reconcile agent panes on `TabUpdate`/`PaneUpdate` deltas, drop stale entries, and add optional `list_clients` repair path if drift is detected.
- [ ] Permissions + config path polish: confirm `/host` config resolution for agents file, and surface a blocking retry prompt when permissions are denied.
- [ ] Locate persisted agent files in practice (current `/host` maps to plugin launch CWD, yielding `./.config/maestro/agents.kdl`); document actual host path/mount and resolve path strategy.
- [ ] Agent pane resync follow-up: confirm event availability post-reload (SessionUpdate/PaneUpdate) or add a `list_clients` repair path to restore panes across reloads.
- [ ] Tests: cover form parsing/validation, state-machine transitions, command construction (titles/env/cwd), and agent pane reconciliation.
- [ ] Docs: refresh README/spec to describe controls, config path, build/reload steps, and current limitations once the prototype works.

## Done
- [x] **Design refactor**: Simplified model from Session/Workspace to AgentPane/Tabs. Removed Workspace abstraction; tabs are first-class, workspace_path is just metadata (CWD). Renamed all types and methods accordingly.
- [x] Agent CRUD + persistence: parse command/env/note inputs, validate (unique name, non-empty command), call `save_agents` on add/edit/delete, and reload list immediately (KDL config at `~/.config/maestro/agents.kdl`).
- [x] Wire input handling and selections: subscribe to key events (arrows, Tab, Enter, Esc, n/a/e/d/x) to move between sections (Tabs/AgentPanes/Agents), change selected items, and surface key hints in the status line.
- [x] New agent pane wizard + launch: workspace path prompt (optional), tab selection (existing or new), agent pick/create inline, build command with env, set cwd/title context, call `open_command_pane`, and stash agent pane entry.
- [x] Agent pane actions: focus/kill bound; CommandPaneExited/CommandPaneReRun handling updates status when panes exit/rerun.
- [x] Escape-to-close behavior in View mode; Esc continues to cancel inside wizards/forms.
- [x] Rust plugin scaffold targeting `wasm32-wasip1` with dev layout for hot reload.
- [x] Permissions/request flow: request needed permissions at load and show basic denied/pending messaging.
- [x] Agent persistence layer: read/write `~/.config/maestro/agents.kdl` with validation and atomic replace.
- [x] Initial state model & basic resync: track agent panes/tabs/agents; subscribe to core events (TabUpdate, PaneUpdate, CommandPaneOpened/Exited/ReRun, etc.).
- [x] Launch pipeline: `open_command_pane` with unique tab title and env injected via argv; capture `pane_id`.
- [x] Focus/kill helpers: focus by tab name with pane fallback; close terminal pane when a pane_id is known.
- [x] Baseline UI rendering: tables for tabs/agent panes and ribbon for agents with status line.
