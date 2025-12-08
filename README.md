# Maestro

Maestro is a Zellij plugin that launches and manages "agent" command panes entirely from the keyboard.

## Quick Start

1. Install prerequisites: `zellij` ≥ 0.43, `rustup`, `cargo`, and a WASI runtime such as `wasmtime`.
2. Enable the WASI target (once): `rustup target add wasm32-wasip1`
3. Build the plugin: `cargo build --release --target wasm32-wasip1`
4. Copy the WASM artifact into Zellij’s plugin directory:
   ```
   mkdir -p ~/.config/zellij/plugins
   cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
   ```
5. Launch from an existing session:

   ```
   zellij action start-plugin file:~/.config/zellij/plugins/maestro.wasm
   ```

   or with a hot-key:

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

   or embed it in a layout:

   ```
   plugins {
       maestro location="file:~/.config/zellij/plugins/maestro.wasm" {
           cwd "/Users/you"  # map /host back to your home directory
       }
   }
   ```

## Key Commands

- `Tab` — toggle focus between the Agent Pane list and the Agent Config list.
- `↑` / `↓` — move within whichever pane is focused.
- `Enter` — focus the selected running pane or advance wizard steps.
- `n` — open the “new agent pane” wizard (workspace → tab → agent).
- `f` — toggle filter mode on the Agent Pane list and type to fuzzy-match.
- `x` — terminate the selected pane.
- `a` / `e` / `d` — add, edit, or delete agents (with confirmation).
- Agent config form order: Name → Command → Args → Note.

## Installation

1. Clone this repo and build the release WASM:
   ```
   git clone https://github.com/<owner>/maestro.git
   cd maestro
   rustup target add wasm32-wasip1
   cargo build --release --target wasm32-wasip1
   ```
2. Copy `target/wasm32-wasip1/release/maestro.wasm` into `~/.config/zellij/plugins/`.
3. Start or reload Zellij and use `zellij action start-plugin file:~/.config/zellij/plugins/maestro.wasm` (or reference it from a layout as shown above). Always load the optimized `--release` build for fast startup.

## Configuration

- Agent definitions live at `~/.config/maestro/agents.kdl`.
- On startup Maestro merges that file with built-in agents (`cursor`, `claude`, `gemini`, `codex`) and deduplicates by name. Only non-default agents are written back.
- Example entry:
  ```
  agent name="my-agent" note="Internal helper" {
      cmd "my-agent-cli" "--api"
      args "--review" "--debug"
  }
  ```
- `cmd` captures the executable (and any inline args); optional `args` entries append additional flags. Notes appear in the Agent Config table. Per-agent environment variables are not supported—set them via your shell or launcher instead.
- Use the in-plugin Agent Config UI to avoid malformed KDL; Maestro creates `~/.config/maestro/agents.kdl` on first save.

## Development

- Run unit tests before committing: `cargo test`
- Lint logic-heavy changes: `cargo clippy --all-targets`
- When debugging inside Zellij, enable logs with `zellij --log-to-file true` and set `RUST_LOG=debug`.

## Releasing

Maestro uses [`cargo-release`](https://github.com/crate-ci/cargo-release).

```
cargo install cargo-release                 # once
cargo release patch --dry-run               # preview
cargo release patch --execute               # bump version, tag, push
```

Tagging `v*` kicks off the GitHub Actions workflow that builds and publishes the WASM artifact.
