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
  - Stored as JSON/TOML in plugin `/data/agents.{json|toml}`.
  - Unique names, non-empty command.
- **Runtime sessions** (in-memory, rebuilt from events): `{ tab_name, pane_id?, workspace_path, agent_name }`
- **Workspaces**: derived set of paths seen in sessions + user inputs; lightweight list, no file.

## Permissions (requested in `load`)
- `ReadApplicationState`, `ChangeApplicationState`
- `RunCommands`, `OpenTerminalsOrPlugins`
- Optional: `WriteToStdin` (if we later send input), `Reconfigure` (if we bind keys), `ReadCliPipes` (future piping).

## Events subscribed
- `TabUpdate`, `PaneUpdate`, `SessionUpdate`
- `CommandPaneOpened`, `CommandPaneExited`, `CommandPaneReRun`, `PaneClosed`
- `BeforeClose`
- (Optional) `ModeUpdate` for hints, `ListClients` if needed.

## Commands used
- Launch: `open_terminal` (or `open_command_pane`) with cwd, env, title `maestro:<agent>:<basename>`.
- Focus: `go_to_tab_name` or `focus_*_pane` by stored id.
- Kill: `close_terminal_pane` / `close_plugin_pane` or tab by name.
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
- **Launch**: Resolve workspace path; pick agent; call `open_terminal` with env and title; record tab name; on open events, stash pane id.
- **Focus**: Prefer `go_to_tab_name` on recorded title; fallback to pane id if present; surface error if missing.
- **Kill**: Close pane/tab; remove from in-memory sessions.
- **Resync**: Rebuild session list from Tab/Pane updates each event pass; drop stale entries.
- **Agent persistence**: save immediately on add/edit/delete to `/data`; reload into memory.
- **Exit**: `BeforeClose` no-op except best-effort flush if pending writes (writes are immediate by design).

## Development setup
- Rust + `zellij-tile`, target `wasm32-wasi`.
- Dev layout with panes: editor, build/watch (`cargo build --target wasm32-wasi && zellij action start-or-reload-plugin file:...`), live plugin pane.
- Tested against Zellij v0.43.1 (current release).

## Risks / unknowns
- Relying on tab names for focus: ensure uniqueness (`maestro:<agent>:<basename>` is usually fine but collisions possible); may add UUID suffix if needed.
- `PaneUpdate`/`TabUpdate` coverage: assume sufficient for resync; if gaps, add lightweight `ListClients` polling.
- Permissions prompts: ensure requested once at load; handle denial gracefully (show blocking message).
