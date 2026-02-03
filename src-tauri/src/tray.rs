use std::path::PathBuf;
use tauri::image::Image;
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_nspanel::ManagerExt;

use crate::panel::{panel_label, position_panel_at_tray_icon};

macro_rules! get_or_init_panel {
    ($app_handle:expr) => {{
        let label = panel_label();
        match $app_handle.get_webview_panel(label) {
            Ok(panel) => Some(panel),
            Err(_) => {
                if let Err(err) = crate::panel::init($app_handle) {
                    eprintln!("Failed to init panel: {}", err);
                    None
                } else {
                    match $app_handle.get_webview_panel(label) {
                        Ok(panel) => Some(panel),
                        Err(err) => {
                            eprintln!("Panel missing after init: {:?}", err);
                            None
                        }
                    }
                }
            }
        }
    }};
}

fn resolve_tray_icon_path(app_handle: &AppHandle) -> tauri::Result<PathBuf> {
    let candidates = [
        (BaseDirectory::Resource, "icons/icon.png"),
        (BaseDirectory::Resource, "icon.png"),
    ];

    for (base, rel) in candidates {
        if let Ok(path) = app_handle.path().resolve(rel, base) {
            if path.is_file() {
                return Ok(path);
            }
        }
    }

    Err(tauri::Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "tray icon missing; tried Resource/App icon paths",
    )))
}

pub fn create(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray_icon_path = resolve_tray_icon_path(app_handle)?;
    let icon = Image::from_path(tray_icon_path)?;

    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("StepCast")
        .on_tray_icon_event(|tray, event| {
            let app_handle = tray.app_handle();

            if let TrayIconEvent::Click {
                button,
                button_state,
                rect,
                ..
            } = event
            {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    let Some(panel) = get_or_init_panel!(app_handle) else {
                        return;
                    };

                    if panel.is_visible() {
                        panel.hide();
                        return;
                    }

                    if let Err(err) = position_panel_at_tray_icon(app_handle, rect) {
                        eprintln!("Failed to position panel: {}", err);
                    }

                    panel.show();
                }
            }
        })
        .build(app_handle)?;

    Ok(())
}
