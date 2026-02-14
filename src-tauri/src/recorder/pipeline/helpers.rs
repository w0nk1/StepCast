//! Pipeline helper functions: capture, filtering, debouncing, context menu detection.

use super::super::ax_helpers::{get_clicked_element_info, is_security_agent_process};
use super::super::capture::CaptureError;
use super::super::cg_capture::{capture_region_cg, capture_region_fast};
use super::super::click_event::ClickEvent;
use super::super::session::Session;
use super::super::types::{ActionType, BoundsPercent, Step};
use super::super::window_info::find_auth_dialog_window;
use super::super::window_info::WindowBounds;
use super::types::*;

use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlelessOverlayKind {
    NotOverlay,
    DropdownMenu,
    Popup,
}

pub fn is_ax_menu_role(role: &str) -> bool {
    role == accessibility_sys::kAXMenuItemRole
        || role == accessibility_sys::kAXMenuBarItemRole
        || role == accessibility_sys::kAXMenuButtonRole
        || role == accessibility_sys::kAXMenuRole
}

pub fn should_use_menu_region_capture(
    overlay_kind: TitlelessOverlayKind,
    ax_role: Option<&str>,
    click_y_in_display: i32,
    recent_menu_open: bool,
) -> bool {
    const MENUBAR_HEIGHT: i32 = 30;
    const TOP_MENU_INTERACTION_HEIGHT: i32 = 180;
    const MENU_FOLLOWUP_HEIGHT: i32 = 320;
    let is_menu_bar_click = (0..MENUBAR_HEIGHT).contains(&click_y_in_display);
    let is_dropdown_menu = overlay_kind == TitlelessOverlayKind::DropdownMenu;
    let is_top_menu_interaction = ax_role.is_some_and(is_ax_menu_role)
        && (0..TOP_MENU_INTERACTION_HEIGHT).contains(&click_y_in_display);
    let is_recent_menu_followup =
        recent_menu_open && (0..MENU_FOLLOWUP_HEIGHT).contains(&click_y_in_display);
    is_menu_bar_click || is_dropdown_menu || is_top_menu_interaction || is_recent_menu_followup
}

/// Prefer region capture for volatile interactions that commonly close/hide
/// overlays during the click handling path (menu rows, picker rows, etc.).
///
/// We keep this role-based and app-agnostic:
/// - `AXMenuItem` / `AXMenu` rows are transient by nature.
/// - `AXGroup` is often used by web/native pickers for clickable rows.
///
/// Right-click keeps the existing context-menu path.
pub fn should_prefer_transient_region_capture(
    ax_role: Option<&str>,
    overlay_kind: TitlelessOverlayKind,
    is_right_click: bool,
) -> bool {
    if is_right_click {
        return false;
    }

    let Some(role) = ax_role else {
        return false;
    };

    // Menu-bar/dropdown interactions are already handled by menu-region rules.
    if overlay_kind == TitlelessOverlayKind::DropdownMenu {
        return false;
    }

    role == accessibility_sys::kAXMenuItemRole
        || role == accessibility_sys::kAXMenuRole
        || (role == accessibility_sys::kAXGroupRole && overlay_kind == TitlelessOverlayKind::Popup)
}

/// Classify titleless overlay windows (menus vs popovers) for capture decisions.
///
/// We only treat a titleless window as a dropdown menu when it is near the menu bar;
/// otherwise it's more likely an in-app popover (sticker picker, emoji panel, etc.)
/// and should be captured by window bounds to avoid cropping.
pub fn classify_titleless_overlay_window(
    window_title: &str,
    window_id: u32,
    main_window_id: u32,
    bounds: &WindowBounds,
    display_top_y: i32,
) -> TitlelessOverlayKind {
    const MENUBAR_REGION_HEIGHT: i32 = 60;

    let is_titleless = window_title.trim().is_empty();
    if !is_titleless {
        return TitlelessOverlayKind::NotOverlay;
    }
    if window_id == 0 || window_id == main_window_id {
        return TitlelessOverlayKind::NotOverlay;
    }
    // Popovers/menus are typically narrow; avoid matching full-size overlays.
    if bounds.width >= 800 {
        return TitlelessOverlayKind::NotOverlay;
    }

    // IMPORTANT: use display-relative Y, not global Y.
    // On secondary displays, global Y may be far from 0 even when the window is near that
    // display's menu bar.
    let y_in_display = bounds.y - display_top_y;
    if (0..=MENUBAR_REGION_HEIGHT).contains(&y_in_display) {
        TitlelessOverlayKind::DropdownMenu
    } else {
        TitlelessOverlayKind::Popup
    }
}

