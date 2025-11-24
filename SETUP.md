# Setup

## Prerequisites
- Zellij 0.43.x (Maestro links against `zellij-tile 0.43.1`).
- Rust toolchain (1.75+ recommended) with `cargo`.
- WASI target: `rustup target add wasm32-wasip1`.
- A launcher location for plugins, typically `~/.config/zellij/plugins/`.

## Install Steps
1. Clone and enter this repo.
2. Build the WASM artifact:
   ```
   cargo build --release --target wasm32-wasip1
   ```
3. Copy the resulting plugin:
   ```
   mkdir -p ~/.config/zellij/plugins
   cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
   ```
4. Reference the plugin from Zellij:
   - ad-hoc: `zellij action start-plugin file:~/.config/zellij/plugins/maestro.wasm`
   - layout snippet (set the `cwd` to your home directory so `/host` mappings stay consistent):
     ```
     plugins {
         maestro location="file:~/.config/zellij/plugins/maestro.wasm" {
             cwd "/Users/you"
         }
     }
     ```
   - Always install the optimized `--release` build copied from `target/wasm32-wasip1/release/`.

## Environment Configuration
- Agent config file: `~/.config/maestro/agents.kdl`. The plugin creates it automatically when you save from the Agent Config UI.
- Ensure `~/.config/maestro/` is writable; Zellij runs plugins in a sandbox where `/host` maps to your actual `$HOME`.

## Troubleshooting
- **Missing WASI target**: run `rustup target add wasm32-wasip1`.
- **Plugin won’t launch**: check Zellij logs (`zellij --log-to-file true`) for permission prompts; Maestro requests `ReadApplicationState`, `ChangeApplicationState`, `OpenTerminalsOrPlugins`, `RunCommands`, and `FullHdAccess`.
- **Agents not showing**: confirm `~/.config/maestro/agents.kdl` is valid KDL; use the in-plugin editor to avoid syntax errors.
- **Pane focus errors**: ensure Zellij session has granted the requested permissions; re-open the plugin if denied.

## Verification
1. Start Zellij and load Maestro.
2. Press `n` to open the wizard, enter a workspace path, choose or create a tab, select an agent, and confirm a command pane launches.
3. Hit `Tab` to switch to Agent Config, `a` to create a throwaway agent, and verify it round-trips to `~/.config/maestro/agents.kdl`.
4. Run `cargo test` locally to ensure unit tests pass after changes.
