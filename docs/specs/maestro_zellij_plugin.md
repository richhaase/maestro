# Maestro Zellij Plugin — Ground-Up Plan (v0.43.1)

This replaces the Go app entirely. It is a Zellij-only plugin (Rust `zellij-tile`, `wasm32-wasi`) that lives inside Zellij and drives everything through the plugin API. No external mux, no legacy config/state baggage.

## What it does
- Show three pillars: **Tabs** (Zellij tabs with agent panes), **Agent Panes** (panes running agents), **Agents** (command presets).
- Create a new agent pane: pick workspace path (optional, for CWD), pick tab (existing or new), pick agent (or create inline), spawn a pane in Zellij running that agent.
- Jump to an existing agent pane (focus tab/pane).
- Kill an agent pane (close its pane).
- Add/edit/delete agent presets, persisted for reuse.

## What it does NOT do
- Support tmux or any non-Zellij backend.
- Reuse or be compatible with the Go config/state schemas.
- Depend on external files beyond the plugin’s own storage.

## Data model & persistence
- **Agents** (persisted): `{ name, command[], env{K:V}?, note? }`
  - Stored as KDL at `~/.config/maestro/agents.kdl` (resolved under `/host`).
  - Unique names, non-empty command.
- **Agent Panes** (in-memory, rebuilt from events): `{ pane_title, tab_name, pane_id?, workspace_path, agent_name, status }`
  - `pane_title`: Internal maestro title (e.g., `maestro:<agent>:<basename>:<uuid>`) set when creating pane
  - `tab_name`: Actual Zellij tab name where the pane lives (may differ from `pane_title` if Zellij changes it)
  - `pane_id`: Zellij-assigned pane ID (available after `CommandPaneOpened` event)
  - `workspace_path`: CWD for the agent pane
  - `agent_name`: Name of the agent running in this pane
  - `status`: Running or Exited with exit code
  - Identity: `pane_id` is primary identifier; `pane_title` used for matching during creation; `tab_name` used for filtering/display
- **Tabs**: List of Zellij tab names, updated from `TabUpdate` events
- **Concurrency**: `/data` is shared across plugin instances (multi-client). Writes are atomic/overwrite; “last write wins”. Consider simple file locks or retry if write fails.

## Permissions (requested in `load`)
- `ReadApplicationState`, `ChangeApplicationState`
- `RunCommands` (for `open_command_pane`) and/or `OpenTerminalsOrPlugins` (for `open_terminal`) depending on launch method. Request the minimal set you actually use.
- Optional: `WriteToStdin` (if we later send input), `Reconfigure` (if we bind keys), `ReadCliPipes` (future piping).
- Handle denial: surface a blocking error with a single retry/re-request path.

## Events subscribed
- `TabUpdate`, `PaneUpdate`, `SessionUpdate`
- `CommandPaneOpened`, `CommandPaneExited`, `CommandPaneReRun`, `PaneClosed`
- `BeforeClose`
- (Optional) `ModeUpdate` for hints, `ListClients` if needed.

## Commands used
- Launch: Prefer **`open_command_pane`** (pane IDs via `CommandPaneOpened`). Inject env by prefixing argv entries as `KEY=VAL cmd ...` (or a thin wrapper if needed). Use **`open_terminal`** only if you want a plain shell and don’t need env; you may need to infer IDs from `PaneUpdate` deltas. Always set a unique title (eg. `maestro:<agent>:<basename>:<uuid4>`).
- Focus: Try `go_to_tab_name` with stored tab title; fall back to `focus_*_pane` (using `pane_id`) if the tab was renamed/duplicated.
- Kill: `close_terminal_pane` / `close_plugin_pane` by `pane_id`; if missing, close tab by name as last resort.
- Bookkeeping: `hide_self`/`show_self` only if we choose to temporarily hide.

## UI/UX
- Single pane UI using built-in components (Table/NestedList/Text).
- Sections: Tabs (with agent pane counts), Agent Panes (filtered by selected tab), Agents.
- Modes:
  - View: navigate with arrows, Tab to switch section, Enter to focus agent pane, `x` to kill, `n` new agent pane, `a` add agent, `e` edit agent, `d` delete agent.
  - New Agent Pane wizard: prompt workspace path (optional, for CWD), then tab selection, then agent select or create inline.
  - Agent form: name, command (space-split), env (KEY=VAL, comma-separated), note (optional).
