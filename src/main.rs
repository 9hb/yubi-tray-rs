#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use hidapi::HidApi;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, CheckMenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
use notify_rust::Notification;

#[cfg(target_os = "windows")]
use winrt_notification::{Duration as WinrtDuration, Toast};

#[derive(Debug, Clone)]
struct YubiKeyDetails {
    product_name: String,
    serial_number: Option<String>,
    vid: u16,
    pid: u16,
    hid_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    #[serde(default = "default_true")]
    pub enable: bool,
    #[serde(default = "default_true")]
    pub on_connect: bool,
    #[serde(default = "default_true")]
    pub on_disconnect: bool,
    #[serde(default)]
    pub sound: String,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enable: true,
            on_connect: true,
            on_disconnect: true,
            sound: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomMessagesConfig {
    #[serde(default = "default_connect_msg")]
    pub on_connect: String,
    #[serde(default = "default_disconnect_msg")]
    pub on_disconnect: String,
}

fn default_true() -> bool {
    true
}

fn default_connect_msg() -> String {
    "YubiKey has been connected".to_string()
}

fn default_disconnect_msg() -> String {
    "YubiKey has been disconnected".to_string()
}

impl Default for CustomMessagesConfig {
    fn default() -> Self {
        Self {
            on_connect: default_connect_msg(),
            on_disconnect: default_disconnect_msg(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub notification: NotificationConfig,
    #[serde(default, alias = "custom messages")]
    pub custom_messages: CustomMessagesConfig,
}

#[derive(Debug)]
enum UserEvent {
    YubiKeyUpdate(Option<YubiKeyDetails>),
    ConfigUpdate(AppConfig),
}

const YUBICO_VENDOR_ID: u16 = 0x1050;

fn get_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "yubi-tray-rs").map(|dirs| dirs.config_dir().join("config.toml"))
}

fn load_config() -> AppConfig {
    if let Some(path) = get_config_path() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<AppConfig>(&content) {
                return cfg;
            }
        }
    }
    let default_cfg = AppConfig::default();
    save_config(&default_cfg);
    default_cfg
}

