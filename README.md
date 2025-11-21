# Maestro (Zellij Plugin)

This repository is a fresh start for a Zellij-only plugin. The working spec lives at `docs/specs/maestro_zellij_plugin.md`.

## Dev workflow
- Toolchain: Rust stable, target `wasm32-wasi`.
- Build & reload inside Zellij: `cargo build --target wasm32-wasi && zellij action start-or-reload-plugin file:target/wasm32-wasi/debug/maestro.wasm`
- Example dev layout: see `docs/specs/dev_layout.md`.
