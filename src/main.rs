use hidapi::HidApi;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, CheckMenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};
use notify_rust::Notification;
use directories::ProjectDirs;

#[derive(Debug, Clone)]
struct YubiKeyDetails {
    product_name: String,
    serial_number: Option<String>,
    vid: u16,
    pid: u16,
    hid_path: String,
}

#[derive(Debug)]
enum UserEvent {
    YubiKeyUpdate(Option<YubiKeyDetails>),
}

const YUBICO_VENDOR_ID: u16 = 0x1050;

fn get_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "yubi-tray-rs").map(|dirs| dirs.config_dir().join("config.txt"))
}

fn load_notifications_enabled() -> bool {
    get_config_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|content| content.trim() == "1")
        .unwrap_or(true)
}

fn save_notifications_enabled(enabled: bool) {
    if let Some(path) = get_config_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, if enabled { "1" } else { "0" });
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

    let initial_notif_state = load_notifications_enabled();
    let notifications_enabled = Arc::new(Mutex::new(initial_notif_state));

    let initial_device = get_yubikey_details();
    let is_connected = initial_device.is_some();

    let (dev_name, dev_hid) = format_menu_strings(&initial_device);

    let tray_menu = Menu::new();
    let notif_toggle = CheckMenuItem::new("Notifications", true, initial_notif_state, None);
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

    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(1000));
            let dev = get_yubikey_details();
            let _ = proxy.send_event(UserEvent::YubiKeyUpdate(dev));
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
                let mut enabled = notifications_enabled.lock().unwrap();
                *enabled = !*enabled;
                let _ = notif_toggle.set_checked(*enabled);
                save_notifications_enabled(*enabled);
            }
            // info_name and info_hid are unselectable and do not trigger actions
        }

        match event {
            tao::event::Event::UserEvent(UserEvent::YubiKeyUpdate(maybe_dev)) => {
                let currently_connected = maybe_dev.is_some();

                if let Some(was_connected) = previous_state {
                    if currently_connected != was_connected {
                        let enabled = *notifications_enabled.lock().unwrap();
                        if enabled {
                            show_notification(currently_connected);
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

fn show_notification(conn: bool) {
    let text = if conn { "YubiKey has been connected" } else { "YubiKey has been disconnected" };

    let _ = Notification::new()
        .summary("yubi-tray-rs")
        .body(text)
        .icon("security-high")
        .show();
}