fn save_config(cfg: &AppConfig) {
    if let Some(path) = get_config_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let content = format!(
            "[notification]\nenable = {}\non_connect = {}\non_disconnect = {}\nsound = \"{}\" # leave empty for no sound\n\n[custom_messages]\non_connect = \"{}\"\non_disconnect = \"{}\"\n",
            cfg.notification.enable,
            cfg.notification.on_connect,
            cfg.notification.on_disconnect,
            cfg.notification.sound,
            cfg.custom_messages.on_connect,
            cfg.custom_messages.on_disconnect
        );

        let _ = fs::write(path, content);
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    unsafe {
        unsafe extern "C" fn dummy_log_handler(
            _log_domain: *const std::os::raw::c_char,
            _log_level: glib_sys::GLogLevelFlags,
            _message: *const std::os::raw::c_char,
            _user_data: *mut std::os::raw::c_void,
        ) {}

        use std::ffi::CString;
        if let Ok(domain) = CString::new("libayatana-appindicator") {
            let flags = glib_sys::G_LOG_LEVEL_WARNING
                | glib_sys::G_LOG_LEVEL_MESSAGE
                | glib_sys::G_LOG_LEVEL_INFO
                | glib_sys::G_LOG_LEVEL_DEBUG;
            let _ = glib_sys::g_log_set_handler(domain.as_ptr(), flags, Some(dummy_log_handler), std::ptr::null_mut());
        }
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let initial_config = load_config();
    let config = Arc::new(Mutex::new(initial_config.clone()));

    let initial_device = get_yubikey_details();
    let is_connected = initial_device.is_some();

    let (dev_name, dev_hid) = format_menu_strings(&initial_device);

    let tray_menu = Menu::new();
    let notif_toggle = CheckMenuItem::new("Notifications", true, initial_config.notification.enable, None);
    let sep1 = PredefinedMenuItem::separator();

    // Unselectable / read-only display items
    let info_name = MenuItem::new(&dev_name, false, None);
    let info_hid = MenuItem::new(&dev_hid, false, None);

    let sep2 = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Exit", true, None);

    let _ = tray_menu.append(&notif_toggle);
    let _ = tray_menu.append(&sep1);
    let _ = tray_menu.append(&info_name);
    let _ = tray_menu.append(&info_hid);
    let _ = tray_menu.append(&sep2);
    let _ = tray_menu.append(&quit_item);

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    let initial_icon = if is_connected {
        generate_icon(0, 230, 118) // Sytá zářivá zelená (#00E676)
    } else {
        generate_icon(231, 76, 60) // Červená
    };

    let initial_tooltip = if let Some(ref d) = initial_device {
        format_tooltip(d)
    } else {
        "Connect a YubiKey".to_string()
    };

    let tray_icon = TrayIconBuilder::new()
        .with_tooltip(initial_tooltip)
        .with_icon(initial_icon)
        .with_menu(Box::new(tray_menu))
        .build()
        .expect("Failed to build tray icon");

    // Detection thread for device presence (1s poll)
    let proxy_presence = proxy.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(1000));
            let dev = get_yubikey_details();
            let _ = proxy_presence.send_event(UserEvent::YubiKeyUpdate(dev));
        }
    });

    // Config file watcher thread for real-time menu sync when edited manually
    let proxy_config = proxy.clone();
    thread::spawn(move || {
        let mut last_modified = None;
        loop {
            thread::sleep(Duration::from_millis(500));
            if let Some(path) = get_config_path() {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if last_modified != Some(modified) {
                            last_modified = Some(modified);
                            let cfg = load_config();
                            let _ = proxy_config.send_event(UserEvent::ConfigUpdate(cfg));
                        }
                    }
                }
            }
        }
    });

    let mut previous_state: Option<bool> = Some(is_connected);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + Duration::from_millis(200)
        );

        if let Ok(_tray_event) = tray_channel.try_recv() {}

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_item.id() {
                *control_flow = ControlFlow::Exit;
                return;
            }
            if event.id == notif_toggle.id() {
                let mut current_cfg = load_config();
                current_cfg.notification.enable = !current_cfg.notification.enable;
                let _ = notif_toggle.set_checked(current_cfg.notification.enable);
                save_config(&current_cfg);
                let mut cfg_guard = config.lock().unwrap();
                *cfg_guard = current_cfg;
            }
        }

        match event {
            tao::event::Event::UserEvent(UserEvent::ConfigUpdate(new_cfg)) => {
                let mut cfg_guard = config.lock().unwrap();
                *cfg_guard = new_cfg.clone();
                let _ = notif_toggle.set_checked(new_cfg.notification.enable);
            }
            tao::event::Event::UserEvent(UserEvent::YubiKeyUpdate(maybe_dev)) => {
                let currently_connected = maybe_dev.is_some();

                if let Some(was_connected) = previous_state {
                    if currently_connected != was_connected {
                        let current_cfg = load_config();
                        {
                            let mut cfg_guard = config.lock().unwrap();
                            *cfg_guard = current_cfg.clone();
                            let _ = notif_toggle.set_checked(current_cfg.notification.enable);
                        }

                        if current_cfg.notification.enable {
                            let should_notify = if currently_connected {
                                current_cfg.notification.on_connect
                            } else {
                                current_cfg.notification.on_disconnect
                            };

                            if should_notify {
                                show_connection_notification(currently_connected, &current_cfg);
                            }
                        }
                    }
                }

                previous_state = Some(currently_connected);

                let (name, hid) = format_menu_strings(&maybe_dev);
                let _ = info_name.set_text(name);
                let _ = info_hid.set_text(hid);

                match maybe_dev {
                    Some(ref d) => {
                        let _ = tray_icon.set_tooltip(Some(format_tooltip(d)));
                        let _ = tray_icon.set_icon(Some(generate_icon(0, 230, 118)));
                    }
                    None => {
                        let _ = tray_icon.set_tooltip(Some("Connect a YubiKey".to_string()));
                        let _ = tray_icon.set_icon(Some(generate_icon(231, 76, 60)));
                    }
                }
            }
            _ => {}
        }
    });
}

fn format_menu_strings(device: &Option<YubiKeyDetails>) -> (String, String) {
    if let Some(d) = device {
        (
            d.product_name.clone(),
            format!("{:04x}:{:04x} ({})", d.vid, d.pid, d.hid_path),
        )
    } else {
        (
            "Disconnected".to_string(),
            "No device".to_string(),
        )
    }
}

