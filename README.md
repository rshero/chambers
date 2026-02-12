# Chambers

A fast, native database client built with Rust and GPUI.

## Features

- Native desktop performance with GPU-accelerated rendering
- Support for PostgreSQL, MongoDB, Redis, MySQL, SQLite
- Clean, minimal interface with dark theme
- UI scaling (Ctrl+=/-)
- Table view with sorting, filtering, pagination
- Connection management with secure credential storage

## Build

```bash
# Default build (PostgreSQL, MongoDB, Redis)
cargo build --release

# Full build (all database drivers)
cargo build --release --features full
```

## Run

```bash
./target/release/chambers
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl+= | Zoom in |
| Ctrl+- | Zoom out |
| Ctrl+0 | Reset zoom |

## Requirements

- Rust 1.75+
- Linux (X11/Wayland), macOS, or Windows

## License

MIT
