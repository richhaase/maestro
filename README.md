# Maestro (Zellij Plugin)

A Zellij plugin for managing and launching AI coding agents.

## Dev workflow
- Toolchain: Rust stable
- Build plugin (WASM): `cargo build --target wasm32-wasip1`
- Build & reload inside Zellij: `cargo build --target wasm32-wasip1 && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/maestro.wasm`
- Run tests (native target): `cargo test --lib`