pub fn debug_log(session: &Session, msg: &str) {
    if !cfg!(debug_assertions) {
        return;
    }

    let log_path = session.temp_dir.join("recording.log");
    let is_new = !log_path.exists();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        use std::io::Write;
        if is_new {
            let _ = writeln!(file, "session_dir={}", session.temp_dir.to_string_lossy());
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let _ = writeln!(file, "[{ts}] {msg}");
    }
}

pub fn write_auth_placeholder(path: &Path, width: u32, height: u32) -> Result<(), CaptureError> {
    use image::{imageops, Rgba, RgbaImage};

    let w = width.max(120);
    let h = height.max(80);

    let bytes = include_bytes!("../assets/coreauth.png");
    let img = image::load_from_memory(bytes)
        .map_err(|e| CaptureError::CgImage(format!("placeholder decode failed: {e}")))?;

    let resized = img.resize(w, h, imageops::FilterType::Lanczos3).to_rgba8();

    let mut canvas = RgbaImage::new(w, h);
    let bg = Rgba([30, 32, 36, 255]);
    for pixel in canvas.pixels_mut() {
        *pixel = bg;
    }

    let x = ((w as i32 - resized.width() as i32) / 2).max(0) as i64;
    let y = ((h as i32 - resized.height() as i32) / 2).max(0) as i64;
    imageops::overlay(&mut canvas, &resized, x, y);

    canvas
        .save(path)
        .map_err(|e| CaptureError::CgImage(format!("placeholder save failed: {e}")))?;

    Ok(())
}

pub fn capture_region_best(
    session: &Session,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    output_path: &Path,
) -> Result<(), CaptureError> {
    match capture_region_fast(x, y, width, height, output_path) {
        Ok(()) => {
            debug_log(
                session,
                &format!("fast_region_capture ok: x={x} y={y} w={width} h={height}",),
            );
            Ok(())
        }
        Err(err) => {
            debug_log(
                session,
                &format!("fast_region_capture failed: {err} (x={x} y={y} w={width} h={height})",),
            );
            capture_region_cg(x, y, width, height, output_path)
        }
    }
}

/// Validate that a screenshot file exists and is non-empty.
pub fn validate_screenshot(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => meta.len() > 0,
        Err(_) => false,
    }
}

pub fn should_emit_auth_prompt(ps: &mut PipelineState, window_id: u32, timestamp_ms: i64) -> bool {
    match ps.last_auth_prompt {
        Some((prev_id, prev_ts))
            if prev_id == window_id && timestamp_ms - prev_ts < AUTH_PROMPT_DEDUP_MS =>
        {
            false
        }
        _ => {
            ps.last_auth_prompt = Some((window_id, timestamp_ms));
            true
        }
    }
}

pub fn find_security_auth_window(
    click_x: i32,
    click_y: i32,
    clicked_info_missing: bool,
) -> Option<super::super::window_info::WindowInfo> {
    let auth_window = find_auth_dialog_window(click_x, click_y, clicked_info_missing)
        .ok()
        .flatten()?;
    if auth_window.window_id == 0 {
        return None;
    }
    if !is_security_agent_process(&auth_window.app_name) {
        return None;
    }
    Some(auth_window)
}

