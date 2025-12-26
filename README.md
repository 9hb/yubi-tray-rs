# yubi-tray-rs

## What it does

Shows a colored icon in your system tray - green when YubiKey is connected, red when it's not. Hover over the icon to see device details.

## Features

- **Tray icon** - Green when connected, red when disconnected
- **Tooltip** - Shows device name, serial number and vendor/product ID
- **Notifications** - Optional Windows toast notifications when YubiKey is connected/disconnected
- **Persistent settings** - Your notification preference is saved and restored after restart

## Building

```bash
cargo build --release
```

The compiled binary will be in `target/release/yubi-tray-rs.exe`.

## Usage

Just run the executable. It'll sit in your tray and do its thing.

Right-click the tray icon to:

- **Notifications** - Toggle connect/disconnect notifications (setting is saved)
- **Exit** - Close the application

## Configuration

Settings are stored in `%APPDATA%\yubi-tray-rs\config.txt`.

## Requirements

- Windows
- Rust (for building)

## License

[Apache 2.0](LICENSE)
