# Configuration

## CLI Reference

```
clipchamp server [--bind <addr>] [--no-mdns]
clipchamp client [--server <addr>]
clipchamp history [get <index> | clear]
clipchamp tray --mode <server|client> [--bind <addr>] [--no-mdns] [--server <addr>]
```

| Flag | Description |
|------|-------------|
| `--bind <addr>` | Override the server listen address (default: `0.0.0.0:9090`) |
| `--no-mdns` | Disable mDNS advertisement |
| `--server <addr>` | Connect to a specific server instead of mDNS auto-discovery |
| `--mode <server\|client>` | Run as server or client (tray subcommand only) |

The `tray` subcommand requires building with `--features gui`. It shows a system tray icon with status and a Quit option. When Quit is selected, the background process stops gracefully.

## Config File

Configuration lives at `~/.config/clipchamp/config.toml`. A default file is generated on first run.

### Options

| Section | Key | Type | Default | Description |
|---------|-----|------|---------|-------------|
| `[server]` | `bind` | string | `"0.0.0.0:9090"` | Server listen address |
| `[server]` | `mdns` | bool | `true` | Advertise the server via mDNS |
| `[client]` | `server` | string | `"auto"` | Server address; `"auto"` uses mDNS discovery |
| `[clipboard]` | `max_size_mb` | integer | `10` | Maximum clipboard content size in MB |
| `[clipboard]` | `poll_interval_ms` | integer | `500` | How often to check for clipboard changes (ms) |
| `[protocol]` | `max_frame_size_mb` | integer | `16` | Maximum wire frame size in MB. Must be >= `max_size_mb` |
| `[history]` | `max_entries` | integer | `100` | Maximum number of history entries to keep |
| `[history]` | `persist` | bool | `true` | Persist history to disk between restarts |
| `[tls]` | `enabled` | bool | `false` | Enable TLS (requires the `tls` feature) |
| `[tls]` | `cert` | string | `""` | Path to TLS certificate file |
| `[tls]` | `key` | string | `""` | Path to TLS private key file |
| `[tls]` | `ca` | string | `""` | Path to CA certificate file |
| `[logging]` | `file` | string or null | `"/var/log/clipchamp/clipchamp.log"` | Log file path; set to `null` to disable file logging |
| `[logging]` | `level` | string | `"info"` | Log level: `trace`, `debug`, `info`, `warn`, or `error` |

If `clipboard.max_size_mb` exceeds `protocol.max_frame_size_mb`, a warning is logged at startup. Raise `max_frame_size_mb` to at least match `max_size_mb`.

### Logging

The default log directory `/var/log/clipchamp/` requires root to create. If it doesn't exist, clipchamp prints a hint to stderr and continues with stdout-only logging:

```sh
sudo mkdir -p /var/log/clipchamp && sudo chown $USER /var/log/clipchamp
```

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

[logging]
file = "/var/log/clipchamp/clipchamp.log"
level = "info"
```
