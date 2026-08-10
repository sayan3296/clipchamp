# Development

## Building

```sh
cargo build --release
```

### Feature Flags

| Flag | Description |
|------|-------------|
| `tls` | TLS transport via rustls |
| `gui` | System tray icon via tray-icon/muda |

```sh
# Default (no optional features)
cargo install --path .

# With TLS
cargo install --path . --features tls

# With system tray
cargo install --path . --features gui

# All features
cargo install --path . --features tls,gui
```

### Platform Dependencies

**System tray (`gui` feature) on Linux** requires GTK3 development libraries:

```sh
# Fedora/RHEL
sudo dnf install gtk3-devel libappindicator-gtk3-devel libxdo-devel

# Debian/Ubuntu
sudo apt install libgtk-3-dev libayatana-appindicator3-dev libxdo-dev
```

macOS and Windows need no extra system packages.

## Testing

```sh
cargo test
```

## Verification

```sh
# Compiles without optional features
cargo check

# Compiles with tray support
cargo check --features gui

# Compiles with TLS
cargo check --features tls
```
