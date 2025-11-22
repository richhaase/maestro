# Maestro Zellij Plugin – MVP Checklist (ordered toward a working prototype)

Status legend: `[ ]` not started · `[~]` in progress · `[x]` complete

## Next actions (do in order)

### High Priority
- [ ] **Smart Wizard Skipping**: Skip workspace step if empty (go straight to tab select), skip tab select if only one tab exists, remember last used workspace/tab as defaults.
- [x] **Quick Launch from Agents Section**: Press `n` on an agent in Agents section to launch it directly (prompts for workspace and tab, skips agent selection).
- [ ] **Better Status Indicators**: Color coding (green for RUNNING, red for EXITED), show exit codes for exited panes (e.g., "EXITED(1)"), timestamps for when pane was created/last active.

### Medium Priority
- [ ] **More Context in Tables**: Show workspace path in Maestro table (maybe as tooltip/hover or 4th column), truncate long paths intelligently, show agent command preview in Agents table.
- [ ] **Bulk Operations**: Select multiple panes (with Shift or marks), kill all panes in a tab, kill all exited panes.
- [ ] **Better Filtering**: Fuzzy matching instead of substring, filter by status (running/exited), filter by tab name.
- [ ] **Agent Templates/Groups**: Tag agents (e.g., "dev", "prod", "monitoring"), filter agents by tags, agent groups/categories.
- [ ] **More Quick Actions**: `:` command mode (like vim) for advanced actions, `g` + key for "go to" actions (g+t for tab, g+a for agent), `?` for help overlay.
- [ ] **Workspace Presets**: Save common workspace paths, quick select from preset list, remember frequently used paths.

### Low Priority
- [ ] **Recent/Favorites**: Track recently launched agents (last 3-5), quick access to recent agents, optional "favorites" or "pinned" agents.
- [ ] **Empty States**: Helpful messages when lists are empty: "Press 'n' to create your first agent pane", quick tips in empty states.
- [ ] **Duplicate/Restart Actions**: `r` key to restart selected agent pane, `d` key to duplicate agent pane (same agent, new pane), quick relaunch of exited panes.

### Infrastructure/Polish
- [ ] Permissions + config path polish: confirm `/host` config resolution for agents file, and surface a blocking retry prompt when permissions are denied.
- [ ] Locate persisted agent files in practice (current `/host` maps to plugin launch CWD, yielding `./.config/maestro/agents.kdl`); document actual host path/mount and resolve path strategy.
- [ ] Tests: cover form parsing/validation, state-machine transitions, command construction (titles/env/cwd), and agent pane reconciliation.
- [ ] Docs: refresh README/spec to describe controls, config path, build/reload steps, and current limitations once the prototype works.

## Done
- [x] **Quick Launch from Agents Section**: Press `n` on an agent in Agents section to launch it directly. Wizard prompts for workspace and tab, skipping agent selection since agent is already chosen.
- [x] **UX: Vim key navigation**: Replaced arrow keys with j/k for movement (except in filter mode where arrows work). Updated section tabs to use ribbon component. Reordered Maestro table columns to Tab, Agent, Status.
- [x] **Design refactor**: Simplified model from Session/Workspace to AgentPane/Tabs. Removed Workspace abstraction; tabs are first-class, workspace_path is just metadata (CWD). Renamed all types and methods accordingly.
- [x] **Pane tracking fixes**: Fixed pane identification by separating `pane_title` (internal maestro title) from `tab_name` (actual Zellij tab). Fixed tab name stability by only updating when empty/invalid, preventing count flickering when tabs are reordered.
- [x] **Pane recovery**: Implemented heuristic recovery of panes after plugin reload by matching command pane titles to agent names (handles Zellij changing pane titles to command names). Panes are correctly restored and tracked across reloads.
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
