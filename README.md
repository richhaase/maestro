# Maestro (Zellij Plugin)

This repository is a fresh start for a Zellij-only plugin. The working spec lives at `docs/specs/maestro_zellij_plugin.md`.

## Dev workflow
- Toolchain: Rust stable
- Build plugin (WASM): `cargo build --target wasm32-wasip1`
- Build & reload inside Zellij: `cargo build --target wasm32-wasip1 && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/maestro.wasm`
- Run tests (native target): `cargo test --lib`
- Example dev layout: see `docs/specs/dev_layout.md`.
