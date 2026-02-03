use tauri::{AppHandle, LogicalSize, Manager, PhysicalRect, Position, Rect, Size, WebviewUrl};
use tauri_nspanel::{tauri_panel, ManagerExt, PanelBuilder, PanelLevel, StyleMask};

const PANEL_LABEL: &str = "panel";
const PANEL_WIDTH: f64 = 350.0;
const PANEL_HEIGHT: f64 = 500.0;

tauri_panel! {
    panel!(StepCastPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            becomes_key_only_if_needed: true,
            is_floating_panel: true,
            hides_on_deactivate: true
        }
    })
}

pub fn panel_label() -> &'static str {
    PANEL_LABEL
}

fn panel_size() -> (f64, f64) {
    (PANEL_WIDTH, PANEL_HEIGHT)
}

fn resolve_monitor_work_area(
    current: Option<PhysicalRect<i32, u32>>,
    primary: Option<PhysicalRect<i32, u32>>,
    available: Vec<PhysicalRect<i32, u32>>,
) -> Option<PhysicalRect<i32, u32>> {
    current.or(primary).or_else(|| available.into_iter().next())
}

fn clamp_panel_position(
    x: f64,
    y: f64,
    panel_width: f64,
    panel_height: f64,
    monitor_rect: PhysicalRect<i32, u32>,
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
    if app_handle.get_webview_panel(PANEL_LABEL).is_ok() {
        return Ok(());
    }

    let panel = PanelBuilder::<_, StepCastPanel>::new(app_handle, PANEL_LABEL)
        .url(WebviewUrl::App("index.html".into()))
        .level(PanelLevel::Floating)
        .style_mask(StyleMask::empty().nonactivating_panel().utility_window())
        .becomes_key_only_if_needed(true)
        .hides_on_deactivate(true)
        .no_activate(true)
        .transparent(true)
        .size(Size::Logical(LogicalSize::new(PANEL_WIDTH, PANEL_HEIGHT)))
        .with_window(|window| window.decorations(false).transparent(true).resizable(false))
        .build()?;

    panel.hide();

    Ok(())
}

pub fn position_panel_at_tray_icon(app_handle: &AppHandle, rect: Rect) -> Result<(), String> {
    let panel = app_handle
        .get_webview_panel(PANEL_LABEL)
        .map_err(|err| format!("panel not found: {err:?}"))?;

    let (tray_x, tray_y) = match rect.position {
        Position::Physical(pos) => (pos.x as f64, pos.y as f64),
        Position::Logical(pos) => (pos.x, pos.y),
    };
    let (tray_width, tray_height) = match rect.size {
        Size::Physical(size) => (size.width as f64, size.height as f64),
        Size::Logical(size) => (size.width, size.height),
    };

    let (panel_width, panel_height) = panel_size();
    let x = tray_x + (tray_width / 2.0) - (panel_width / 2.0);
    let y = tray_y + tray_height;

    let window = panel
        .to_window()
        .ok_or_else(|| "panel window missing".to_string())?;

    let monitor_rect = resolve_monitor_work_area(
        window
            .current_monitor()
            .ok()
            .flatten()
            .map(|monitor| *monitor.work_area()),
        window
            .primary_monitor()
            .ok()
            .flatten()
            .map(|monitor| *monitor.work_area()),
        window
            .available_monitors()
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|monitor| *monitor.work_area())
            .collect(),
    );
    let (x, y) = match monitor_rect {
        Some(monitor_rect) => clamp_panel_position(x, y, panel_width, panel_height, monitor_rect),
        None => (x, y),
    };

    let position = Position::Physical(tauri::PhysicalPosition::new(
        x.round() as i32,
        y.round() as i32,
    ));
    window
        .set_position(position)
        .map_err(|err| err.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{clamp_panel_position, panel_label, resolve_monitor_work_area};
    use tauri::{PhysicalPosition, PhysicalRect, PhysicalSize};

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
        assert_eq!(panel_label(), "panel");
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
