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

## Installation

### Pre-built Binaries

Download the latest release from [GitHub Releases](../../releases).

**Linux (Fedora/RHEL):**

```sh
sudo dnf install ./clipchamp-*-1.x86_64.rpm
```

**macOS:**

```sh
# Choose the archive matching your architecture (x86_64 or aarch64)
tar xzf clipchamp-*-macos-*.tar.gz
sudo mv clipchamp /usr/local/bin/
# Tray feature requires GTK3: brew install gtk+3
```

**Windows:**

Extract `clipchamp-*-windows-x86_64.zip` to a folder and run `clipchamp.exe`. GTK3 DLLs are bundled.

### Build from Source

Requires [Rust](https://www.rust-lang.org/tools/install) 1.85+ (edition 2024).

```sh
cargo install --path .
```

See [DEVEL.md](DEVEL.md) for feature flags (`--features gui`, `--features tls`) and platform-specific build dependencies.

## Quick Start

```sh
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
