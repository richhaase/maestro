# Setup

## Prerequisites

- Zellij 0.43.x (Maestro links against `zellij-tile 0.43.1`).
- Rust toolchain with `cargo` (Rust 1.75+ recommended) and `rustup`.
- WASI target install: `rustup target add wasm32-wasip1`.
- WASI runtime such as `wasmtime` (required by Zellij when loading WASM plugins).
- Writable plugin directory, typically `~/.config/zellij/plugins/`.

## Install Steps

1. Clone and enter the repo:
   ```
   git clone https://github.com/<owner>/maestro.git
   cd maestro
   ```
2. Build the WASM artifact:
   ```
   cargo build --release --target wasm32-wasip1
   ```
3. Install the plugin for Zellij:
   ```
   mkdir -p ~/.config/zellij/plugins
   cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
   ```
4. Load the plugin:
   - Ad-hoc: `zellij action start-plugin file:~/.config/zellij/plugins/maestro.wasm`
   - Hot-key snippet:

     ```
     shared_among "normal" "locked" {

       ...

       bind "Alt m" {
         LaunchOrFocusPlugin "file:~/.config/zellij/plugins/maestro.wasm" {
           floating true
           move_to_focused_tab true
           cwd "/Users/you"   # ensure /host maps back to $HOME
         }
       }
     }
     ```

     Always copy the optimized release artifact; debug builds are large and slow.

## Environment & Permissions

- Maestro stores user agents at `~/.config/maestro/agents.kdl`; the file is created/updated when you save from the Agent Config UI.
- Inside Zellij, `/host` maps to your actual `$HOME`. Ensure `~/.config/maestro/` exists and is writable.
- When prompted, grant Maestro the requested permissions: `ReadApplicationState`, `ChangeApplicationState`, `OpenTerminalsOrPlugins`, `RunCommands`, and `FullHdAccess`. Denying them prevents pane creation/focus.

## Troubleshooting

- **Missing WASI target** → rerun `rustup target add wasm32-wasip1`.
- **Plugin fails to launch** → start Zellij with logging (`zellij --log-to-file true`) and check for denied permissions; reopen the plugin after granting access.
- **Agents don’t appear** → validate `~/.config/maestro/agents.kdl` (use the in-plugin editor to avoid malformed KDL) and ensure default agents weren’t accidentally deleted.
- **Pane focuses wrong directory** → set the plugin’s `cwd` to your home directory in layouts so `/host` resolves correctly.

## Verification

1. Start Zellij, run Maestro, and press `n` to run through the workspace → tab → agent wizard; confirm a new pane opens in the requested workspace.
2. Hit `Tab`, press `a`, and create a temporary agent. Confirm it shows up in the list and is written to `~/.config/maestro/agents.kdl`.
3. Run the test suite locally: `cargo test`.
4. If you maintain a layout file, reload it and verify the plugin starts without additional prompts.