- Status line for errors/info; concise key hints.

## Behavior details
- **Launch**: Resolve workspace path (optional, for CWD); pick tab (existing or create new); pick agent; call `open_command_pane` with env baked into argv and unique title (`maestro:<agent>:<basename>:<uuid>`); record tab name; when `CommandPaneOpened`/`PaneUpdate` arrives, stash pane id.
- **Focus**: Prefer `go_to_tab_name` on recorded tab name; fallback to pane id if present; surface error if missing.
- **Kill**: Close by `pane_id` when known; then drop agent pane from memory.
- **Resync**: Maintain a “seen panes” map; rebuild agent panes from `TabUpdate`/`PaneUpdate` on each pass; drop stale entries. Keep optional repair path via `list_clients` if event sync drifts.
- **Agent persistence**: save immediately on add/edit/delete to `/data`; reload into memory.
- **Exit**: `BeforeClose` best-effort; writes are immediate so nothing critical to flush.

## Event/command matrix (how each action works)
- **Launch**: request permissions in `load`; issue `open_command_pane` with cwd/env/title (`maestro:<agent>:<basename>:<uuid>`); add pane entry with `pane_id: None`; update on `CommandPaneOpened` (set `pane_id`, match by `pane_title` from context); `PaneUpdate` updates status/tab_name by `pane_id`.
- **Focus**: call `go_to_tab_name` using stored tab name; if that fails, call `focus_*_pane` with `pane_id`; TabUpdate reflects focus change if needed for UI.
- **Kill**: call `close_terminal_pane` / `close_plugin_pane` with `pane_id`; expect `PaneClosed` and drop from agent panes.
- **Resync**: On plugin reload, `SessionUpdate` rebuilds panes using heuristic (match command pane titles to agent names, since Zellij changes titles). On `TabUpdate`, update tab list and remove panes in deleted tabs. On `PaneUpdate`, update status/tab_name by `pane_id` (only update `tab_name` if empty or invalid to prevent reassignments). Rebuild agent pane list from events, not persisted state.
- **Agent CRUD**: no plugin events; read/write `/data/agents.kdl`; rehydrate agents list immediately after file write.
- **BeforeClose**: ignore for persistence (writes immediate); can clear transient state.

## State machine (ASCII)
```
                +-------------+
                |    View     |
                +------+------+ 
        n  / a / e / d | enter / x / esc
                       v
        +--------------+-------------+
        | NewPaneWorkspace           |
        +--------------+-------------+
             | enter/tab                 | esc
             v                          |
        +----+---------------------+    |
        | NewPaneTabSelect         |<---+
        +-----------+--------------+
             | enter                    | esc
             v                          |
        +----+---------------------+    |
        | NewPaneAgentSelect       |<---+
        +-----------+--------------+
            | enter on existing -> Launch -> View
            | enter on "create"
            v
        +---+----------------------+ 
        | NewPaneAgentCreate       |
        +-----------+--------------+
            | enter advance/save -> Launch -> View
            | esc -> NewPaneAgentSelect

From View:
- a -> AgentForm (isNew) -> enter advances/save -> View; esc -> View
- e -> AgentForm (edit)  -> enter advances/save -> View; esc -> View
- d -> DeleteConfirm -> y delete -> View; n/esc -> View
- enter on agent pane -> Focus -> View
- x on agent pane -> Kill -> View

Notes:
- Errors keep current state and surface status; retries stay local.
- Resync runs in background off events; does not change UI state.
```

## Development setup
- Rust + `zellij-tile`, target `wasm32-wasi`.
- Dev layout with panes: editor, build/watch (`cargo build --target wasm32-wasi && zellij action start-or-reload-plugin file:...`), live plugin pane.
- Tested against Zellij v0.43.1 (current release).

## Risks / unknowns
- Relying on tab names for focus: ensure uniqueness (`maestro:<agent>:<basename>` is usually fine but collisions possible); may add UUID suffix if needed.
- `PaneUpdate`/`TabUpdate` coverage: assume sufficient for resync; if gaps, add lightweight `ListClients` polling.
- Permissions prompts: ensure requested once at load; handle denial gracefully (show blocking message).
