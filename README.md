# yubi-tray-rs

A lightweight Linux system tray application that monitors YubiKey connection status in real-time.

```text
┌─────────────────┐
│  YubiKey (HID)  │
└────────┬────────┘
         │ USB event detection (1s poll)
         ▼
┌─────────────────┐
│  yubi-tray-rs   │
└────────┬────────┘
         ├───> Tray Icon: Green (Connected) / Red (Disconnected)
         ├───> Tooltip: Product name, Serial Number, VID:PID
         └───> Desktop Notifications (libnotify / notify-rust)
```

---

## Features

- Real-time status indicator with green (connected) and red (disconnected) states
- Device details in tooltip with model name, serial number, and USB vendor/product IDs
- Desktop notifications via libnotify / D-Bus on connect and disconnect events
- Persistent configuration toggle for notification preferences
- Native Wayland and X11 system tray integration via AppIndicator / GTK

---

## Requirements

- Linux (GNOME, KDE Plasma, XFCE, Sway, or any desktop environment supporting AppIndicator)
- System packages: `gtk3-devel`, `libappindicator-gtk3`, `libxdo-devel`, `systemd-devel`

---

## Building & Installation

```bash
git clone -b linux https://github.com/9hb/yubi-tray-rs.git
cd yubi-tray-rs
cargo build --release
install -Dm755 target/release/yubikey-watch ~/.local/bin/yubi-tray-rs
```

---

## Usage

Start the background tray indicator:

```bash
yubi-tray-rs &
```

Right-click the tray icon to:
- Toggle desktop notifications
- Exit the application

Configuration is saved in `~/.config/yubi-tray-rs/config.txt`.
