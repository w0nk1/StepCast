use std::path::PathBuf;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent, TrayIconId};
use tauri::{AppHandle, Emitter, Manager};
use tauri_nspanel::ManagerExt;

use crate::panel::TrayIconMetrics;
use crate::panel::{panel_label, position_panel_at_tray_icon};
use crate::recorder::pipeline::PanelRect;

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

/// Show the panel positioned at the tray icon. Used by tray menu and global shortcut.
pub fn show_panel(app_handle: &AppHandle) {
    let Some(panel) = get_or_init_panel!(app_handle) else {
        return;
    };
    panel.show_and_make_key();
    let is_fallback = position_panel_at_current_tray_icon(app_handle).is_err();
    if is_fallback {
        eprintln!("Tray position unavailable, using fallback");
        if let Err(fb_err) = crate::panel::fallback_panel_position(app_handle) {
            eprintln!("Fallback position also failed: {fb_err}");
        }
    }
    let _ = app_handle.emit("panel-positioned", !is_fallback);
    if let Ok(bounds) = crate::panel::panel_bounds(app_handle) {
        let ps = &app_handle.state::<crate::RecorderAppState>().pipeline_state;
        crate::recorder::pipeline::record_panel_bounds(ps, bounds);
    }
    let ps = &app_handle.state::<crate::RecorderAppState>().pipeline_state;
    crate::recorder::pipeline::set_panel_visible(ps, true);
}

/// Toggle panel visibility. Used by global shortcut handler.
pub fn toggle_panel(app_handle: &AppHandle) {
    let Some(panel) = get_or_init_panel!(app_handle) else {
        return;
    };
    let ps = &app_handle.state::<crate::RecorderAppState>().pipeline_state;
    if panel.is_visible() {
        panel.hide();
        crate::recorder::pipeline::set_panel_visible(ps, false);
    } else {
        show_panel(app_handle);
    }
}

fn should_toggle_panel(button: MouseButton, state: MouseButtonState) -> bool {
    button == MouseButton::Left && state == MouseButtonState::Up
}

fn select_tray_rect(api_rect: Option<tauri::Rect>, event_rect: tauri::Rect) -> tauri::Rect {
    let Some(api_rect) = api_rect else {
        return event_rect;
    };

    let event_is_physical = matches!(event_rect.position, tauri::Position::Physical(_))
        && matches!(event_rect.size, tauri::Size::Physical(_));
    let api_is_physical = matches!(api_rect.position, tauri::Position::Physical(_))
        && matches!(api_rect.size, tauri::Size::Physical(_));

    if event_is_physical && api_is_physical {
        let (event_pos, event_size) = match (&event_rect.position, &event_rect.size) {
            (tauri::Position::Physical(pos), tauri::Size::Physical(size)) => (pos, size),
            _ => return event_rect,
        };
        let (api_pos, api_size) = match (&api_rect.position, &api_rect.size) {
            (tauri::Position::Physical(pos), tauri::Size::Physical(size)) => (pos, size),
            _ => return event_rect,
        };

        let dx = (event_pos.x - api_pos.x).abs();
        let dy = (event_pos.y - api_pos.y).abs();
        let dw = (event_size.width as i32 - api_size.width as i32).abs();
        let dh = (event_size.height as i32 - api_size.height as i32).abs();

        if dx <= 2 && dy <= 2 && dw <= 2 && dh <= 2 {
            return api_rect;
        }
    }

    event_rect
}

fn rect_debug(rect: &tauri::Rect) -> String {
    let (pos, size) = (&rect.position, &rect.size);
    match (pos, size) {
        (tauri::Position::Physical(pos), tauri::Size::Physical(size)) => format!(
            "physical pos=({},{}) size=({}x{})",
            pos.x, pos.y, size.width, size.height
        ),
        (tauri::Position::Logical(pos), tauri::Size::Logical(size)) => format!(
            "logical pos=({:.2},{:.2}) size=({:.2}x{:.2})",
            pos.x, pos.y, size.width, size.height
        ),
        _ => "mixed-units".to_string(),
    }
}

