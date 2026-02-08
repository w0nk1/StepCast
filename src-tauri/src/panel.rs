use tauri::{AppHandle, Manager, Position, Size};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

const PANEL_LABEL: &str = "main";

#[derive(Debug, Clone, Copy)]
pub struct TrayIconMetrics {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f64,
}

tauri_panel! {
    panel!(StepCastPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            becomes_key_only_if_needed: true,
            is_floating_panel: true,
            hides_on_deactivate: false
        }
    })
}

pub fn panel_label() -> &'static str {
    PANEL_LABEL
}

fn panel_level() -> i64 {
    PanelLevel::MainMenu.value() + 1
}

fn panel_collection_behavior() -> CollectionBehavior {
    CollectionBehavior::new()
        .can_join_all_spaces()
        .stationary()
        .full_screen_auxiliary()
}

fn panel_style_mask() -> StyleMask {
    StyleMask::empty().nonactivating_panel()
}

fn should_convert_existing_window(has_panel: bool, has_window: bool) -> bool {
    !has_panel && has_window
}

#[cfg(test)]
fn resolve_monitor_work_area(
    current: Option<tauri::PhysicalRect<i32, u32>>,
    primary: Option<tauri::PhysicalRect<i32, u32>>,
    available: Vec<tauri::PhysicalRect<i32, u32>>,
) -> Option<tauri::PhysicalRect<i32, u32>> {
    current.or(primary).or_else(|| available.into_iter().next())
}

#[cfg(test)]
fn clamp_panel_position(
    x: f64,
    y: f64,
    panel_width: f64,
    panel_height: f64,
    monitor_rect: tauri::PhysicalRect<i32, u32>,
) -> (f64, f64) {
    let monitor_x = monitor_rect.position.x as f64;
    let monitor_y = monitor_rect.position.y as f64;
    let monitor_width = monitor_rect.size.width as f64;
    let monitor_height = monitor_rect.size.height as f64;

    let min_x = monitor_x;
    let min_y = monitor_y;
    let max_x = monitor_x + monitor_width - panel_width;
    let max_y = monitor_y + monitor_height - panel_height;

    let clamped_x = if max_x < min_x {
        min_x
    } else {
        x.clamp(min_x, max_x)
    };
    let clamped_y = if max_y < min_y {
        min_y
    } else {
        y.clamp(min_y, max_y)
    };

    (clamped_x, clamped_y)
}

pub fn init(app_handle: &AppHandle) -> tauri::Result<()> {
    let has_panel = app_handle.get_webview_panel(PANEL_LABEL).is_ok();
    if has_panel {
        return Ok(());
    }

    let window = app_handle.get_webview_window(PANEL_LABEL);
    if should_convert_existing_window(has_panel, window.is_some()) {
        if let Some(window) = window {
            let panel = window.to_panel::<StepCastPanel>()?;
            panel.set_has_shadow(false);
            panel.set_opaque(false);
            panel.set_level(panel_level());
            panel.set_collection_behavior(panel_collection_behavior().value());
            panel.set_style_mask(panel_style_mask().value());
            panel.set_movable_by_window_background(true);

            panel.hide();
            return Ok(());
        }
    }

    Ok(())
}

fn icon_rect_physical(
    icon_position: &Position,
    icon_size: &Size,
    scale_factor: f64,
) -> (i32, i32, i32, i32) {
    let (icon_phys_x, icon_phys_y) = match icon_position {
        Position::Physical(pos) => (pos.x, pos.y),
        Position::Logical(pos) => (
            (pos.x * scale_factor).round() as i32,
            (pos.y * scale_factor).round() as i32,
        ),
    };

    let (icon_width_phys, icon_height_phys) = match icon_size {
        Size::Physical(size) => (size.width as i32, size.height as i32),
        Size::Logical(size) => (
            (size.width * scale_factor).round() as i32,
            (size.height * scale_factor).round() as i32,
        ),
    };

    (icon_phys_x, icon_phys_y, icon_width_phys, icon_height_phys)
}

