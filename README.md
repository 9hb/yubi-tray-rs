# yubi-tray-rs

## What it does

Shows a colored icon in your system tray - green when YubiKey is connected, red when it's not. Hover over the icon to see device details.

## Building

```bash
cargo build --release
```

The compiled binary will be in `target/release/yubi-tray-rs.exe`.

## Usage

Just run the executable. It'll sit in your tray and do its thing. Right-click and select Exit when you want to close it.

## Requirements

- Windows
- Rust (for building)

## License

[Apache 2.0](LICENSE)