pub fn handle_auth_prompt(
    click: &ClickEvent,
    session: &mut Session,
    pipeline_state: &Mutex<PipelineState>,
) -> (Option<Step>, bool) {
    const AUTH_PLACEHOLDER_DESCRIPTION: &str =
        "Authenticate with Touch ID or enter your password to continue.";

    let clicked_info = get_clicked_element_info(click.x, click.y);
    let auth_window = match find_security_auth_window(click.x, click.y, clicked_info.is_none()) {
        Some(window) => window,
        None => return (None, false),
    };

    {
        let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        ps.last_auth_click_ms = Some(click.timestamp_ms);
    }

    let should_emit = {
        let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        should_emit_auth_prompt(&mut ps, auth_window.window_id, click.timestamp_ms)
    };

    if !should_emit {
        debug_log(
            session,
            &format!(
                "auth_prompt_dedup: window_id={} owner='{}'",
                auth_window.window_id, auth_window.app_name
            ),
        );
        return (None, true);
    }

    let bounds = &auth_window.bounds;
    if bounds.width == 0 || bounds.height == 0 {
        return (None, true);
    }

    let step_id = session.next_step_id();
    let screenshot_path = session.screenshot_path(&step_id);
    if let Err(err) = write_auth_placeholder(&screenshot_path, bounds.width, bounds.height) {
        if cfg!(debug_assertions) {
            eprintln!("Auth placeholder write failed: {err}");
        }
        return (None, true);
    }

    let center_x = bounds.x + (bounds.width as i32 / 2);
    let center_y = bounds.y + (bounds.height as i32 / 2);

    let step = Step {
        id: step_id,
        ts: click.timestamp_ms,
        action: ActionType::Click,
        x: center_x,
        y: center_y,
        click_x_percent: 50.0,
        click_y_percent: 50.0,
        app: "Authentication".to_string(),
        window_title: "Authentication dialog (secure)".to_string(),
        screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
        note: None,
        description: Some(AUTH_PLACEHOLDER_DESCRIPTION.to_string()),
        description_source: None,
        description_status: None,
        description_error: None,
        ax: None,
        capture_status: None,
        capture_error: None,
        crop_region: None,
    };

    debug_log(
        session,
        &format!(
            "auth_prompt_step: window_id={} bounds=({}, {}, {}x{})",
            auth_window.window_id, bounds.x, bounds.y, bounds.width, bounds.height
        ),
    );

    session.add_step(step.clone());
    (Some(step), true)
}

pub fn record_tray_click(pipeline_state: &Mutex<PipelineState>, rect: TrayRect) {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
    ps.last_tray_click = Some(TrayClick { rect, timestamp_ms });
}

pub fn record_panel_bounds(pipeline_state: &Mutex<PipelineState>, rect: PanelRect) {
    let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
    ps.panel_state.rect = Some(rect);
}

pub fn set_panel_visible(pipeline_state: &Mutex<PipelineState>, visible: bool) {
    let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
    ps.panel_state.visible = visible;
}

pub fn should_filter_tray_click(ps: &PipelineState, click: &ClickEvent) -> bool {
    let Some(tray_click) = ps.last_tray_click else {
        return false;
    };

    let time_diff = (click.timestamp_ms - tray_click.timestamp_ms).abs();
    if time_diff > TRAY_CLICK_WINDOW_MS {
        return false;
    }

    tray_click.rect.contains(click.x, click.y)
}

pub fn should_filter_panel_click(ps: &PipelineState, click: &ClickEvent) -> bool {
    if !ps.panel_state.visible {
        return false;
    }
    let Some(rect) = ps.panel_state.rect else {
        return false;
    };
    rect.contains(click.x, click.y)
}

/// Get main screen dimensions in logical points (not pixels)
#[allow(dead_code)]
pub fn get_main_screen_size() -> (i32, i32) {
    use core_graphics::display::CGDisplay;
    let main = CGDisplay::main();
    // Use bounds() which returns CGRect in logical points, not pixels
    // This is important for Retina displays where pixels != points
    let bounds = main.bounds();
    (bounds.size.width as i32, bounds.size.height as i32)
}

