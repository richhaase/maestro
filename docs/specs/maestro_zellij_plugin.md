# Maestro Zellij Plugin — Ground-Up Plan (v0.43.1)

This replaces the Go app entirely. It is a Zellij-only plugin (Rust `zellij-tile`, `wasm32-wasi`) that lives inside Zellij and drives everything through the plugin API. No external mux, no legacy config/state baggage.

## What it does
- Show three pillars: **Workspaces** (paths), **Sessions** (per workspace), **Agents** (command presets).
- Create a new session: pick workspace path (defaults to caller cwd), pick agent (or create inline), spawn a pane/tab in Zellij running that agent.
- Jump to an existing session (focus tab/pane).
- Kill a session (close its pane/tab).
- Add/edit/delete agent presets, persisted for reuse.

## What it does NOT do
- Support tmux or any non-Zellij backend.
- Reuse or be compatible with the Go config/state schemas.
- Depend on external files beyond the plugin’s own storage.

## Data model & persistence
- **Agents** (persisted): `{ name, command[], env{K:V}?, note? }`
  - Stored as JSON/TOML at `~/.config/maestro/agents.{json|toml}` (resolved under `/host`).
  - Unique names, non-empty command.
- **Runtime sessions** (in-memory, rebuilt from events): `{ tab_name, pane_id?, workspace_path, agent_name }`
  - Session identity: `tab_name` + `pane_id` (when available) + `workspace_path` + `agent_name`. Keep all four to avoid killing/focusing the wrong pane when duplicates exist.
- **Workspaces**: derived set of paths seen in sessions + user inputs; lightweight list, no file.
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
- Sections: Workspaces (with counts), Sessions (for selected workspace), Agents.
- Modes:
  - View: navigate with arrows, Tab to switch section, Enter to focus session, `x` to kill, `n` new session, `a` add agent, `e` edit agent, `d` delete agent.
  - New Session wizard: prompt workspace path (default caller cwd), then agent select or create inline.
  - Agent form: name, command (space-split), env (KEY=VAL, comma-separated), note (optional).
- Status line for errors/info; concise key hints.

## Behavior details
- **Launch**: Resolve workspace path; pick agent; call `open_command_pane` (or `open_terminal`) with env baked into argv and unique title; record tab name; when `CommandPaneOpened`/`PaneUpdate` arrives, stash pane id.
- **Focus**: Prefer `go_to_tab_name` on recorded title; fallback to pane id if present; surface error if missing.
- **Kill**: Close by `pane_id` when known; otherwise close tab by name as last resort; then drop session from memory.
- **Resync**: Maintain a “seen panes” map; rebuild sessions from `TabUpdate`/`PaneUpdate` on each pass; drop stale entries. Keep optional repair path via `list_clients` if event sync drifts.
- **Agent persistence**: save immediately on add/edit/delete to `/data`; reload into memory.
- **Exit**: `BeforeClose` best-effort; writes are immediate so nothing critical to flush.

## Event/command matrix (how each action works)
- **Launch**: request permissions in `load`; issue `open_command_pane` (or `open_terminal`) with cwd/env/title; update state on `CommandPaneOpened` (pane_id) and `PaneUpdate`/`TabUpdate` deltas; expect `CommandPaneExited/ReRun` for lifecycle.
- **Focus**: call `go_to_tab_name` using stored title; if that fails, call `focus_*_pane` with `pane_id`; TabUpdate reflects focus change if needed for UI.
- **Kill**: call `close_terminal_pane` / `close_plugin_pane` with `pane_id`; if unknown, close tab by name; expect `PaneClosed` and drop from sessions.
- **Resync**: reconcile on every `TabUpdate`/`PaneUpdate`; optional manual repair via `list_clients` + reply event; rebuild session map from events, not persisted state.
- **Agent CRUD**: no plugin events; read/write `/data/agents.*`; rehydrate agents list immediately after file write.
- **BeforeClose**: ignore for persistence (writes immediate); can clear transient state.

## State machine (ASCII)
```
                +-------------+
                |    View     |
                +------+------+ 
        n  / a / e / d | enter / x / esc
                       v
        +--------------+-------------+
        | NewSessionWorkspace        |
        +--------------+-------------+
             | enter(valid)             | esc
             v                          |
        +----+---------------------+    |
        | NewSessionAgentSelect    |<---+
        +-----------+--------------+
            | enter on existing -> Launch -> View
            | enter on "create"
            v
        +---+----------------------+ 
        | NewSessionAgentCreate    |
        +-----------+--------------+
            | enter advance/save -> Launch -> View
            | esc -> NewSessionAgentSelect

From View:
- a -> AgentForm (isNew) -> enter advances/save -> View; esc -> View
- e -> AgentForm (edit)  -> enter advances/save -> View; esc -> View
- d -> DeleteConfirm -> y delete -> View; n/esc -> View
- enter on session -> Focus -> View
- x on session -> Kill -> View

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
