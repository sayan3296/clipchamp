# clipchamp

Cross-platform network clipboard sync. Copy on one machine, paste on another.

A single Rust binary that syncs clipboard content (text, images, URLs) across Linux (Wayland), macOS, and Windows machines over TCP. One machine runs the server (hub), the rest connect as clients. Changes propagate in real time.

## Features

- Real-time clipboard sync over TCP with MessagePack framing
- Automatic server discovery via mDNS (`_clipchamp._tcp.local.`)
- Server-side clipboard history with persistence
- Content deduplication via BLAKE3 hashing
- Text, image (PNG), and URL content types
- Optional TLS and system tray icon (feature flags)
- Configurable file logging with log levels

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) 1.85+ (edition 2024)

**Linux / macOS:**

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Windows:** Download and run the installer from [rustup.rs](https://rustup.rs).

### Platform Dependencies

| Platform | Requirement | Notes |
|----------|-------------|-------|
| Linux | `wl-clipboard` | Runtime clipboard access on Wayland |
| macOS | Xcode Command Line Tools | `xcode-select --install` |
| Windows | MSVC Build Tools | [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) |

See [DEVEL.md](DEVEL.md) for additional dependencies when building with optional features (`--features gui`, `--features tls`).

## Quick Start

```sh
cargo install --path .

# Start the server
clipchamp server

# Connect a client (auto-discovers via mDNS)
clipchamp client

# View clipboard history
clipchamp history
```

## Architecture

The server acts as a hub: it accepts client connections, broadcasts clipboard updates to all connected clients, maintains clipboard history, and syncs its own clipboard with incoming changes. Clients watch the local clipboard and relay changes to the server.

The wire protocol is length-prefixed MessagePack over TCP (`[4-byte BE length][MessagePack payload]`). Content is deduplicated using BLAKE3 hashes. Server discovery uses mDNS service type `_clipchamp._tcp.local.`.

## Documentation

- **[CONFIG.md](CONFIG.md)** — CLI reference, configuration options, example config
- **[DEVEL.md](DEVEL.md)** — Building, feature flags, platform dependencies, testing

## Built With

This project was built in collaboration with [Claude Code](https://claude.ai/claude-code) (Claude Opus 4.6).

## License

MIT
