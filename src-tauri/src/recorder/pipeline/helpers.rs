//! Pipeline helper functions: capture, filtering, debouncing, context menu detection.

use super::super::capture::CaptureError;
use super::super::cg_capture::{capture_region_cg, capture_region_fast};
use super::super::click_event::ClickEvent;
use super::super::session::Session;
use super::super::types::{ActionType, Step};
use super::super::window_info::find_auth_dialog_window;
use super::super::ax_helpers::{
    get_clicked_element_info, is_security_agent_process,
};
use super::types::*;

use std::path::Path;
use std::sync::Mutex;

pub fn debug_log(session: &Session, msg: &str) {
    if !cfg!(debug_assertions) {
        return;
    }

    let log_path = session.temp_dir.join("recording.log");
    let is_new = !log_path.exists();
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        use std::io::Write;
        if is_new {
            let _ = writeln!(
                file,
                "session_dir={}",
                session.temp_dir.to_string_lossy()
            );
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
    let img = image::load_from_memory(bytes).map_err(|e| {
        CaptureError::CgImage(format!("placeholder decode failed: {e}"))
    })?;

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
                &format!(
                    "fast_region_capture ok: x={x} y={y} w={width} h={height}",
                ),
            );
            Ok(())
        }
        Err(err) => {
            debug_log(
                session,
                &format!(
                    "fast_region_capture failed: {err} (x={x} y={y} w={width} h={height})",
                ),
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
        Some((prev_id, prev_ts)) if prev_id == window_id && timestamp_ms - prev_ts < AUTH_PROMPT_DEDUP_MS => false,
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
)-> (Option<Step>, bool) {
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
        capture_status: None,
        capture_error: None,
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
pub fn find_context_menu_near_click(click_x: i32, click_y: i32, app_name: &str) -> Option<super::super::window_info::WindowBounds> {
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
            .map(|i| core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef)
            .collect()
    };

    for window_dict in windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(window_dict)
        };

        // Get window title - context menus typically have empty titles
        let title_key = CFString::new("kCGWindowName");
        let window_title = dict.find(&title_key)
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
        let owner_name = dict.find(&owner_name_key)
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
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(v.as_CFTypeRef() as _) };

                let x = bounds_dict.find(CFString::new("X")).and_then(|n| n.to_i32()).unwrap_or(0);
                let y = bounds_dict.find(CFString::new("Y")).and_then(|n| n.to_i32()).unwrap_or(0);
                let width = bounds_dict.find(CFString::new("Width")).and_then(|n| n.to_i32()).unwrap_or(0) as u32;
                let height = bounds_dict.find(CFString::new("Height")).and_then(|n| n.to_i32()).unwrap_or(0) as u32;

                super::super::window_info::WindowBounds { x, y, width, height }
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
pub fn is_debounced(ps: &mut PipelineState, timestamp_ms: i64, x: i32, y: i32, click_count: i64) -> (bool, bool) {
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