fn format_tooltip(d: &YubiKeyDetails) -> String {
    if let Some(ref s) = d.serial_number {
        format!("{}\nS/N: {}\nID: {:04x}:{:04x}", d.product_name, s, d.vid, d.pid)
    } else {
        format!("{}\nID: {:04x}:{:04x}", d.product_name, d.vid, d.pid)
    }
}

fn get_yubikey_details() -> Option<YubiKeyDetails> {
    match HidApi::new() {
        Ok(api) => {
            for device in api.device_list() {
                if device.vendor_id() == YUBICO_VENDOR_ID {
                    let product = device.product_string().unwrap_or("YubiKey").to_string();
                    let serial = device.serial_number().filter(|s| !s.is_empty()).map(|s| s.to_string());
                    let path_str = device.path().to_str().unwrap_or("hidraw0").to_string();

                    return Some(YubiKeyDetails {
                        product_name: product,
                        serial_number: serial,
                        vid: device.vendor_id(),
                        pid: device.product_id(),
                        hid_path: path_str,
                    });
                }
            }
            None
        }
        Err(_) => None,
    }
}

fn generate_icon(r: u8, g: u8, b: u8) -> Icon {
    let width = 64;
    let height = 64;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let dx = (x as i32) - (width as i32) / 2;
            let dy = (y as i32) - (height as i32) / 2;
            let distance = ((dx * dx + dy * dy) as f64).sqrt();

            if distance < (width as f64) / 2.0 - 4.0 {
                rgba.push(r);
                rgba.push(g);
                rgba.push(b);
                rgba.push(255);
            } else {
                rgba.push(0);
                rgba.push(0);
                rgba.push(0);
                rgba.push(0);
            }
        }
    }
    Icon::from_rgba(rgba, width, height).expect("failed to create icon")
}

fn play_sound(sound_path: &str) {
    let raw_path = sound_path.trim();
    if raw_path.is_empty() {
        return;
    }

    let expanded_path = if raw_path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(&raw_path[2..])
        } else {
            PathBuf::from(raw_path)
        }
    } else {
        PathBuf::from(raw_path)
    };

    if !expanded_path.exists() {
        return;
    }

    let path_str = expanded_path.to_string_lossy().to_string();

    #[cfg(target_os = "linux")]
    {
        thread::spawn(move || {
            let _ = Command::new("pw-play")
                .arg(&path_str)
                .output()
                .or_else(|_| Command::new("paplay").arg(&path_str).output())
                .or_else(|_| Command::new("aplay").arg(&path_str).output())
                .or_else(|_| Command::new("canberra-gtk-play").arg("-f").arg(&path_str).output())
                .or_else(|_| Command::new("ffplay").arg("-nodisp").arg("-autoexit").arg(&path_str).output());
        });
    }

    #[cfg(target_os = "macos")]
    {
        thread::spawn(move || {
            let _ = Command::new("afplay").arg(&path_str).output();
        });
    }

    #[cfg(target_os = "windows")]
    {
        thread::spawn(move || {
            let script = format!("(New-Object Media.SoundPlayer '{}').PlaySync()", path_str.replace('\'', "''"));
            let _ = Command::new("powershell")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg(script)
                .output();
        });
    }
}

fn show_connection_notification(conn: bool, cfg: &AppConfig) {
    let message = if conn {
        if cfg.custom_messages.on_connect.trim().is_empty() {
            "YubiKey has been connected"
        } else {
            cfg.custom_messages.on_connect.as_str()
        }
    } else {
        if cfg.custom_messages.on_disconnect.trim().is_empty() {
            "YubiKey has been disconnected"
        } else {
            cfg.custom_messages.on_disconnect.as_str()
        }
    };

    if !cfg.notification.sound.trim().is_empty() {
        play_sound(&cfg.notification.sound);
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Notification::new()
            .summary("yubi-tray-rs")
            .body(message)
            .icon("security-high")
            .show();
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"yubi-tray-rs\"",
            message
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = Toast::new(Toast::POWERSHELL_APP_ID)
            .title("yubi-tray-rs")
            .text1(message)
            .duration(WinrtDuration::Short)
            .show();
    }
}
