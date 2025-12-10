# Setup Guide

Complete setup instructions for Maestro.

## Prerequisites

| Requirement | Version | Check Command      |
| ----------- | ------- | ------------------ |
| **Rust**    | 1.70+   | `rustc --version`  |
| **Cargo**   | 1.70+   | `cargo --version`  |
| **Zellij**  | 0.43.1+ | `zellij --version` |
| **Git**     | Any     | `git --version`    |

### Install Prerequisites

**Rust & Cargo** (via rustup):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**Zellij** (choose one):

```sh
# macOS (Homebrew)
brew install zellij

# Linux (cargo)
cargo install --locked zellij

# Arch Linux
pacman -S zellij

# See https://zellij.dev/documentation/installation for other options
```

**WASM target**:

```sh
rustup target add wasm32-wasip1
```

## Installation Steps

### 1. Clone Repository

```sh
git clone https://github.com/richhaase/maestro.git
cd maestro
```

### 2. Build WASM Plugin

```sh
cargo build --release --target wasm32-wasip1
```

Expected output: `target/wasm32-wasip1/release/maestro.wasm`

### 3. Install Plugin

```sh
mkdir -p ~/.config/zellij/plugins
cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
```

### 4. Configure Zellij

**Locate your home directory**:

```sh
echo $HOME
# Example output: /home/username or /Users/username
```

**Edit `~/.config/zellij/config.kdl`** and add:

```kdl
keybinds {

  ...

  shared_among "normal" "locked" {

    ...

    bind "Alt m" {
      LaunchOrFocusPlugin "file:~/.config/zellij/plugins/maestro.wasm" {
        floating true
        move_to_focused_tab true
        cwd "/home/username"  // ← Replace with your $HOME
      }
    }
  }
}

plugins {

  ...

  maestro location="file:~/.config/zellij/plugins/maestro.wasm" {
    cwd "/home/username"  // ← Replace with your $HOME
  }
}
```

**Important**: The `cwd` value must be your **actual home directory path** (not `~`). The WASI sandbox maps `/host` to this path, allowing Maestro to launch agents in any subdirectory of your home and read/write `~/.config/maestro/agents.kdl`.

## Verification

### 1. Launch Zellij

```sh
zellij
```

### 2. Open Maestro

Press `Alt+m`

**First run**: Accept the permissions prompt for:

- Read/Change application state
- Open files
- Full filesystem access
- Run commands
- Open terminals/plugins

**Expected behavior**: Floating pane appears with the Maestro UI

### 3. Test Pane Creation

- Press `n` (new pane wizard)
- Type a workspace path (e.g., `src/maestro`)
- Press `Enter`
- Select an agent (or press `Esc` to cancel)

### 4. Verify Config Persistence

```sh
ls ~/.config/maestro/
```

**Note**: The `agents.kdl` file is created only when you create, edit, or delete an agent through the Maestro UI. On first launch, the default agents (`cursor`, `claude`, `gemini`, `codex`) are available in memory but not yet written to disk.

## Troubleshooting

### Plugin doesn't load / `Alt+m` does nothing

**Check plugin exists**:

```sh
ls -lh ~/.config/zellij/plugins/maestro.wasm
```

**Check Zellij config syntax**:

```sh
zellij setup --check
```

**View Zellij logs**:

```sh
zellij --log-to-file true
tail -f ~/.cache/zellij/zellij.log
```

### Permission denied errors

**Verify `cwd` path in config**:

```kdl
# Must be absolute path, not ~
cwd "/home/username"  # ✓ Correct
cwd "~"               # ✗ Wrong
```

**Check directory permissions**:

```sh
ls -ld ~/.config/maestro
# Should be writable by your user
```

### Agents don't appear or fail to launch

**Verify agent is installed**:

```sh
which claude        # For Claude Code
which cursor-agent  # For Cursor
which gemini        # For Gemini
which codex         # For Codex
```

**Check agent config**:

```sh
cat ~/.config/maestro/agents.kdl
```

**Test agent manually**:

```sh
cd /home/username/projects/some-workspace
claude  # Should launch successfully
```

### Config file is malformed

**Backup and regenerate**:

```sh
mv ~/.config/maestro/agents.kdl ~/.config/maestro/agents.kdl.backup
# Restart Maestro to regenerate defaults
```

**Always edit via Maestro UI** (`Alt+m` → `c`) to avoid KDL syntax errors.

### Build fails with "target not found"

**Install WASM target**:

```sh
rustup target add wasm32-wasip1
rustup target list --installed | grep wasm32-wasip1
```

### Debug mode for development

**Enable verbose logging**:

```sh
RUST_LOG=debug zellij --log-to-file true
```

**View plugin logs**:

```sh
tail -f ~/.cache/zellij/zellij.log | grep maestro
```

## Updating Maestro

```sh
cd maestro
git pull origin main
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/
# Restart Zellij or reload plugins
```

## Uninstalling

```sh
rm ~/.config/zellij/plugins/maestro.wasm
rm -rf ~/.config/maestro
# Remove Maestro config from ~/.config/zellij/config.kdl
```