pub fn tray_icon_metrics(
    app_handle: &AppHandle,
    icon_position: &Position,
    icon_size: &Size,
) -> Result<TrayIconMetrics, String> {
    let window = app_handle
        .get_webview_window(PANEL_LABEL)
        .ok_or_else(|| "panel window missing".to_string())?;

    let monitors = window.available_monitors().map_err(|err| err.to_string())?;
    for monitor in monitors {
        let scale_factor = monitor.scale_factor();
        let (icon_phys_x, icon_phys_y, icon_width_phys, icon_height_phys) =
            icon_rect_physical(icon_position, icon_size, scale_factor);
        let pos = monitor.position();
        let size = monitor.size();
        let x_in = icon_phys_x >= pos.x && icon_phys_x < pos.x + size.width as i32;
        let y_in = icon_phys_y >= pos.y && icon_phys_y < pos.y + size.height as i32;
        if x_in && y_in {
            return Ok(TrayIconMetrics {
                x: icon_phys_x,
                y: icon_phys_y,
                width: icon_width_phys,
                height: icon_height_phys,
                scale_factor,
            });
        }
    }

    Err("no monitor found containing tray icon position".to_string())
}

pub fn panel_bounds(app_handle: &AppHandle) -> Result<crate::recorder::pipeline::PanelRect, String> {
    let window = app_handle
        .get_webview_window(PANEL_LABEL)
        .ok_or_else(|| "panel window missing".to_string())?;
    let position = window.outer_position().map_err(|err| err.to_string())?;
    let size = window.outer_size().map_err(|err| err.to_string())?;

    Ok(crate::recorder::pipeline::PanelRect {
        x: position.x,
        y: position.y,
        width: size.width as i32,
        height: size.height as i32,
    })
}

/// Fallback position when tray icon location is unavailable (e.g. Menu Bar Hider).
/// Places the panel at the top-right of the primary monitor, just below the menu bar.
pub fn fallback_panel_position(app_handle: &AppHandle) -> Result<(), String> {
    let window = app_handle
        .get_webview_window(PANEL_LABEL)
        .ok_or_else(|| "panel window missing".to_string())?;

    let monitor = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no primary monitor".to_string())?;

    let scale = monitor.scale_factor();
    let screen_size = monitor.size();
    let screen_pos = monitor.position();
    let window_size = window.outer_size().map_err(|e| e.to_string())?;

    // Menu bar height: ~24pt on standard, ~37pt on notch displays
    let menu_bar_gap = (38.0 * scale).round() as i32;
    let padding_right = (12.0 * scale).round() as i32;

    let panel_x =
        screen_pos.x + screen_size.width as i32 - window_size.width as i32 - padding_right;
    let panel_y = screen_pos.y + menu_bar_gap;

    window
        .set_position(tauri::PhysicalPosition::new(panel_x, panel_y))
        .map_err(|e| e.to_string())
}

