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

fn should_toggle_panel(button: MouseButton, state: MouseButtonState) -> bool {
    button == MouseButton::Left && state == MouseButtonState::Up
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
        .show_menu_on_left_click(false)
        .tooltip("StepCast")
        .on_tray_icon_event(|tray, event| {
            if cfg!(debug_assertions) {
                eprintln!("Tray event: {:?}", event);
            }
            let app_handle = tray.app_handle();

            if let TrayIconEvent::Click {
                button,
                button_state,
                rect,
                ..
            } = event
            {
                if should_toggle_panel(button, button_state) {
                    let Some(panel) = get_or_init_panel!(app_handle) else {
                        return;
                    };

                    if panel.is_visible() {
                        panel.hide();
                        return;
                    }

                    panel.show_and_make_key();
                    if let Err(err) =
                        position_panel_at_tray_icon(app_handle, rect.position, rect.size)
                    {
                        eprintln!("Failed to position panel: {}", err);
                    }
                }
            }
        })
        .build(app_handle)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_toggle_panel;
    use tauri::tray::{MouseButton, MouseButtonState};

    #[test]
    fn toggles_on_left_up() {
        assert!(should_toggle_panel(MouseButton::Left, MouseButtonState::Up));
    }

    #[test]
    fn ignores_left_down() {
        assert!(!should_toggle_panel(
            MouseButton::Left,
            MouseButtonState::Down
        ));
    }

    #[test]
    fn ignores_right_clicks() {
        assert!(!should_toggle_panel(
            MouseButton::Right,
            MouseButtonState::Down
        ));
    }
}