const PANEL_ALIGN_X_PADDING_PX: i32 = 12;
const PANEL_ALIGN_Y_SLACK_PX: i32 = 120;

fn should_hide_panel(
    panel_visible: bool,
    panel_bounds: Option<PanelRect>,
    tray_metrics: &TrayIconMetrics,
) -> bool {
    if !panel_visible {
        return false;
    }
    let Some(bounds) = panel_bounds else {
        return true;
    };

    let tray_center_x = tray_metrics.x + (tray_metrics.width / 2);
    let panel_center_x = bounds.x + (bounds.width / 2);
    let dx = (panel_center_x - tray_center_x).abs();
    let aligned_x = dx <= (bounds.width / 2 + PANEL_ALIGN_X_PADDING_PX);

    let y_min = tray_metrics.y;
    let y_max = tray_metrics.y + tray_metrics.height + PANEL_ALIGN_Y_SLACK_PX;
    let aligned_y = bounds.y >= y_min && bounds.y <= y_max;

    aligned_x && aligned_y
}

/// Set tray to recording state with red recording icon
pub fn set_recording_icon(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray = app_handle
        .tray_by_id(&TrayIconId::new(TRAY_ID))
        .ok_or_else(|| {
            tauri::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "tray icon not found",
            ))
        })?;

    // Load recording icon
    let icon_path = app_handle
        .path()
        .resolve("icons/recording.png", BaseDirectory::Resource)?;
    let icon = Image::from_path(icon_path)?;

    tray.set_icon(Some(icon))?;
    tray.set_icon_as_template(false)?; // Keep red color, don't adapt to system theme
    let locale = crate::i18n::system_locale();
    tray.set_tooltip(Some(crate::i18n::tray_recording_tooltip(locale)))?;
    Ok(())
}

/// Reset tray to default state
pub fn set_default_icon(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray = app_handle
        .tray_by_id(&TrayIconId::new(TRAY_ID))
        .ok_or_else(|| {
            tauri::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "tray icon not found",
            ))
        })?;

    // Load default icon
    let icon_path = resolve_tray_icon_path(app_handle)?;
    let icon = Image::from_path(icon_path)?;

    tray.set_icon(Some(icon))?;
    tray.set_icon_as_template(true)?; // Adapt to system theme
    let locale = crate::i18n::system_locale();
    tray.set_tooltip(Some(crate::i18n::tray_tooltip(locale)))?;
    Ok(())
}

pub fn position_panel_at_current_tray_icon(app_handle: &AppHandle) -> Result<(), String> {
    let tray = app_handle
        .tray_by_id(&TrayIconId::new(TRAY_ID))
        .ok_or_else(|| "tray icon not found".to_string())?;
    let rect = tray
        .rect()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "tray icon rect unavailable".to_string())?;

    position_panel_at_tray_icon(app_handle, rect.position, rect.size)
}

