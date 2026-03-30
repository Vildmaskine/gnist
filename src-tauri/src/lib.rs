use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

struct SerialState {
    port: Arc<Mutex<Option<Box<dyn serialport::SerialPort>>>>,
    stop_flag: Arc<AtomicBool>,
}

impl Default for SerialState {
    fn default() -> Self {
        Self {
            port: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[tauri::command]
fn list_ports() -> Vec<String> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.port_name)
        .filter(|name| {
            let base = name.trim_start_matches("/dev/");
            base.starts_with("ttyUSB")
                || base.starts_with("ttyACM")
                || base.starts_with("ttyAMA")
        })
        .collect()
}

#[tauri::command]
fn connect_port(
    state: tauri::State<SerialState>,
    app: AppHandle,
    port_name: String,
    baud_rate: u32,
) -> Result<(), String> {
    // Stop existing read thread if any
    state.stop_flag.store(true, Ordering::Relaxed);
    *state.port.lock().unwrap() = None;

    let port = serialport::new(&port_name, baud_rate)
        .timeout(Duration::from_millis(10))
        .open()
        .map_err(|e| e.to_string())?;

    let mut reader = port.try_clone().map_err(|e| e.to_string())?;
    *state.port.lock().unwrap() = Some(port);

    state.stop_flag.store(false, Ordering::Relaxed);
    let stop_flag = state.stop_flag.clone();

    std::thread::spawn(move || {
        let mut buf = vec![0u8; 256];
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            match reader.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let _ = app.emit("serial-data", buf[..n].to_vec());
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => {
                    let _ = app.emit("serial-disconnected", ());
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn disconnect_port(state: tauri::State<SerialState>) {
    state.stop_flag.store(true, Ordering::Relaxed);
    *state.port.lock().unwrap() = None;
}

#[tauri::command]
fn write_port(state: tauri::State<SerialState>, data: Vec<u8>) -> Result<(), String> {
    let mut lock = state.port.lock().unwrap();
    match lock.as_mut() {
        Some(port) => port.write_all(&data).map_err(|e| e.to_string()),
        None => Err("Port not open".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SerialState::default())
        .setup(|_app| {
            #[cfg(target_os = "linux")]
            // tao hardcodes the button layout on the HeaderBar for Wayland windows,
            // overriding the system setting. We clear it so GTK reads the layout
            // from the Settings portal / XSETTINGS — the same way all other GTK apps do.
            glib::idle_add_once(|| {
                use gtk::prelude::*;
                for widget in gtk::Window::list_toplevels() {
                    let win = match widget.downcast::<gtk::Window>() {
                        Ok(w) => w,
                        Err(_) => continue,
                    };
                    if let Some(titlebar) = win.titlebar() {
                        let header = if let Some(eb) = titlebar.downcast_ref::<gtk::EventBox>() {
                            eb.children().into_iter().find_map(|c| c.downcast::<gtk::HeaderBar>().ok())
                        } else {
                            titlebar.downcast::<gtk::HeaderBar>().ok()
                        };
                        if let Some(h) = header {
                            h.set_decoration_layout(None::<&str>);
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_ports,
            connect_port,
            disconnect_port,
            write_port,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}