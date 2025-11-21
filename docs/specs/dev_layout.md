# Dev Layout & Hot Reload (Zellij)

Target: Zellij v0.43.1, Rust stable, `wasm32-wasi`.

## Build & reload command
Run this in a shell pane (rerun after edits):

```
cargo build --target wasm32-wasip1 && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/maestro.wasm
```

## Example dev layout (KDL)

```
layout {
  tab name="dev" {
    pane size=1 borderless=true {                          # status/logs
      command "bash"
      args "-lc" "tail -f zellij-dev.log"
    }
    pane command="bash" {                                  # build/reload shell
      args "-lc" "cargo build --target wasm32-wasip1 && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/maestro.wasm | tee zellij-dev.log"
    }
    pane edit="src/lib.rs"                                 # editor
    pane { plugin location=\"file:target/wasm32-wasip1/debug/maestro.wasm\" }   # live plugin
  }
}
```

- Re-run the build/reload command after changes (or wrap with `watchexec`/`cargo watch` if you prefer).
- The `tail` pane is optional; it just surfaces build/reload logs.
