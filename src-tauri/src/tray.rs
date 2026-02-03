use std::path::PathBuf;
use tauri::image::Image;
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent, TrayIconId};
use tauri::{AppHandle, Manager};
use tauri_nspanel::ManagerExt;

use crate::panel::{panel_label, position_panel_at_tray_icon};

const TRAY_ID: &str = "tray";

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

/// Create a red recording dot icon (22x22 for retina)
fn create_recording_icon() -> Image<'static> {
    const SIZE: usize = 22;
    const CENTER: f32 = SIZE as f32 / 2.0;
    const RADIUS: f32 = 7.0;

    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - CENTER;
            let dy = y as f32 - CENTER;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = (y * SIZE + x) * 4;

            if dist <= RADIUS {
                // Red color with slight anti-aliasing at edge
                let alpha = if dist > RADIUS - 1.0 {
                    ((RADIUS - dist) * 255.0) as u8
                } else {
                    255
                };
                rgba[idx] = 255;     // R
                rgba[idx + 1] = 59;  // G
                rgba[idx + 2] = 48;  // B (Apple red: #ff3b30)
                rgba[idx + 3] = alpha;
            }
        }
    }

    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}

/// Set tray icon to recording state (red dot)
pub fn set_recording_icon(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray = app_handle.tray_by_id(&TrayIconId::new(TRAY_ID))
        .ok_or_else(|| tauri::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "tray icon not found"
        )))?;

    let icon = create_recording_icon();
    tray.set_icon(Some(icon))?;
    tray.set_icon_as_template(false)?; // Show actual red color
    tray.set_tooltip(Some("StepCast - Recording..."))?;
    Ok(())
}

/// Reset tray icon to default state
pub fn set_default_icon(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray = app_handle.tray_by_id(&TrayIconId::new(TRAY_ID))
        .ok_or_else(|| tauri::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "tray icon not found"
        )))?;

    let icon_path = resolve_tray_icon_path(app_handle)?;
    let icon = Image::from_path(icon_path)?;
    tray.set_icon(Some(icon))?;
    tray.set_icon_as_template(true)?;
    tray.set_tooltip(Some("StepCast"))?;
    Ok(())
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

    TrayIconBuilder::with_id(TRAY_ID)
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
