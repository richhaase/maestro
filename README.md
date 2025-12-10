# Maestro

Keyboard-driven Zellij plugin for spawning and managing CLI-based AI coding agents (Claude, Cursor, etc.)

## Features

- Launch AI agents in dedicated panes from a floating overlay (`Alt+m`)
- Fuzzy workspace selection with path autocomplete
- Agent config management (add/edit/delete) persisted to `~/.config/maestro/agents.kdl`
- Session-aware pane tracking across Zellij reloads
- Default agents for Claude Code, Cursor, Gemini, and Codex

## Prerequisites

- **Rust** 1.70+ and **Cargo** 1.70+
- **Zellij** 0.43.1+ (terminal multiplexer)
- **wasm32-wasip1** target (`rustup target add wasm32-wasip1`)
- One or more CLI agents installed (e.g., `claude`, `cursor-agent`)

## Installation

1. **Clone and build**

   ```sh
   git clone https://github.com/richhaase/maestro.git
   cd maestro
   rustup target add wasm32-wasip1
   cargo build --release --target wasm32-wasip1
   ```

2. **Install plugin**

   ```sh
   mkdir -p ~/.config/zellij/plugins
   cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
   ```

3. **Configure Zellij** (`~/.config/zellij/config.kdl`)

   Add the keybinding (replace `/home/you` with your actual home directory):

   ```kdl
   keybinds {

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
   }
   ```

   Add the plugin alias:

   ```kdl
   plugins {

     ...

     maestro location="file:~/.config/zellij/plugins/maestro.wasm" {
       cwd "/home/you"
     }
   }
   ```

   The `cwd` must point to your host home directory so `/host` inside the WASI sandbox maps to where `~/.config/maestro` lives.

4. **Launch and test**
   ```sh
   zellij
   # Press Alt+m
   # Accept permissions prompt
   # Press Alt+m again to toggle Maestro
   ```

## Key Commands

| Context             | Key     | Action                                |
| ------------------- | ------- | ------------------------------------- |
| **Main pane list**  | `↑/↓`   | Select panes                          |
|                     | `Enter` | Focus pane (auto-closes Maestro)      |
|                     | `d`     | Kill selected pane                    |
|                     | `n`     | New-pane wizard                       |
|                     | `c`     | Agent config                          |
|                     | `Esc`   | Close Maestro                         |
| **Agent config**    | `↑/↓`   | Navigate agents                       |
|                     | `a`     | Add agent                             |
|                     | `e`     | Edit agent                            |
|                     | `d`     | Delete (with confirmation)            |
|                     | `Esc`   | Return to main                        |
| **New-pane wizard** | Type    | Enter/filter workspace path           |
| (Step 1: Workspace) | `↑/↓`   | Navigate workspace suggestions        |
|                     | `Tab`   | Accept workspace suggestion           |
|                     | `Enter` | Confirm workspace                     |
|                     | `Esc`   | Cancel                                |
| **New-pane wizard** | Type    | Filter agents                         |
| (Step 2: Agent)     | `↑/↓`   | Navigate agent matches                |
|                     | `Enter` | Spawn pane with selected agent        |
|                     | `Esc`   | Cancel                                |
| **Agent form**      | `↑/↓`   | Cycle fields (Name/Command/Args/Note) |
|                     | Type    | Edit field                            |
|                     | `Enter` | Save                                  |
|                     | `Esc`   | Cancel                                |

## Configuration

Agents are persisted to `~/.config/maestro/agents.kdl`. Default agents (`cursor`, `claude`, `gemini`, `codex`) are merged at startup. When you create, edit, or delete agents through the UI, the complete list (defaults + custom) is saved to preserve any customizations to built-in agents.

Example agent:

```kdl
agent name="codex-reviewer" note="Run codex review" {
    cmd "codex"
    args "review" "--base" "main"
}
```

Manage agents via the in-plugin UI to avoid malformed KDL.

## Development

Run before committing:

```sh
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

Build release WASM:

```sh
cargo build --release --target wasm32-wasip1
```

Debug logging:

```sh
zellij --log-to-file true
# Set RUST_LOG=debug before launching Maestro
# Logs appear under ~/.cache/zellij/
```

## License

MIT
