# Xiaozhi Linux Core (Rust Version)

This is a Rust rewrite of the `control_center` for the Xiaozhi Linux project. It replaces the original C++ implementation with a memory-safe, async, and easier-to-cross-compile version.

## Architecture

- **EventHub**: Main event loop handling state transitions.
- **NetLink**: WebSocket client using `tokio-tungstenite` and `rustls` (no OpenSSL dependency).
- **AudioBridge**: UDP communication with `sound_app`.
- **GuiBridge**: UDP communication with the GUI.

## Prerequisites

- Rust (latest stable)
- `pkg-config` (optional, usually not needed for pure Rust deps)

## Build

```bash
cargo build --release
```

## Cross Compilation

To cross-compile for ARM Linux (e.g., for the target board):

```bash
# Install target
rustup target add aarch64-unknown-linux-musl

# Build statically linked binary
cargo build --release --target aarch64-unknown-linux-musl
```

## Configuration

Configuration is currently handled in `src/config.rs`. You can modify the default values there or extend it to load from a file.

## Running

```bash
./target/release/xiaozhi_linux_core
```

Ensure `sound_app` and the GUI are running and listening on the expected UDP ports.
