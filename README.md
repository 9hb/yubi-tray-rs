# yubi-tray-rs

A lightweight Linux system tray application that monitors YubiKey connection status in real-time.

<div align="center">
  <img src="assets/menu.png" alt="Context Menu" width="220" />
  &nbsp;&nbsp;&nbsp;&nbsp;
  <img src="assets/notification.png" alt="Desktop Notification" width="460" />
</div>

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
         ├───> Context Menu: Model name & VID:PID (e.g. /dev/hidraw0)
         └───> Desktop Notifications (libnotify / notify-rust)
```

---

## Features

- Real-time status indicator with green (connected) and red (disconnected) states
- Quick device inspection in context menu showing model name and USB VID:PID path
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
