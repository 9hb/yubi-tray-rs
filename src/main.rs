#![windows_subsystem = "windows"]

use hidapi::{ HidApi, DeviceInfo };
use std::sync::{ mpsc, Arc, Mutex };
use std::thread;
use std::time::Duration;
use tao::event_loop::{ ControlFlow, EventLoop };
use tray_icon::menu::{ Menu, MenuEvent, MenuItem, CheckMenuItem };
use tray_icon::{ Icon, TrayIconBuilder, TrayIconEvent };
use winrt_notification::Toast;

const YUBICO_VENDOR_ID: u16 = 0x1050;

fn main() {
    let event_loop = EventLoop::new();

    // setup tray menu with "exit" item
    let notifications_enabled = Arc::new(Mutex::new(false));

    let tray_menu = Menu::new();
    let notif_toggle = CheckMenuItem::new("Notifications", true, false, None);
    let quit_item = MenuItem::new("Exit", true, None);
    let _ = tray_menu.append(&notif_toggle);
    let _ = tray_menu.append(&quit_item);

    let (tx, rx) = mpsc::channel::<Option<String>>();
    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    // detection thread that checks for yubikey presence every second
    thread::spawn(move || {
        loop {
            let info = get_yubikey_info();
            let _ = tx.send(info);
            thread::sleep(Duration::from_secs(1));
        }
    });

    let icon = generate_icon(255, 0, 0); // initial icon (red)

    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("searching for yubikey...")
        .with_icon(icon)
        .with_menu(Box::new(tray_menu))
        .build()
        .unwrap();

    let mut previous_state: Option<bool> = None;

    // main event loop
    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + Duration::from_millis(200)
        );

        // [1] check for menu events
        if let Ok(_tray_event) = tray_channel.try_recv() {
        }

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_item.id() {
                *control_flow = ControlFlow::Exit;
                return;
            }
            if event.id == notif_toggle.id() {
                let mut enabled = notifications_enabled.lock().unwrap();
                *enabled = !*enabled;
                let _ = notif_toggle.set_checked(*enabled);
            }
        }

        // [2]
        // [2] check for yubikey status updates
        if let Ok(maybe_info) = rx.try_recv() {
            let currently_connected = maybe_info.is_some();

            if let Some(was_connected) = previous_state {
                if currently_connected != was_connected {
                    let enabled = *notifications_enabled.lock().unwrap();
                    if enabled {
                        show_notification(currently_connected);
                    }
                }
            }

            previous_state = Some(currently_connected);

            match maybe_info {
                Some(info_text) => {
                    // CONNECTED (green)
                    let _ = tray_icon.set_tooltip(Some(info_text));
                    let _ = tray_icon.set_icon(Some(generate_icon(0, 255, 0)));
                }
                None => {
                    // DISCONNECTED (red)
                    let _ = tray_icon.set_tooltip(Some("Connect a YubiKey".to_string()));
                    let _ = tray_icon.set_icon(Some(generate_icon(255, 0, 0)));
                }
            }
        }
    });
}

fn get_yubikey_info() -> Option<String> {
    match HidApi::new() {
        Ok(api) => {
            for device in api.device_list() {
                if device.vendor_id() == YUBICO_VENDOR_ID {
                    return Some(format_device_info(device));
                }
            }
            None
        }
        Err(_) => None,
    }
}

fn format_device_info(device: &DeviceInfo) -> String {
    let product = device.product_string().unwrap_or("Unknown Device");
    let vid = device.vendor_id();
    let pid = device.product_id();

    match device.serial_number() {
        Some(s) if !s.is_empty() => {
            // s/n exists -> show it
            format!(
                "{}\n\
                 S/N: {}\n\
                 ID: {:04x}:{:04x}",
                product,
                s,
                vid,
                pid
            )
        }
        _ => {
            // s/n does not exist or is empty -> ignore it
            format!("{}\n\
                 ID: {:04x}:{:04x}", product, vid, pid)
        }
    }
}

fn generate_icon(r: u8, g: u8, b: u8) -> Icon {
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let dx = (x as i32) - (width as i32) / 2;
            let dy = (y as i32) - (height as i32) / 2;
            let distance = ((dx * dx + dy * dy) as f64).sqrt();

            if distance < (width as f64) / 2.0 - 2.0 {
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
    let (t1, text) = if conn {
        ("yubi-tray-rs", "YubiKey has been connected")
    } else {
        ("yubi-tray-rs", "YubiKey has been disconnected")
    };

    if let Ok(toast) = Toast::new(Toast::POWERSHELL_APP_ID)
        .title(t1)
        .text1(text)
        .duration(winrt_notification::Duration::Short)
        .show()
    {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            let _ = toast.hide();
        });
    }
}