pub fn get_display_bounds_for_click(click_x: i32, click_y: i32) -> (i32, i32, i32, i32) {
    use core_graphics::display::CGDisplay;

    let displays = CGDisplay::active_displays().unwrap_or_default();
    let mut display_bounds = CGDisplay::main().bounds();

    for &disp_id in &displays {
        let disp = CGDisplay::new(disp_id);
        let bounds = disp.bounds();
        let x = bounds.origin.x as i32;
        let y = bounds.origin.y as i32;
        let w = bounds.size.width as i32;
        let h = bounds.size.height as i32;
        let in_display = click_x >= x && click_x < x + w && click_y >= y && click_y < y + h;
        if in_display {
            display_bounds = bounds;
            break;
        }
    }

    (
        display_bounds.origin.x as i32,
        display_bounds.origin.y as i32,
        display_bounds.size.width as i32,
        display_bounds.size.height as i32,
    )
}

/// Find a context menu window near the click position.
/// Context menus are typically: empty title, small layer, appear near the click.
pub fn find_context_menu_near_click(
    click_x: i32,
    click_y: i32,
    app_name: &str,
) -> Option<super::super::window_info::WindowBounds> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;

    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };

    if window_list.is_null() {
        return None;
    }

    let windows: Vec<CFDictionaryRef> = unsafe {
        let count = core_foundation::array::CFArrayGetCount(window_list as _);
        (0..count)
            .map(|i| {
                core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i)
                    as CFDictionaryRef
            })
            .collect()
    };

    for window_dict in windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                window_dict,
            )
        };

        // Get window title - context menus typically have empty titles
        let title_key = CFString::new("kCGWindowName");
        let window_title = dict
            .find(&title_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_default();

        if !window_title.is_empty() {
            continue;
        }

        // Get owner name
        let owner_name_key = CFString::new("kCGWindowOwnerName");
        let owner_name = dict
            .find(&owner_name_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_default();

        // Context menu should be from the same app (use contains for flexibility)
        let owner_lower = owner_name.to_lowercase();
        let app_lower = app_name.to_lowercase();
        if !owner_lower.contains(&app_lower) && !app_lower.contains(&owner_lower) {
            if cfg!(debug_assertions) {
                eprintln!("Context menu search: skipping window from '{owner_name}' (looking for '{app_name}')");
            }
            continue;
        }

        // Get window bounds
        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = match dict.find(&bounds_key) {
            Some(v) => {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> = unsafe {
                    core_foundation::dictionary::CFDictionary::wrap_under_get_rule(
                        v.as_CFTypeRef() as _
                    )
                };

                let x = bounds_dict
                    .find(CFString::new("X"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let y = bounds_dict
                    .find(CFString::new("Y"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let width = bounds_dict
                    .find(CFString::new("Width"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;
                let height = bounds_dict
                    .find(CFString::new("Height"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;

                super::super::window_info::WindowBounds {
                    x,
                    y,
                    width,
                    height,
                }
            }
            None => continue,
        };

        // Context menus are typically narrow, but Finder menus can reach ~500px
        if bounds.width > 600 || bounds.width < 50 || bounds.height < 50 {
            continue;
        }

        // Context menu should be near the click position (within 300px)
        let dx = (bounds.x - click_x).abs();
        let dy = (bounds.y - click_y).abs();
        if dx > 300 || dy > 300 {
            continue;
        }

        if cfg!(debug_assertions) {
            eprintln!(
                "Found context menu near click: bounds=({}, {}, {}x{})",
                bounds.x, bounds.y, bounds.width, bounds.height
            );
        }

        return Some(bounds);
    }

    if cfg!(debug_assertions) {
        eprintln!("No context menu found near click ({click_x}, {click_y}) for app '{app_name}'");
    }
    None
}

/// Check if click should be debounced (too close in time/position to previous)
/// Returns (should_debounce, should_upgrade_previous) - upgrade means replace last Click with DoubleClick
pub fn is_debounced(
    ps: &mut PipelineState,
    timestamp_ms: i64,
    x: i32,
    y: i32,
    click_count: i64,
) -> (bool, bool) {
    if let Some((last_ts, last_x, last_y, last_count)) = ps.last_click {
        let time_diff = timestamp_ms - last_ts;
        let same_position = (x - last_x).abs() < 5 && (y - last_y).abs() < 5;

        // If this is a double-click (click_count=2) at the same position, signal upgrade
        if same_position && click_count > last_count && time_diff < 500 {
            // Update with new click_count
            ps.last_click = Some((timestamp_ms, x, y, click_count));
            return (false, true); // Don't debounce, but upgrade previous step
        }

        // Debounce if click is within threshold time AND at nearly same position AND same click_count
        if time_diff < DEBOUNCE_MS && same_position && click_count == last_count {
            return (true, false);
        }
    }

    // Update last click
    ps.last_click = Some((timestamp_ms, x, y, click_count));
    (false, false)
}

/// Calculate click position as a percentage within a window dimension.
///
/// # Arguments
///
/// * `click_coord` - The absolute screen coordinate of the click
/// * `window_offset` - The window's position on that axis
/// * `window_size` - The window's size on that axis
///
/// # Returns
///
/// The click position as a percentage (0.0 to 100.0), clamped to valid range.
/// Returns 0.0 if window_size is zero or negative.
pub fn calculate_click_percent(click_coord: i32, window_offset: i32, window_size: i32) -> f64 {
    if window_size <= 0 {
        return 0.0;
    }
    let relative = click_coord - window_offset;
    let percent = (relative as f64 / window_size as f64) * 100.0;
    percent.clamp(0.0, 100.0)
}

fn clamp_percent(v: f64) -> f64 {
    v.clamp(0.0, 100.0)
}

/// Compute a default focus crop for large display-like captures.
///
/// This is intentionally conservative:
/// - only for large captures (typically full-screen snapshots),
/// - based on click location (and optional AX element bounds),
/// - leaves smaller/tighter captures untouched.
pub fn suggested_focus_crop_for_capture(
    capture_bounds: &WindowBounds,
    click_x_percent: f64,
    click_y_percent: f64,
    element_bounds_in_capture: Option<&BoundsPercent>,
) -> Option<BoundsPercent> {
    const LARGE_CAPTURE_MIN_W: u32 = 1400;
    const LARGE_CAPTURE_MIN_H: u32 = 800;
    if capture_bounds.width < LARGE_CAPTURE_MIN_W || capture_bounds.height < LARGE_CAPTURE_MIN_H {
        return None;
    }

    let mut center_x = clamp_percent(click_x_percent);
    let mut center_y = clamp_percent(click_y_percent);
    let mut crop_w = 46.0_f64;
    let mut crop_h = 46.0_f64;

    if let Some(bounds) = element_bounds_in_capture {
        let x = clamp_percent(bounds.x_percent as f64);
        let y = clamp_percent(bounds.y_percent as f64);
        let w = clamp_percent(bounds.width_percent as f64);
        let h = clamp_percent(bounds.height_percent as f64);
        if w > 0.0 && h > 0.0 {
            center_x = x + (w / 2.0);
            center_y = y + (h / 2.0);
            crop_w = (w * 3.2).clamp(24.0, 70.0);
            crop_h = (h * 4.0).clamp(22.0, 70.0);
        }
    }

    // Menus/toolbars are often near the top and need wider context.
    if center_y < 18.0 {
        crop_h = crop_h.max(36.0);
        center_y = center_y.max(12.0);
    }

    let mut x = center_x - crop_w / 2.0;
    let mut y = center_y - crop_h / 2.0;
    x = x.clamp(0.0, 100.0 - crop_w);
    y = y.clamp(0.0, 100.0 - crop_h);

    let x = clamp_percent(x);
    let y = clamp_percent(y);
    let crop_w = crop_w.clamp(2.0, 100.0 - x);
    let crop_h = crop_h.clamp(2.0, 100.0 - y);
    if crop_w < 2.0 || crop_h < 2.0 {
        return None;
    }

    Some(BoundsPercent {
        x_percent: x as f32,
        y_percent: y as f32,
        width_percent: crop_w as f32,
        height_percent: crop_h as f32,
    })
}

/// Decide whether an auto focus-crop should be applied for a capture.
///
/// We keep this conservative: only for large captures where full-frame output
/// tends to make UI details too small in tutorial exports.
pub fn should_apply_focus_crop(
    capture_bounds: &WindowBounds,
    display_width: i32,
    display_height: i32,
) -> bool {
    const MIN_CAPTURE_W: u32 = 1100;
    const MIN_CAPTURE_H: u32 = 620;
    const MIN_AREA_RATIO: f64 = 0.55;
    const MIN_WIDTH_RATIO: f64 = 0.75;
    const MIN_HEIGHT_RATIO: f64 = 0.70;

    if capture_bounds.width < MIN_CAPTURE_W || capture_bounds.height < MIN_CAPTURE_H {
        return false;
    }

    let dw = display_width.max(1) as f64;
    let dh = display_height.max(1) as f64;
    let cw = capture_bounds.width as f64;
    let ch = capture_bounds.height as f64;

    let area_ratio = (cw * ch) / (dw * dh);
    let width_ratio = cw / dw;
    let height_ratio = ch / dh;

    area_ratio >= MIN_AREA_RATIO
        || (width_ratio >= MIN_WIDTH_RATIO && height_ratio >= MIN_HEIGHT_RATIO)
}

pub fn bounds_percent_in_capture(
    element: &WindowBounds,
    capture: &WindowBounds,
) -> Option<BoundsPercent> {
    let cw = capture.width as f64;
    let ch = capture.height as f64;
    if cw <= 1.0 || ch <= 1.0 {
        return None;
    }

    let x_pct = (((element.x - capture.x) as f64 / cw) * 100.0).clamp(0.0, 100.0);
    let y_pct = (((element.y - capture.y) as f64 / ch) * 100.0).clamp(0.0, 100.0);
    let w_pct = ((element.width as f64 / cw) * 100.0).clamp(0.0, 100.0);
    let h_pct = ((element.height as f64 / ch) * 100.0).clamp(0.0, 100.0);

    if w_pct <= 0.0 || h_pct <= 0.0 {
        return None;
    }

    Some(BoundsPercent {
        x_percent: x_pct as f32,
        y_percent: y_pct as f32,
        width_percent: w_pct as f32,
        height_percent: h_pct as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_percent_in_capture_basic() {
        let capture = WindowBounds {
            x: 100,
            y: 200,
            width: 1000,
            height: 500,
        };
        let element = WindowBounds {
            x: 200,
            y: 250,
            width: 100,
            height: 50,
        };
        let pct = bounds_percent_in_capture(&element, &capture).expect("bounds percent");
        assert!((pct.x_percent - 10.0).abs() < 0.01);
        assert!((pct.y_percent - 10.0).abs() < 0.01);
        assert!((pct.width_percent - 10.0).abs() < 0.01);
        assert!((pct.height_percent - 10.0).abs() < 0.01);
    }

    #[test]
    fn suggested_focus_crop_only_for_large_captures() {
        let small = WindowBounds {
            x: 0,
            y: 0,
            width: 1200,
            height: 700,
        };
        assert!(suggested_focus_crop_for_capture(&small, 50.0, 50.0, None).is_none());

        let large = WindowBounds {
            x: 0,
            y: 0,
            width: 2560,
            height: 1080,
        };
        let crop = suggested_focus_crop_for_capture(&large, 50.0, 50.0, None)
            .expect("focus crop for large capture");
        assert!(crop.width_percent < 100.0);
        assert!(crop.height_percent < 100.0);
    }

    #[test]
    fn should_apply_focus_crop_for_near_fullscreen_or_large_area() {
        let display_w = 2560;
        let display_h = 1080;

        let near_full = WindowBounds {
            x: 0,
            y: 30,
            width: 2560,
            height: 987,
        };
        assert!(should_apply_focus_crop(&near_full, display_w, display_h));

        let large_dialog_union = WindowBounds {
            x: 600,
            y: 80,
            width: 1585,
            height: 974,
        };
        assert!(should_apply_focus_crop(
            &large_dialog_union,
            display_w,
            display_h
        ));

        let small = WindowBounds {
            x: 900,
            y: 220,
            width: 900,
            height: 560,
        };
        assert!(!should_apply_focus_crop(&small, display_w, display_h));
    }

    #[test]
    fn classify_titleless_overlay_window_dropdown_vs_popup() {
        let bounds_top = WindowBounds {
            x: 100,
            y: 0,
            width: 600,
            height: 500,
        };
        assert_eq!(
            classify_titleless_overlay_window("", 2, 1, &bounds_top, 0),
            TitlelessOverlayKind::DropdownMenu
        );

        let bounds_mid = WindowBounds {
            x: 100,
            y: 200,
            width: 600,
            height: 500,
        };
        assert_eq!(
            classify_titleless_overlay_window("", 2, 1, &bounds_mid, 0),
            TitlelessOverlayKind::Popup
        );

        let bounds_wide = WindowBounds {
            x: 100,
            y: 0,
            width: 1200,
            height: 500,
        };
        assert_eq!(
            classify_titleless_overlay_window("", 2, 1, &bounds_wide, 0),
            TitlelessOverlayKind::NotOverlay
        );

        let bounds_same_id = WindowBounds {
            x: 100,
            y: 0,
            width: 600,
            height: 500,
        };
        assert_eq!(
            classify_titleless_overlay_window("", 1, 1, &bounds_same_id, 0),
            TitlelessOverlayKind::NotOverlay
        );

        let bounds_zero_id = WindowBounds {
            x: 100,
            y: 0,
            width: 600,
            height: 500,
        };
        assert_eq!(
            classify_titleless_overlay_window("", 0, 1, &bounds_zero_id, 0),
            TitlelessOverlayKind::NotOverlay
        );

        // Secondary display offset: still near menu bar on that display.
        let bounds_offset_display = WindowBounds {
            x: 3300,
            y: 460,
            width: 432,
            height: 422,
        };
        assert_eq!(
            classify_titleless_overlay_window("", 2, 1, &bounds_offset_display, 420),
            TitlelessOverlayKind::DropdownMenu
        );
    }

    #[test]
    fn should_use_menu_region_capture_rules() {
        assert!(should_use_menu_region_capture(
            TitlelessOverlayKind::NotOverlay,
            None,
            12,
            false
        ));
        assert!(should_use_menu_region_capture(
            TitlelessOverlayKind::DropdownMenu,
            None,
            220,
            false
        ));
        assert!(should_use_menu_region_capture(
            TitlelessOverlayKind::NotOverlay,
            Some(accessibility_sys::kAXMenuItemRole),
            120,
            false
        ));
        assert!(!should_use_menu_region_capture(
            TitlelessOverlayKind::NotOverlay,
            Some(accessibility_sys::kAXMenuItemRole),
            720,
            false
        ));
        assert!(!should_use_menu_region_capture(
            TitlelessOverlayKind::NotOverlay,
            Some(accessibility_sys::kAXButtonRole),
            120,
            false
        ));
        assert!(should_use_menu_region_capture(
            TitlelessOverlayKind::NotOverlay,
            Some(accessibility_sys::kAXGroupRole),
            140,
            true
        ));
    }

    #[test]
    fn prefer_transient_region_capture_for_menu_and_group_roles() {
        assert!(should_prefer_transient_region_capture(
            Some(accessibility_sys::kAXMenuItemRole),
            TitlelessOverlayKind::NotOverlay,
            false
        ));
        assert!(should_prefer_transient_region_capture(
            Some(accessibility_sys::kAXMenuRole),
            TitlelessOverlayKind::Popup,
            false
        ));
        assert!(should_prefer_transient_region_capture(
            Some(accessibility_sys::kAXGroupRole),
            TitlelessOverlayKind::Popup,
            false
        ));
        assert!(!should_prefer_transient_region_capture(
            Some(accessibility_sys::kAXGroupRole),
            TitlelessOverlayKind::NotOverlay,
            false
        ));
    }

    #[test]
    fn no_transient_region_capture_for_right_click_or_dropdown_menu() {
        assert!(!should_prefer_transient_region_capture(
            Some(accessibility_sys::kAXMenuItemRole),
            TitlelessOverlayKind::NotOverlay,
            true
        ));
        assert!(!should_prefer_transient_region_capture(
            Some(accessibility_sys::kAXGroupRole),
            TitlelessOverlayKind::DropdownMenu,
            false
        ));
    }
}