pub fn position_panel_at_tray_icon(
    app_handle: &AppHandle,
    icon_position: Position,
    icon_size: Size,
) -> Result<(), String> {
    let window = app_handle
        .get_webview_window(PANEL_LABEL)
        .ok_or_else(|| "panel window missing".to_string())?;

    let metrics = tray_icon_metrics(app_handle, &icon_position, &icon_size)?;
    let scale_factor = metrics.scale_factor;
    let window_size = window.outer_size().map_err(|err| err.to_string())?;
    let window_width_phys = window_size.width as i32;

    let icon_center_x_phys = metrics.x + (metrics.width / 2);
    let panel_x_phys = icon_center_x_phys - (window_width_phys / 2);
    let gap_points = 4.0;
    let gap_phys = (gap_points * scale_factor).round() as i32;
    let panel_y_phys = metrics.y + metrics.height + gap_phys;

    let position = tauri::PhysicalPosition::new(panel_x_phys, panel_y_phys);
    window
        .set_position(position)
        .map_err(|err| err.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_panel_position, icon_rect_physical, panel_collection_behavior, panel_label,
        panel_level, panel_style_mask, resolve_monitor_work_area, should_convert_existing_window,
    };
    use serde_json::Value;
    use tauri::{
        LogicalPosition, LogicalSize, PhysicalPosition, PhysicalRect, PhysicalSize, Position, Size,
    };
    use tauri_nspanel::{CollectionBehavior, PanelLevel, StyleMask};

    fn rect_at(x: i32, y: i32, width: u32, height: u32) -> PhysicalRect<i32, u32> {
        PhysicalRect {
            position: PhysicalPosition { x, y },
            size: PhysicalSize { width, height },
        }
    }

    fn rect_origin(rect: &PhysicalRect<i32, u32>) -> (i32, i32) {
        (rect.position.x, rect.position.y)
    }

    #[test]
    fn panel_label_is_stable() {
        assert_eq!(panel_label(), "main");
    }

    #[test]
    fn should_convert_existing_window_when_missing_panel() {
        assert!(should_convert_existing_window(false, true));
        assert!(!should_convert_existing_window(true, true));
        assert!(!should_convert_existing_window(false, false));
    }

    #[test]
    fn panel_size_matches_tauri_config() {
        let config: Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("valid tauri.conf.json");
        let width = config["app"]["windows"][0]["width"]
            .as_f64()
            .expect("width is number");
        let height = config["app"]["windows"][0]["height"]
            .as_f64()
            .expect("height is number");
        let expected_width = 340.0;
        let expected_height = 554.0;

        assert_eq!(width, expected_width);
        assert_eq!(height, expected_height);
    }

    #[test]
    fn panel_level_is_main_menu_plus_one() {
        assert_eq!(panel_level(), PanelLevel::MainMenu.value() + 1);
    }

    #[test]
    fn panel_collection_behavior_matches_expected() {
        let expected = CollectionBehavior::new()
            .can_join_all_spaces()
            .stationary()
            .full_screen_auxiliary();
        assert_eq!(panel_collection_behavior(), expected);
    }

    #[test]
    fn panel_style_mask_matches_expected() {
        let expected = StyleMask::empty().nonactivating_panel();
        assert_eq!(panel_style_mask(), expected);
    }

    #[test]
    fn icon_rect_physical_scales_logical_values() {
        let position = Position::Logical(LogicalPosition { x: 100.0, y: 10.0 });
        let size = Size::Logical(LogicalSize {
            width: 20.0,
            height: 12.0,
        });

        let rect = icon_rect_physical(&position, &size, 2.0);

        assert_eq!(rect, (200, 20, 40, 24));
    }

    #[test]
    fn icon_rect_physical_keeps_physical_values() {
        let position = Position::Physical(PhysicalPosition { x: 120, y: 8 });
        let size = Size::Physical(PhysicalSize {
            width: 18,
            height: 6,
        });

        let rect = icon_rect_physical(&position, &size, 2.0);

        assert_eq!(rect, (120, 8, 18, 6));
    }

    #[test]
    fn resolve_monitor_prefers_current() {
        let current = rect_at(10, 12, 100, 80);
        let primary = rect_at(1, 2, 200, 160);
        let available = vec![rect_at(3, 4, 300, 240)];

        let resolved = resolve_monitor_work_area(Some(current), Some(primary), available)
            .expect("monitor should resolve");

        assert_eq!(rect_origin(&resolved), (10, 12));
    }

    #[test]
    fn resolve_monitor_falls_back_to_primary() {
        let primary = rect_at(5, 6, 200, 160);
        let available = vec![rect_at(7, 8, 300, 240)];

        let resolved = resolve_monitor_work_area(None, Some(primary), available)
            .expect("monitor should resolve");

        assert_eq!(rect_origin(&resolved), (5, 6));
    }

    #[test]
    fn resolve_monitor_falls_back_to_first_available() {
        let available = vec![rect_at(7, 8, 300, 240)];

        let resolved =
            resolve_monitor_work_area(None, None, available).expect("monitor should resolve");

        assert_eq!(rect_origin(&resolved), (7, 8));
    }

    #[test]
    fn clamp_panel_position_keeps_within_monitor_bounds() {
        let monitor = rect_at(0, 0, 100, 100);
        let (x, y) = clamp_panel_position(-10.0, 90.0, 40.0, 30.0, monitor);

        assert_eq!((x, y), (0.0, 70.0));
    }

    #[test]
    fn clamp_panel_position_handles_panel_larger_than_monitor() {
        let monitor = rect_at(10, 20, 100, 80);
        let (x, y) = clamp_panel_position(50.0, 60.0, 120.0, 90.0, monitor);

        assert_eq!((x, y), (10.0, 20.0));
    }
}
