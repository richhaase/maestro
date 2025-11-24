# Maestro

Maestro is a Zellij plugin that launches and manages "agent" command panes with a keyboard-only workflow.

## Quick Start
1. Install prerequisites: `zellij` ≥ 0.43, `rustup`, and `wasmtime`/`wasm32-wasip1` target support.
2. Add the WASI target once: `rustup target add wasm32-wasip1`.
3. Build the plugin: `cargo build --release --target wasm32-wasip1`.
4. Install it for Zellij:
   ```
   mkdir -p ~/.config/zellij/plugins
   cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
   ```
5. Launch inside Zellij:
   ```
   zellij action start-plugin file:~/.config/zellij/plugins/maestro.wasm
   ```
   or reference the plugin inside a layout:
   ```
   plugins {
       maestro location="file:~/.config/zellij/plugins/maestro.wasm" {
           cwd "/Users/you"  # point to your home directory for consistent /host mapping
       }
   }
   ```

## Running & Key Commands
- `Tab` switches between the Agent Pane list and the Agent Config list.
- `↑ / ↓` move within the focused section.
- `Enter` focuses the selected agent pane or starts the new-pane wizard when the Agents list is focused.
- `n` starts the "new agent pane" wizard.
- `f` toggles filter mode in the Agent Pane section (type to fuzzy-filter).
- `x` kills the selected running pane.
- `a` opens the agent creation form; `e` edits; `d` deletes (with confirmation).
- The new-pane wizard flows through workspace input → tab selection → agent selection/creation.

## Build & Install
```
rustup target add wasm32-wasip1      # once
cargo build --release --target wasm32-wasip1
mkdir -p ~/.config/zellij/plugins
cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
```
Reload Zellij or use `zellij action start-plugin file:~/.config/zellij/plugins/maestro.wasm` to attach Maestro.
Always install the optimized `--release` build, as debug artifacts are significantly larger and slower to load.

## Configuration
Agent definitions live at `~/.config/maestro/agents.kdl`. Maestro merges this file with built-in agents (cursor, claude, gemini, codex) and deduplicates by name.

Example:
```
agent name="my-agent" note="Internal helper" {
    cmd "my-agent-cli" "--api"
    env "API_KEY" "sk-xxx"
}
```
- `cmd` entries become the command + args executed in each pane.
- `env` pairs are exported before the command (rendered as `KEY=value` prefixes).
- Notes appear in the Agent Config table.

Use the in-plugin Agent Config section (`a` to add, `e` to edit) to persist changes; Maestro writes only non-default agents back to the file.

## Development
- Run tests: `cargo test`.
- Run clippy when modifying logic: `cargo clippy --all-targets`.
- Logs/errors show up in the Zellij host terminal; use `RUST_LOG` as needed when running `zellij`.
