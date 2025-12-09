# Maestro
Zellij plugin that spawns and manages panes running CLI-based AI coding agents (Claude, Cursor, etc.) entirely from the keyboard.

## Quick Start
1. **Build the plugin once**
   ```sh
   git clone https://github.com/richhaase/maestro.git
   cd maestro
   rustup target add wasm32-wasip1
   cargo build --release --target wasm32-wasip1
   mkdir -p ~/.config/zellij/plugins
   cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
   ```

2. **Wire up the hot-key exactly as shown in `~/.config/zellij/config.kdl`**

   Edit `~/.config/zellij/config.kdl` and ensure the following entries exist (replace `/home/you` with your actual host home directory):

   ```kdl
   keybinds clear-defaults=true {
     ...
     shared_among "normal" "locked" {
       ...
       bind "Alt m" {
         LaunchOrFocusPlugin "file:~/.config/zellij/plugins/maestro.wasm" {
           floating true
           move_to_focused_tab true
           cwd "/home/you"
         }
       }
     }
     ...
   }

   plugins {
     ...
     maestro location="file:~/.config/zellij/plugins/maestro.wasm" {
       cwd "/home/you"
     }
   }
   ```

   - The `bind "Alt m"` stanza lives in the `shared_among "normal" "locked"` block so the shortcut works from either mode.
   - The `plugins { maestro ... }` alias preloads Maestro and sets the same host-side `cwd`. This ensures `/host` inside the WASI sandbox maps back to your real home directory where `~/.config/maestro` lives.

3. **Reload/launch Zellij and press `Alt+m`**
   - The first invocation prompts for permissions (Read/Change application state, Open terminals/plugins, Run commands, etc.). Accept once.
   - Subsequent presses of `Alt+m` toggle Maestro as a floating pane focused on the current tab.

## Key Commands
- **Main pane list:** `↑/↓` select panes • `Enter` focus selected pane (Maestro auto-closes) • `d` kill pane • `n` new-pane wizard • `c` agent config • `Esc` close Maestro.
- **Agent Config overlay:** `↑/↓` move • `a` add • `e` edit • `d` delete (with confirmation) • `Esc` return.
- **New-pane wizard:**
  - Workspace step: type path, `Tab` accept suggestion, `↑/↓` move through suggestions, `Enter` confirm, `Esc` cancel.
  - Agent step: type to fuzzy-filter, `↑/↓` highlight match, `Enter` spawn pane, `Esc` cancel.
- **Agent form:** `↑/↓` cycle fields (Name → Command → Args → Note) • type to edit • `Enter` save • `Esc` cancel.
- **Delete confirm:** `Enter`/`y` delete • `Esc`/`n` cancel.

## Configuration
- Maestro persists agents to `~/.config/maestro/agents.kdl` on the host (seen as `/host/.config/maestro/agents.kdl` inside the plugin).
- Default agents (`cursor`, `claude`, `gemini`, `codex`) merge in at startup; only non-default entries are written back.
- Manage agents via the in-plugin UI to avoid malformed KDL.

Example entry:
```kdl
agent name="codex-review" note="Verbose reviewer" {
    cmd "codex"
    args "/review" "--verbose"
}
```

## Development
- Format, lint, and test before committing:
  ```sh
  cargo fmt
  cargo clippy --all-targets --all-features
  cargo test
  ```
- Release build (needed for Zellij): `cargo build --release --target wasm32-wasip1`
- Debugging: run `zellij --log-to-file true` and set `RUST_LOG=debug` before launching Maestro to capture plugin logs under `~/.cache/zellij/`.
