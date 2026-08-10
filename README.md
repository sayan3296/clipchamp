# clipchamp

Cross-platform network clipboard sync. Copy on one machine, paste on another.

A single Rust binary that syncs clipboard content (text, images, URLs) across Linux (Wayland), macOS, and Windows machines over TCP. One machine runs the server (hub), the rest connect as clients. Changes propagate in real time.

## Features

- Real-time clipboard sync over TCP with MessagePack framing
- Automatic server discovery via mDNS (`_clipchamp._tcp.local.`)
- Server-side clipboard history with persistence
- Content deduplication via BLAKE3 hashing
- Echo prevention (your own pastes don't bounce back)
- Text, image (PNG), and URL content types
- Optional TLS (behind `tls` feature flag)

## Installation

```sh
cargo install --path .
```

With TLS support:

```sh
cargo install --path . --features tls
```

## Quick Start

```sh
# Start the server (advertises via mDNS by default)
clipchamp server

# Connect a client (auto-discovers server via mDNS)
clipchamp client

# View clipboard history
clipchamp history

# Copy a history entry to the local clipboard
clipchamp history get 3

# Clear all history
clipchamp history clear
```

## CLI Reference

```
clipchamp server [--bind <addr>] [--no-mdns]
clipchamp client [--server <addr>]
clipchamp history [get <index> | clear]
```

| Flag | Description |
|------|-------------|
| `--bind <addr>` | Override the server listen address (default: `0.0.0.0:9090`) |
| `--no-mdns` | Disable mDNS advertisement |
| `--server <addr>` | Connect to a specific server instead of mDNS auto-discovery |

## Configuration

Configuration lives at `~/.config/clipchamp/config.toml`. A default file is generated on first run.

### Options

| Section | Key | Type | Default | Description |
|---------|-----|------|---------|-------------|
| `[server]` | `bind` | string | `"0.0.0.0:9090"` | Server listen address |
| `[server]` | `mdns` | bool | `true` | Advertise the server via mDNS |
| `[client]` | `server` | string | `"auto"` | Server address; `"auto"` uses mDNS discovery |
| `[clipboard]` | `max_size_mb` | integer | `10` | Maximum clipboard content size in MB. Content larger than this is dropped. |
| `[clipboard]` | `poll_interval_ms` | integer | `500` | How often to check for clipboard changes (ms) |
| `[protocol]` | `max_frame_size_mb` | integer | `16` | Maximum wire frame size in MB. Must be >= `max_size_mb`. |
| `[history]` | `max_entries` | integer | `100` | Maximum number of history entries to keep |
| `[history]` | `persist` | bool | `true` | Persist history to disk between restarts |
| `[tls]` | `enabled` | bool | `false` | Enable TLS (requires the `tls` feature) |
| `[tls]` | `cert` | string | `""` | Path to TLS certificate file |
| `[tls]` | `key` | string | `""` | Path to TLS private key file |
| `[tls]` | `ca` | string | `""` | Path to CA certificate file |

If `clipboard.max_size_mb` exceeds `protocol.max_frame_size_mb`, a warning is logged at startup. Raise `max_frame_size_mb` to at least match `max_size_mb`.

### Example config

```toml
[server]
bind = "0.0.0.0:9090"
mdns = true

[client]
server = "auto"

[clipboard]
max_size_mb = 10
poll_interval_ms = 500

[protocol]
max_frame_size_mb = 16

[history]
max_entries = 100
persist = true

[tls]
enabled = false
cert = ""
key = ""
ca = ""
```

## Architecture

The server acts as a hub: it accepts client connections, broadcasts clipboard updates to all connected clients, maintains clipboard history, and syncs its own clipboard with incoming changes. Clients watch the local clipboard and relay changes to the server.

The wire protocol is length-prefixed MessagePack over TCP (`[4-byte BE length][MessagePack payload]`). Content is deduplicated using BLAKE3 hashes. Server discovery uses mDNS service type `_clipchamp._tcp.local.`.

## Built With

This project was built in collaboration with [Claude Code](https://claude.ai/claude-code) (Claude Opus 4.6).

## License

MIT
