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

**System tray (`gui` feature)** requires GTK3 development libraries:

| Platform | Packages |
|----------|----------|
| Fedora/RHEL | `sudo dnf install gtk3-devel libappindicator-gtk3-devel libxdo-devel` |
| Debian/Ubuntu | `sudo apt install libgtk-3-dev libayatana-appindicator3-dev libxdo-dev` |
| macOS | `brew install gtk+3 pkg-config` |
| Windows | MSYS2 UCRT64: `pacman -S mingw-w64-ucrt-x86_64-gtk3 mingw-w64-ucrt-x86_64-pkg-config mingw-w64-ucrt-x86_64-rust` |

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

## CI

CI runs on every push and pull request to `master` via `.github/workflows/ci.yml`:

- **Linux:** `cargo check`, `cargo check --features gui`, `cargo test`, `cargo clippy`
- **macOS:** `cargo check --features gui` (GTK3 via Homebrew)
- **Windows:** `cargo check --features gui` (GTK3 via MSYS2/UCRT64)

## Release Process

Releases are automated via `.github/workflows/release.yml`. To create a release:

1. Update the `VERSION` file at the repo root (format: `x.y` or `x.y.z`, e.g. `1.1`)
2. Commit with the exact message: `Release the newest version`
3. Push to `master`

The workflow will:
- Compare `VERSION` against existing GitHub releases
- Skip if a release for that version already exists
- Build for Linux (x86_64 with GUI, RPM + tar.gz), macOS (x86_64 + aarch64 with GUI), and Windows (x86_64 with GUI, bundled DLLs)
- Create a GitHub Release tagged `v{version}` with all artifacts

### Release Artifacts

| Platform | Artifact | Notes |
|----------|----------|-------|
| Linux | `.rpm`, `.tar.gz` | RPM declares `gtk3` and `wl-clipboard` as dependencies |
| macOS Intel | `.tar.gz` | Runtime: `brew install gtk+3` for tray support |
| macOS ARM | `.tar.gz` | Runtime: `brew install gtk+3` for tray support |
| Windows | `.zip` | GTK3 DLLs bundled in the archive |
