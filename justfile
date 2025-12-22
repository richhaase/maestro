default:
    @just --list

# Build the WASM plugin (debug)
build:
    cargo build --target wasm32-wasip1

# Build the WASM plugin (release)
build-release:
    cargo build --release --target wasm32-wasip1

# Run tests
test:
    cargo test

# Run clippy and fmt check
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

# Format code
fmt:
    cargo fmt

# Build release and install to Zellij plugins directory
install: build-release
    mkdir -p ~/.config/zellij/plugins
    cp target/wasm32-wasip1/release/maestro.wasm ~/.config/zellij/plugins/

# Uninstall from Zellij plugins directory
uninstall:
    rm -f ~/.config/zellij/plugins/maestro.wasm