fn resolve_tray_icon_path(app_handle: &AppHandle) -> tauri::Result<PathBuf> {
    let candidates = [
        (BaseDirectory::Resource, "icons/tray.png"),
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
    let locale = crate::i18n::system_locale();

    let open = MenuItem::with_id(
        app_handle,
        "open",
        crate::i18n::tray_menu_open(locale),
        true,
        None::<&str>,
    )?;
    let quick_start = MenuItem::with_id(
        app_handle,
        "quick_start",
        crate::i18n::tray_menu_quick_start(locale),
        true,
        None::<&str>,
    )?;
    let sep = PredefinedMenuItem::separator(app_handle)?;
    let quit = MenuItem::with_id(
        app_handle,
        "quit",
        crate::i18n::tray_menu_quit(locale),
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(app_handle, &[&open, &quick_start, &sep, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .show_menu_on_left_click(false)
        .menu(&menu)
        .on_menu_event(|app_handle, event| {
            let id = event.id().0.as_str();
            match id {
                "open" => show_panel(app_handle),
                "quick_start" => {
                    show_panel(app_handle);
                    let _ = app_handle.emit("show-quick-start", ());
                }
                "quit" => app_handle.exit(0),
                _ => {}
            }
        })
        .tooltip(crate::i18n::tray_tooltip(locale))
        .on_tray_icon_event(|tray, event| {
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

                    let api_rect = tray.rect().ok().flatten();
                    let effective_rect = select_tray_rect(api_rect, rect);
                    if cfg!(debug_assertions) {
                        let api_rect_str = api_rect
                            .as_ref()
                            .map(rect_debug)
                            .unwrap_or_else(|| "none".to_string());
                        eprintln!(
                            "Tray rects: event={} api={} effective={}",
                            rect_debug(&rect),
                            api_rect_str,
                            rect_debug(&effective_rect)
                        );
                    }

                    let panel_visible = panel.is_visible();
                    let panel_bounds = crate::panel::panel_bounds(app_handle).ok();

                    let metrics = crate::panel::tray_icon_metrics(
                        app_handle,
                        &effective_rect.position,
                        &effective_rect.size,
                    );
                    let should_hide = metrics
                        .as_ref()
                        .map(|m| should_hide_panel(panel_visible, panel_bounds, m))
                        .unwrap_or(panel_visible);

                    let ps = &app_handle.state::<crate::RecorderAppState>().pipeline_state;
                    if let Ok(ref metrics) = metrics {
                        crate::recorder::pipeline::record_tray_click(
                            ps,
                            crate::recorder::pipeline::TrayRect {
                                x: metrics.x,
                                y: metrics.y,
                                width: metrics.width,
                                height: metrics.height,
                            },
                        );
                    }

                    if should_hide {
                        panel.hide();
                        crate::recorder::pipeline::set_panel_visible(ps, false);
                        return;
                    }

                    panel.show_and_make_key();
                    let click_is_fallback = position_panel_at_tray_icon(app_handle, effective_rect.position, effective_rect.size).is_err();
                    if click_is_fallback {
                        eprintln!("Tray position unavailable on click, using fallback");
                        if let Err(fb_err) = crate::panel::fallback_panel_position(app_handle) {
                            eprintln!("Fallback position also failed: {fb_err}");
                        }
                    }
                    let _ = app_handle.emit("panel-positioned", !click_is_fallback);
                    // Reposition shortly after showing to account for late size updates
                    {
                        let app_handle = app_handle.clone();
                        let position = effective_rect.position;
                        let size = effective_rect.size;
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(60));
                            let app_handle_inner = app_handle.clone();
                            let _ = app_handle.run_on_main_thread(move || {
                                if let Err(err) = position_panel_at_tray_icon(&app_handle_inner, position, size) {
                                    eprintln!("Delayed reposition failed ({err}), using fallback");
                                    if let Err(fb_err) = crate::panel::fallback_panel_position(&app_handle_inner) {
                                        eprintln!("Fallback position also failed: {fb_err}");
                                    }
                                }
                                if cfg!(debug_assertions) {
                                    if let Ok(bounds) = crate::panel::panel_bounds(&app_handle_inner) {
                                        eprintln!(
                                            "Panel bounds after delayed reposition: x={} y={} w={} h={}",
                                            bounds.x, bounds.y, bounds.width, bounds.height
                                        );
                                    }
                                }
                                if let Ok(bounds) = crate::panel::panel_bounds(&app_handle_inner) {
                                    let ps_inner = &app_handle_inner.state::<crate::RecorderAppState>().pipeline_state;
                                    crate::recorder::pipeline::record_panel_bounds(ps_inner, bounds);
                                }
                            });
                        });
                    }
                    if cfg!(debug_assertions) {
                        if let Ok(bounds) = crate::panel::panel_bounds(app_handle) {
                            eprintln!(
                                "Panel bounds after show: x={} y={} w={} h={}",
                                bounds.x, bounds.y, bounds.width, bounds.height
                            );
                        }
                    }
                    if let Ok(bounds) = crate::panel::panel_bounds(app_handle) {
                        crate::recorder::pipeline::record_panel_bounds(ps, bounds);
                    }
                    crate::recorder::pipeline::set_panel_visible(ps, true);
                }
            }
        })
        .build(app_handle)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{rect_debug, select_tray_rect, should_hide_panel, should_toggle_panel};
    use crate::panel::TrayIconMetrics;
    use crate::recorder::pipeline::PanelRect;
    use tauri::tray::{MouseButton, MouseButtonState};
    use tauri::{Position, Rect, Size};

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

    #[test]
    fn tray_rect_prefers_event_rect_when_mismatched() {
        let event_rect = Rect {
            position: Position::Physical((10, 10).into()),
            size: Size::Physical((16, 16).into()),
        };
        let api_rect = Rect {
            position: Position::Physical((20, 20).into()),
            size: Size::Physical((18, 18).into()),
        };

        let selected = select_tray_rect(Some(api_rect), event_rect);

        assert_eq!(selected.position, event_rect.position);
        assert_eq!(selected.size, event_rect.size);
    }

    #[test]
    fn tray_rect_uses_api_rect_when_equivalent() {
        let event_rect = Rect {
            position: Position::Physical((10, 10).into()),
            size: Size::Physical((16, 16).into()),
        };
        let api_rect = Rect {
            position: Position::Physical((11, 11).into()),
            size: Size::Physical((16, 16).into()),
        };

        let selected = select_tray_rect(Some(api_rect), event_rect);

        assert_eq!(selected.position, api_rect.position);
        assert_eq!(selected.size, api_rect.size);
    }

    #[test]
    fn tray_rect_falls_back_to_event_rect() {
        let event_rect = Rect {
            position: Position::Physical((10, 10).into()),
            size: Size::Physical((16, 16).into()),
        };

        let selected = select_tray_rect(None, event_rect);

        assert_eq!(selected.position, event_rect.position);
        assert_eq!(selected.size, event_rect.size);
    }

    #[test]
    fn rect_debug_formats_physical_rect() {
        let rect = Rect {
            position: Position::Physical((10, 20).into()),
            size: Size::Physical((16, 12).into()),
        };

        assert_eq!(rect_debug(&rect), "physical pos=(10,20) size=(16x12)");
    }

    #[test]
    fn rect_debug_formats_logical_rect() {
        let rect = Rect {
            position: Position::Logical((10.0, 20.0).into()),
            size: Size::Logical((16.0, 12.0).into()),
        };

        assert_eq!(
            rect_debug(&rect),
            "logical pos=(10.00,20.00) size=(16.00x12.00)"
        );
    }

    #[test]
    fn should_hide_panel_when_aligned() {
        let metrics = TrayIconMetrics {
            x: 100,
            y: 0,
            width: 20,
            height: 10,
            scale_factor: 1.0,
        };
        let bounds = PanelRect {
            x: 0,
            y: 12,
            width: 200,
            height: 100,
        };

        assert!(should_hide_panel(true, Some(bounds), &metrics));
    }

    #[test]
    fn should_not_hide_panel_when_far() {
        let metrics = TrayIconMetrics {
            x: 100,
            y: 0,
            width: 20,
            height: 10,
            scale_factor: 1.0,
        };
        let bounds = PanelRect {
            x: 1000,
            y: 400,
            width: 200,
            height: 100,
        };

        assert!(!should_hide_panel(true, Some(bounds), &metrics));
    }

    #[test]
    fn should_not_hide_panel_when_not_visible() {
        let metrics = TrayIconMetrics {
            x: 100,
            y: 0,
            width: 20,
            height: 10,
            scale_factor: 1.0,
        };
        let bounds = PanelRect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        };

        assert!(!should_hide_panel(false, Some(bounds), &metrics));
    }
}
