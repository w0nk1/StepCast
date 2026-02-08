//! Capture pipeline that orchestrates click → window info → screenshot → step creation.
//!
//! This module connects all the pieces of the recording flow:
//! - Receives a click event from the click listener
//! - Gets the frontmost window info
//! - Captures a screenshot of that window
//! - Creates a Step with the click position as percentages within the window

use super::capture::CaptureError;
use super::click_event::ClickEvent;
use super::cg_capture::{capture_region_cg, capture_region_fast, capture_window_cg};
use super::macos_screencapture::capture_window as capture_window_by_id;
use super::session::Session;
use super::types::{ActionType, CaptureStatus, Step};
use super::window_info::{
    find_auth_dialog_window,
    find_attached_dialog_window,
    get_frontmost_window,
    get_main_window_for_pid,
    get_security_agent_window,
    get_topmost_window_at_point,
    WindowError,
};

use super::ax_helpers::{
    get_clicked_element_info, get_clicked_element_label, is_security_agent_process,
    is_system_ui_process,
};

use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

/// Minimum time between clicks to avoid duplicates (milliseconds)
const DEBOUNCE_MS: i64 = 150;

/// Cooldown after auth dialog clicks to prevent phantom clicks when dialog closes (milliseconds)
/// This is longer than normal debounce because the phantom click appears at a DIFFERENT position
/// and can occur with significant delay as the dialog animates closed
const AUTH_DIALOG_COOLDOWN_MS: i64 = 800;

const TRAY_CLICK_WINDOW_MS: i64 = 1_000;
const AUTH_PROMPT_DEDUP_MS: i64 = 5_000;

/// All transient pipeline state that should be reset between recording sessions.
///
/// Previously these fields were file-level `static Mutex` values that persisted
/// across sessions.  Wrapping them in a struct stored inside `RecorderAppState`
/// lets us `reset()` cleanly on start / stop / discard.
pub struct PipelineState {
    /// Track last click for debouncing: (timestamp, x, y, click_count)
    pub last_click: Option<(i64, i32, i32, i64)>,
    /// Track last auth dialog click timestamp for extended cooldown
    pub last_auth_click_ms: Option<i64>,
    pub last_tray_click: Option<TrayClick>,
    pub panel_state: PanelState,
    pub last_auth_prompt: Option<(u32, i64)>,
}

impl PipelineState {
    pub fn new() -> Self {
        Self {
            last_click: None,
            last_auth_click_ms: None,
            last_tray_click: None,
            panel_state: PanelState::new(),
            last_auth_prompt: None,
        }
    }

    /// Reset all transient state so a new recording session starts cleanly.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayClick {
    pub rect: TrayRect,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelState {
    pub rect: Option<PanelRect>,
    pub visible: bool,
}

impl PanelState {
    pub const fn new() -> Self {
        Self {
            rect: None,
            visible: false,
        }
    }
}

impl TrayRect {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width
            && y >= self.y
            && y < self.y + self.height
    }
}

impl PanelRect {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width
            && y >= self.y
            && y < self.y + self.height
    }
}

fn debug_log(session: &Session, msg: &str) {
    if !cfg!(debug_assertions) {
        return;
    }

    let log_path = session.temp_dir.join("recording.log");
    let is_new = !log_path.exists();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
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

fn write_auth_placeholder(path: &Path, width: u32, height: u32) -> Result<(), CaptureError> {
    use image::{imageops, Rgba, RgbaImage};

    let w = width.max(120);
    let h = height.max(80);

    let bytes = include_bytes!("assets/coreauth.png");
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

fn capture_region_best(
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
fn validate_screenshot(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => meta.len() > 0,
        Err(_) => false,
    }
}

fn should_emit_auth_prompt(ps: &mut PipelineState, window_id: u32, timestamp_ms: i64) -> bool {
    match ps.last_auth_prompt {
        Some((prev_id, prev_ts)) if prev_id == window_id && timestamp_ms - prev_ts < AUTH_PROMPT_DEDUP_MS => false,
        _ => {
            ps.last_auth_prompt = Some((window_id, timestamp_ms));
            true
        }
    }
}

fn find_security_auth_window(
    click_x: i32,
    click_y: i32,
    clicked_info_missing: bool,
) -> Option<super::window_info::WindowInfo> {
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

fn should_filter_tray_click(ps: &PipelineState, click: &ClickEvent) -> bool {
    let Some(tray_click) = ps.last_tray_click else {
        return false;
    };

    let time_diff = (click.timestamp_ms - tray_click.timestamp_ms).abs();
    if time_diff > TRAY_CLICK_WINDOW_MS {
        return false;
    }

    tray_click.rect.contains(click.x, click.y)
}

fn should_filter_panel_click(ps: &PipelineState, click: &ClickEvent) -> bool {
    if !ps.panel_state.visible {
        return false;
    }
    let Some(rect) = ps.panel_state.rect else {
        return false;
    };
    rect.contains(click.x, click.y)
}

/// Get main screen dimensions in logical points (not pixels)
fn get_main_screen_size() -> (i32, i32) {
    use core_graphics::display::CGDisplay;
    let main = CGDisplay::main();
    // Use bounds() which returns CGRect in logical points, not pixels
    // This is important for Retina displays where pixels != points
    let bounds = main.bounds();
    (bounds.size.width as i32, bounds.size.height as i32)
}

fn get_display_bounds_for_click(click_x: i32, click_y: i32) -> (i32, i32, i32, i32) {
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
fn find_context_menu_near_click(click_x: i32, click_y: i32, app_name: &str) -> Option<super::window_info::WindowBounds> {
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

                super::window_info::WindowBounds { x, y, width, height }
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

/// Errors that can occur during the capture pipeline.
#[derive(Debug)]
pub enum PipelineError {
    /// Failed to get information about the frontmost window.
    WindowInfoFailed(String),
    /// Failed to capture a screenshot.
    ScreenshotFailed(String),
    /// Click was on our own app - should be skipped.
    OwnAppClick,
    /// Click was too soon after previous click (debounced).
    DebouncedClick,
    /// This click upgrades the previous step to DoubleClick (no new step needed).
    UpgradedToDblClick,
    /// Click was a menu open/expand action that shouldn't create a step.
    IgnoredMenuOpen,
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineError::WindowInfoFailed(msg) => write!(f, "window info failed: {msg}"),
            PipelineError::ScreenshotFailed(msg) => write!(f, "screenshot failed: {msg}"),
            PipelineError::OwnAppClick => write!(f, "click on own app"),
            PipelineError::DebouncedClick => write!(f, "click debounced (too fast)"),
            PipelineError::UpgradedToDblClick => write!(f, "upgraded previous step to double-click"),
            PipelineError::IgnoredMenuOpen => write!(f, "ignored menu open click"),
        }
    }
}

impl std::error::Error for PipelineError {}

/// Check if click should be debounced (too close in time/position to previous)
/// Returns (should_debounce, should_upgrade_previous) - upgrade means replace last Click with DoubleClick
fn is_debounced(ps: &mut PipelineState, timestamp_ms: i64, x: i32, y: i32, click_count: i64) -> (bool, bool) {
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

impl From<WindowError> for PipelineError {
    fn from(err: WindowError) -> Self {
        PipelineError::WindowInfoFailed(err.to_string())
    }
}

impl From<CaptureError> for PipelineError {
    fn from(err: CaptureError) -> Self {
        PipelineError::ScreenshotFailed(err.to_string())
    }
}

/// Process a click event and create a step with screenshot.
///
/// This function orchestrates the full capture pipeline:
/// 1. Get frontmost window info
/// 2. Capture screenshot of that window
/// 3. Calculate click position as percentage within window bounds
/// 4. Create and return Step
///
/// # Arguments
///
/// * `click` - The click event to process
/// * `session` - The current recording session (used for step IDs and screenshot paths)
///
/// # Returns
///
/// Returns the created Step on success, or a PipelineError if any step fails.
pub fn process_click(click: &ClickEvent, session: &mut Session, pipeline_state: &Mutex<PipelineState>) -> Result<Step, PipelineError> {
    debug_log(
        session,
        &format!(
            "click: x={} y={} button={:?} count={} ts={}",
            click.x, click.y, click.button, click.click_count, click.timestamp_ms
        ),
    );

    session.diagnostics.clicks_received += 1;

    // Filter clicks on our panel / tray icon
    {
        let ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        if should_filter_panel_click(&ps, click) {
            debug_log(session, "filtered: panel click");
            session.diagnostics.clicks_filtered += 1;
            return Err(PipelineError::OwnAppClick);
        }
        if should_filter_tray_click(&ps, click) {
            debug_log(session, "filtered: tray click");
            session.diagnostics.clicks_filtered += 1;
            return Err(PipelineError::OwnAppClick);
        }
    }

    // 0a. Get info about the actual clicked element
    let clicked_info = get_clicked_element_info(click.x, click.y);
    let clicked_ax = get_clicked_element_label(click.x as f32, click.y as f32);
    if let Some(ax) = clicked_ax.as_ref() {
        debug_log(
            session,
            &format!(
                "ax_click: role={} label='{}' win_role={:?} win_subrole={:?} top_role={:?} top_subrole={:?} parent_role={:?} parent_subrole={:?} cancel={} default={}",
                ax.role,
                ax.label,
                ax.window_role,
                ax.window_subrole,
                ax.top_level_role,
                ax.top_level_subrole,
                ax.parent_dialog_role,
                ax.parent_dialog_subrole,
                ax.is_cancel_button,
                ax.is_default_button
            ),
        );
    }

    // 0b. Filter clicks on our own app using Accessibility API
    if let Some((clicked_pid, ref clicked_app)) = clicked_info {
        let our_pid = std::process::id() as i32;
        let is_own_app = clicked_pid == our_pid || clicked_app.to_lowercase().contains("stepcast");

        if is_own_app {
            debug_log(
                session,
                &format!("filtered: own app click {clicked_app} (PID {clicked_pid})"),
            );
            if cfg!(debug_assertions) {
                eprintln!("Filtered own app click at ({}, {}): {clicked_app} (PID {clicked_pid})", click.x, click.y);
            }
            session.diagnostics.clicks_filtered += 1;
            return Err(PipelineError::OwnAppClick);
        }
    }

    // 0c. Debounce rapid duplicate clicks (but allow double-click upgrades)
    let (should_debounce, should_upgrade) = {
        let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        is_debounced(&mut ps, click.timestamp_ms, click.x, click.y, click.click_count)
    };

    if should_upgrade {
        // This is a double-click - upgrade the previous step
        if let Some(last_step) = session.last_step_mut() {
            if last_step.action == ActionType::Click {
                last_step.action = ActionType::DoubleClick;
                debug_log(session, "upgraded previous step to DoubleClick");
                if cfg!(debug_assertions) {
                    eprintln!("Upgraded previous step to DoubleClick at ({}, {})", click.x, click.y);
                }
                return Err(PipelineError::UpgradedToDblClick);
            }
        }
    }

    if should_debounce {
        debug_log(session, "filtered: debounced click");
        if cfg!(debug_assertions) {
            eprintln!("Debounced click at ({}, {})", click.x, click.y);
        }
        session.diagnostics.clicks_filtered += 1;
        return Err(PipelineError::DebouncedClick);
    }

    // 0d. Check cooldown after auth dialog clicks (phantom click prevention)
    {
        let ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(last_ts) = ps.last_auth_click_ms {
            let time_since_auth = click.timestamp_ms - last_ts;
            if time_since_auth > 0 && time_since_auth < AUTH_DIALOG_COOLDOWN_MS {
                debug_log(
                    session,
                    &format!("filtered: phantom click {time_since_auth}ms after auth dialog"),
                );
                if cfg!(debug_assertions) {
                    eprintln!(
                        "Filtered phantom click at ({}, {}) - {time_since_auth}ms after auth dialog",
                        click.x, click.y
                    );
                }
                session.diagnostics.clicks_filtered += 1;
                return Err(PipelineError::DebouncedClick);
            }
        }
    }

    // Check if click is on a security agent (Touch ID, password dialog)
    // Primary: heuristic window detection, fallback: process name list
    let mut auth_window = if let Some((_, ref clicked_app)) = clicked_info {
        if is_security_agent_process(clicked_app) {
            find_security_auth_window(click.x, click.y, clicked_info.is_none())
        } else {
            None
        }
    } else {
        find_security_auth_window(click.x, click.y, true)
    };

    if auth_window.is_none() {
        if let Ok(Some(window)) = get_security_agent_window() {
            auth_window = Some(window);
            if cfg!(debug_assertions) {
                eprintln!("Auth dialog detected via security agent name fallback");
            }
        }
    }

    let is_auth_dialog = auth_window.is_some()
        || clicked_info
            .as_ref()
            .map(|(_, app)| is_security_agent_process(app))
            .unwrap_or(false);

    if cfg!(debug_assertions) && is_auth_dialog {
        eprintln!("Detected auth dialog click at ({}, {})", click.x, click.y);
    }
    debug_log(
        session,
        &format!(
            "auth_detected={} clicked_info={} auth_window_id={}",
            is_auth_dialog,
            if clicked_info.is_some() { "some" } else { "none" },
            auth_window.as_ref().map(|w| w.window_id).unwrap_or(0)
        ),
    );

    // Record auth dialog click timestamp for phantom click prevention
    if is_auth_dialog {
        let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        ps.last_auth_click_ms = Some(click.timestamp_ms);
    }

    // Fast-path for sheet dialog button clicks: capture immediately around the click.
    // This reduces the chance of capturing the close animation frame.
    let is_sheet_button_click = clicked_ax
        .as_ref()
        .map(|ax| {
            let is_sheet = ax.window_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                || ax.window_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                || ax.window_subrole.as_deref() == Some(accessibility_sys::kAXSystemDialogSubrole)
                || ax.top_level_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                || ax.top_level_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                || ax.top_level_subrole.as_deref() == Some(accessibility_sys::kAXSystemDialogSubrole)
                || ax.parent_dialog_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                || ax.parent_dialog_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                || ax.parent_dialog_subrole.as_deref() == Some(accessibility_sys::kAXSystemDialogSubrole)
                || ax.is_cancel_button
                || ax.is_default_button;
            let is_button = ax.role == accessibility_sys::kAXButtonRole
                || ax.role == accessibility_sys::kAXPopUpButtonRole;
            is_sheet && is_button
        })
        .unwrap_or(false);

    if is_sheet_button_click && !is_auth_dialog {
        let step_id = session.next_step_id();
        let screenshot_path = session.screenshot_path(&step_id);
        let (display_x, display_y, display_w, display_h) = get_display_bounds_for_click(click.x, click.y);
        let preferred_dialog_bounds = clicked_ax.as_ref().and_then(|ax| {
            ax.parent_dialog_bounds
                .clone()
                .or_else(|| ax.top_level_bounds.clone())
                .or_else(|| ax.window_bounds.clone())
        });

        // Prefer AX-derived dialog bounds with tight margins for cleaner output.
        let (mut region_x, mut region_y, region_width, region_height, bounds_source) =
            if let Some(bounds) = preferred_dialog_bounds {
                let mut x = bounds.x - 40;
                let mut y = bounds.y - 52;
                let mut w = bounds.width as i32 + 80;
                let mut h = bounds.height as i32 + 104;

                x = x.max(display_x);
                y = y.max(display_y);
                let right = (x + w).min(display_x + display_w);
                let bottom = (y + h).min(display_y + display_h);
                w = (right - x).max(420);
                h = (bottom - y).max(280);

                (x, y, w, h, "ax_bounds")
            } else {
                let w = display_w.clamp(500, 980);
                let h = display_h.clamp(420, 700);
                // Button is on right/lower part of dialog; bias left/up.
                let mut x = click.x - (w * 3 / 4);
                let mut y = click.y - (h * 3 / 4);
                let max_x = display_x + display_w - w;
                let max_y = display_y + display_h - h;
                x = x.clamp(display_x, max_x.max(display_x));
                y = y.clamp(display_y, max_y.max(display_y));
                (x, y, w, h, "fallback")
            };

        let max_x = display_x + display_w - region_width;
        let max_y = display_y + display_h - region_height;
        region_x = region_x.clamp(display_x, max_x.max(display_x));
        region_y = region_y.clamp(display_y, max_y.max(display_y));

        debug_log(
            session,
            &format!(
                "sheet_fast_path: role={} label='{}' window_role={:?} source={} region=({}, {}, {}x{})",
                clicked_ax.as_ref().map(|a| a.role.as_str()).unwrap_or(""),
                clicked_ax.as_ref().map(|a| a.label.as_str()).unwrap_or(""),
                clicked_ax
                    .as_ref()
                    .and_then(|a| a.window_role.clone())
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.window_subrole.clone()))
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.top_level_role.clone()))
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.top_level_subrole.clone()))
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.parent_dialog_role.clone()))
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.parent_dialog_subrole.clone())),
                bounds_source,
                region_x,
                region_y,
                region_width,
                region_height
            ),
        );

        capture_region_best(
            session,
            region_x,
            region_y,
            region_width,
            region_height,
            &screenshot_path,
        )
        .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

        let click_x_percent =
            ((click.x - region_x) as f64 / region_width as f64 * 100.0).clamp(0.0, 100.0);
        let click_y_percent =
            ((click.y - region_y) as f64 / region_height as f64 * 100.0).clamp(0.0, 100.0);

        use super::click_event::MouseButton;
        let action = match (click.button, click.click_count) {
            (MouseButton::Right, _) => ActionType::RightClick,
            (MouseButton::Left, 2) => ActionType::DoubleClick,
            (MouseButton::Left, n) if n >= 3 => ActionType::DoubleClick,
            _ => ActionType::Click,
        };

        let app_name = clicked_info
            .as_ref()
            .map(|(_, app)| app.clone())
            .unwrap_or_else(|| "Application".to_string());
        let mut window_title = "Dialog".to_string();
        if let Some(ref ax) = clicked_ax {
            if !ax.label.is_empty() {
                window_title = format!("Dialog - {}", ax.label);
            }
        }

        let step = Step {
            id: step_id,
            ts: click.timestamp_ms,
            action,
            x: click.x,
            y: click.y,
            click_x_percent: click_x_percent as f32,
            click_y_percent: click_y_percent as f32,
            app: app_name,
            window_title,
            screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
            note: None,
            capture_status: Some(CaptureStatus::Ok),
            capture_error: None,
        };

        session.add_step(step.clone());
        return Ok(step);
    }

    // 1. Get the main (largest) window of the frontmost app
    let window_info =
        get_frontmost_window().map_err(|e| PipelineError::WindowInfoFailed(format!("{e}")))?;

    // 2. Check if click is on a popup/menu window (only for frontmost app's windows)
    //    We look for smaller overlay windows that belong to the same app
    let topmost_at_click = get_topmost_window_at_point(click.x, click.y);

    // Determine which window to use for capture:
    // - For auth dialogs, use the security agent window
    // - If click is on a regular window from the SAME app (with title), use that window
    // - For popup/menus (empty title, smaller), use the popup window
    // - For system UI (Dock, Spotlight), use the main window
    let mut is_sheet_dialog = false;
    let attached_dialog = if !is_auth_dialog {
        if let Some(ref topmost) = topmost_at_click {
            if topmost.window_id == window_info.window_id {
                find_attached_dialog_window(click.x, click.y, &window_info)
            } else {
                None
            }
        } else {
            find_attached_dialog_window(click.x, click.y, &window_info)
        }
    } else {
        None
    };

    if let Some(ref dialog) = attached_dialog {
        debug_log(
            session,
            &format!(
                "attached_dialog_window: id={} bounds=({}, {}, {}x{}) title='{}' owner='{}'",
                dialog.window_id,
                dialog.bounds.x,
                dialog.bounds.y,
                dialog.bounds.width,
                dialog.bounds.height,
                dialog.window_title,
                dialog.app_name
            ),
        );
    }

    // Track whether capture_window came from topmost overlay detection.
    // When true, don't replace it in the clicked-app reconciliation block —
    // the overlay (e.g. GIF picker, popup) is the correct capture target.
    let mut capture_from_topmost = false;

    let capture_window = if is_auth_dialog {
        // For auth dialogs, prefer heuristic window, fallback to named security agent window
        if let Some(auth_window) = auth_window.clone() {
            debug_log(
                session,
                &format!(
                    "capture_window: auth heuristic id={} bounds=({}, {}, {}x{})",
                    auth_window.window_id,
                    auth_window.bounds.x,
                    auth_window.bounds.y,
                    auth_window.bounds.width,
                    auth_window.bounds.height
                ),
            );
            if cfg!(debug_assertions) {
                eprintln!(
                    "Using heuristic auth window for capture: '{}' id={} bounds=({}, {}, {}x{})",
                    auth_window.app_name, auth_window.window_id,
                    auth_window.bounds.x, auth_window.bounds.y,
                    auth_window.bounds.width, auth_window.bounds.height
                );
            }
            auth_window
        } else if let Ok(Some(auth_window)) = get_security_agent_window() {
            debug_log(
                session,
                &format!(
                    "capture_window: auth fallback id={} bounds=({}, {}, {}x{})",
                    auth_window.window_id,
                    auth_window.bounds.x,
                    auth_window.bounds.y,
                    auth_window.bounds.width,
                    auth_window.bounds.height
                ),
            );
            if cfg!(debug_assertions) {
                eprintln!(
                    "Using security agent window for capture: '{}' id={} bounds=({}, {}, {}x{})",
                    auth_window.app_name, auth_window.window_id,
                    auth_window.bounds.x, auth_window.bounds.y,
                    auth_window.bounds.width, auth_window.bounds.height
                );
            }
            auth_window
        } else {
            // Fallback to window_info if no auth window found
            window_info.clone()
        }
    } else if let Some(dialog) = attached_dialog {
        is_sheet_dialog = true;
        dialog
    } else if let Some(ref topmost) = topmost_at_click {
        let topmost_area = topmost.bounds.width as u64 * topmost.bounds.height as u64;
        let main_area = window_info.bounds.width as u64 * window_info.bounds.height as u64;

        // List of system apps we should NOT use as overlay capture
        // Use process names (language-independent) instead of localized app names
        let is_system_ui = is_system_ui_process(&topmost.app_name);

        // Consider "same app" if the topmost window matches either the frontmost window
        // OR the clicked element's app (from AX API). This handles menu-bar apps where the
        // frontmost window belongs to a different app than the one being clicked.
        let same_app = topmost.app_name.to_lowercase() == window_info.app_name.to_lowercase()
            || clicked_info.as_ref().is_some_and(|(_, clicked_app)| {
                topmost.app_name.to_lowercase() == clicked_app.to_lowercase()
            });
        let is_reasonable_size = topmost.bounds.width >= 50 && topmost.bounds.height >= 20;

        // Regular same-app window WITH a title: use it (handles multiple windows of same app)
        let is_regular_same_app_window = same_app && !topmost.window_title.is_empty();
        // Menu/popup: empty title, smaller than main window, and same app
        let is_menu_popup = same_app && topmost.window_title.is_empty() && topmost_area < main_area;

        if !is_system_ui && is_reasonable_size && (is_regular_same_app_window || is_menu_popup) {
            capture_from_topmost = true;
            if cfg!(debug_assertions) {
                eprintln!(
                    "Using clicked window for capture: '{}' - '{}' (id={}, {}x{})",
                    topmost.app_name, topmost.window_title, topmost.window_id,
                    topmost.bounds.width, topmost.bounds.height
                );
            }
            topmost.clone()
        } else {
            if cfg!(debug_assertions) && topmost.window_id != window_info.window_id {
                eprintln!(
                    "Ignoring topmost window '{}' - '{}' (system_ui={}, same_app={}, area_ratio={:.1}%)",
                    topmost.app_name, topmost.window_title, is_system_ui, same_app,
                    (topmost_area as f64 / main_area as f64) * 100.0
                );
            }
            window_info.clone()
        }
    } else {
        window_info.clone()
    };

    // Use clicked element's app name if available, otherwise captured window's app.
    // When the click targets a different app than the frontmost, also resolve that app's
    // main window so the capture bounds and screenshot match the actual click target.
    // EXCEPTION: when capture_window came from topmost overlay detection (e.g. GIF picker,
    // popup panel), keep it — the overlay is the correct capture target.
    let mut capture_window = capture_window;
    let (actual_app_name, mut actual_window_title) = if let Some((clicked_pid, ref clicked_app)) = clicked_info {
        if clicked_app.to_lowercase() != capture_window.app_name.to_lowercase() && !capture_from_topmost {
            // Try to find the clicked app's main window for correct capture bounds
            if let Some(clicked_window) = get_main_window_for_pid(clicked_pid, clicked_app) {
                if cfg!(debug_assertions) {
                    eprintln!(
                        "Resolved clicked app window: '{}' - '{}' id={} bounds=({}, {}, {}x{})",
                        clicked_window.app_name, clicked_window.window_title,
                        clicked_window.window_id,
                        clicked_window.bounds.x, clicked_window.bounds.y,
                        clicked_window.bounds.width, clicked_window.bounds.height
                    );
                }
                let title = if clicked_window.window_title.is_empty() {
                    format!("Click on {clicked_app}")
                } else {
                    clicked_window.window_title.clone()
                };
                capture_window = clicked_window;
                (clicked_app.clone(), title)
            } else {
                (clicked_app.clone(), format!("Click on {clicked_app}"))
            }
        } else if capture_from_topmost && clicked_app.to_lowercase() != capture_window.app_name.to_lowercase() {
            // Topmost overlay from different app: keep capture window, use clicked_app for label
            if cfg!(debug_assertions) {
                eprintln!(
                    "Keeping topmost overlay for capture (clicked_app='{}' != capture='{}')",
                    clicked_app, capture_window.app_name
                );
            }
            let title = if capture_window.window_title.is_empty() {
                format!("Click on {clicked_app}")
            } else {
                capture_window.window_title.clone()
            };
            (clicked_app.clone(), title)
        } else {
            (capture_window.app_name.clone(), capture_window.window_title.clone())
        }
    } else {
        (capture_window.app_name.clone(), capture_window.window_title.clone())
    };

    let mut resolved_window_title = actual_window_title.clone();

    if cfg!(debug_assertions) {
        eprintln!("Recording click on: {actual_app_name} - {actual_window_title}");
    }

    // 2. Generate step ID and screenshot path
    let step_id = session.next_step_id();
    let screenshot_path = session.screenshot_path(&step_id);
    debug_log(
        session,
        &format!(
            "screenshot_path={} window_id={} title='{}' app='{}'",
            screenshot_path.to_string_lossy(),
            capture_window.window_id,
            capture_window.window_title,
            capture_window.app_name
        ),
    );

    // Check if click is on Dock (system UI at bottom of screen)
    let is_dock_click = if let Some((_, ref clicked_app)) = clicked_info {
        clicked_app.to_lowercase() == "dock"
    } else {
        false
    };

    // Filter out clicks on StepCast itself
    let is_stepcast_click = if let Some((_, ref clicked_app)) = clicked_info {
        let app_lower = clicked_app.to_lowercase();
        app_lower.contains("stepcast") || app_lower.contains("step cast")
    } else {
        false
    };

    if is_stepcast_click {
        if cfg!(debug_assertions) {
            eprintln!("Filtered click on StepCast app");
        }
        session.diagnostics.clicks_filtered += 1;
        return Err(PipelineError::DebouncedClick);
    }

    // Track capture outcome across all branches
    let mut final_capture_status = CaptureStatus::Ok;
    let mut final_capture_error: Option<String> = None;

    // 3. Capture screenshot - special handling for Dock, auth dialogs, then windows
    let (click_x_percent, click_y_percent) = if is_dock_click {
        // Dock click - capture zoomed region, centered on the clicked icon
        use core_graphics::display::CGDisplay;

        // Find the display containing the click
        let displays = CGDisplay::active_displays().unwrap_or_default();
        let mut display_bounds = CGDisplay::main().bounds(); // fallback to main

        for &disp_id in &displays {
            let disp = CGDisplay::new(disp_id);
            let bounds = disp.bounds();
            let in_display = click.x >= bounds.origin.x as i32
                && click.x < (bounds.origin.x + bounds.size.width) as i32
                && click.y >= bounds.origin.y as i32
                && click.y < (bounds.origin.y + bounds.size.height) as i32;
            if in_display {
                display_bounds = bounds;
                break;
            }
        }

        let display_x = display_bounds.origin.x as i32;
        let display_y = display_bounds.origin.y as i32;
        let display_width = display_bounds.size.width as i32;
        let display_height = display_bounds.size.height as i32;

        let region_width = 800;
        let region_height = 150;

        // Calculate click position relative to the display
        let click_rel_x = click.x - display_x;

        // Center the region on the click (but clamp to display bounds)
        let region_rel_x = (click_rel_x - region_width / 2).max(0).min(display_width - region_width);
        let region_x = display_x + region_rel_x;
        let region_y = display_y + display_height - region_height;

        capture_region_best(
            session,
            region_x,
            region_y,
            region_width,
            region_height,
            &screenshot_path,
        )
            .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

        // Calculate click position within the captured region
        let x_pct = ((click_rel_x - region_rel_x) as f64 / region_width as f64 * 100.0).clamp(0.0, 100.0);
        let y_pct = ((click.y - region_y) as f64 / region_height as f64 * 100.0).clamp(0.0, 100.0);
        (x_pct, y_pct)
    } else if is_auth_dialog && capture_window.window_id > 0 {
        // Auth dialogs (Touch ID, password, WireGuard picker) - capture the SPECIFIC window by ID
        // Using screencapture -l ensures we capture only the dialog, not background windows
        let bounds = &capture_window.bounds;

        if cfg!(debug_assertions) {
            eprintln!(
                "Auth dialog detected - window capture by ID: id={}, bounds=({}, {}, {}x{})",
                capture_window.window_id, bounds.x, bounds.y, bounds.width, bounds.height
            );
        }

        let capture_result = capture_window_by_id(capture_window.window_id, &screenshot_path);
        if let Err(err) = capture_result {
            debug_log(
                session,
                &format!("auth_window_capture_failed: {err}"),
            );
            if cfg!(debug_assertions) {
                eprintln!("Auth window capture failed ({err}), falling back to region capture");
            }

            if bounds.width == 0 || bounds.height == 0 {
                return Err(PipelineError::ScreenshotFailed(format!("{err}")));
            }

            write_auth_placeholder(&screenshot_path, bounds.width, bounds.height)
                .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

            actual_window_title = "Authentication dialog (secure)".to_string();
            resolved_window_title = actual_window_title.clone();
            debug_log(session, "auth_placeholder_written");

            let x_pct = calculate_click_percent(click.x, bounds.x, bounds.width as i32);
            let y_pct = calculate_click_percent(click.y, bounds.y, bounds.height as i32);
            (x_pct, y_pct)
        } else {
            debug_log(session, "auth_window_capture: ok");
            // Calculate click position relative to window bounds
            let x_pct = calculate_click_percent(click.x, bounds.x, bounds.width as i32);
            let y_pct = calculate_click_percent(click.y, bounds.y, bounds.height as i32);
            (x_pct, y_pct)
        }
    } else if capture_window.window_id > 0 {
        // Check if click is in menubar region (top ~30 pixels) OR on a dropdown menu
        const MENUBAR_HEIGHT: i32 = 30;
        const DROPDOWN_REGION_HEIGHT: i32 = 500; // Typical max height for dropdown menus

        // Detect if this is a popup/dropdown menu (empty title, different from main window)
        let is_popup_menu = capture_window.window_title.is_empty()
            && capture_window.window_id != window_info.window_id
            && (capture_window.bounds.width as i32) < 800; // Popups are typically narrow

        // Use region capture for: menubar clicks OR dropdown menu clicks in upper screen
        let use_region_capture = click.y < MENUBAR_HEIGHT
            || (is_popup_menu && click.y < DROPDOWN_REGION_HEIGHT);

        if resolved_window_title.is_empty() {
            if is_sheet_dialog {
                resolved_window_title = "Dialog".to_string();
            } else if is_popup_menu || use_region_capture {
                resolved_window_title = "Menu".to_string();
            } else {
                resolved_window_title = "Window".to_string();
            }
        }

        if use_region_capture {
            // Menubar/dropdown click - capture a region around the click
            let region_height = 500; // Include dropdown content
            let region_width = 600;

            // Center horizontally on click (in global coordinates, can be negative)
            let region_x = click.x - region_width / 2;
            // For dropdown clicks, start capture from top of screen to include menubar
            let region_y = 0;

            // Capture the region
            capture_region_best(
                session,
                region_x,
                region_y,
                region_width,
                region_height,
                &screenshot_path,
            )
                .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

            // Calculate click position as percentage within captured region
            let x_pct = ((click.x - region_x) as f64 / region_width as f64 * 100.0).clamp(0.0, 100.0);
            let y_pct = ((click.y - region_y) as f64 / region_height as f64 * 100.0).clamp(0.0, 100.0);

            let step = Step {
                id: step_id,
                ts: click.timestamp_ms,
                action: match (click.button, click.click_count) {
                    (super::click_event::MouseButton::Right, _) => ActionType::RightClick,
                    (super::click_event::MouseButton::Left, 2) => ActionType::DoubleClick,
                    (super::click_event::MouseButton::Left, n) if n >= 3 => ActionType::DoubleClick,
                    _ => ActionType::Click,
                },
                x: click.x,
                y: click.y,
                click_x_percent: x_pct as f32,
                click_y_percent: y_pct as f32,
                app: actual_app_name,
                window_title: resolved_window_title,
                screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
                note: None,
                capture_status: Some(CaptureStatus::Ok),
                capture_error: None,
            };
            session.add_step(step.clone());
            return Ok(step);
        }

        // Check if this is a popup/menu (empty title, smaller than main window)
        let is_popup_menu = capture_window.window_title.is_empty()
            && capture_window.window_id != window_info.window_id;

        // For right-clicks, poll for the context menu to appear.
        // macOS renders context menus asynchronously; a single fixed delay
        // sometimes captures before the menu is visible.  We poll a few
        // times with short sleeps (total max ~250ms) so the screenshot
        // reliably includes the menu.
        let is_right_click = matches!(click.button, super::click_event::MouseButton::Right);
        let context_menu_bounds = if is_right_click && !is_popup_menu {
            if cfg!(debug_assertions) {
                eprintln!("Looking for context menu near click ({}, {}) for app '{}'", click.x, click.y, &capture_window.app_name);
            }
            let mut found = None;
            for attempt in 0..5 {
                std::thread::sleep(std::time::Duration::from_millis(if attempt == 0 { 80 } else { 40 }));
                found = find_context_menu_near_click(click.x, click.y, &capture_window.app_name);
                if found.is_some() {
                    debug_log(
                        session,
                        &format!("context_menu found on attempt {}", attempt + 1),
                    );
                    // Let the menu finish its open animation before measuring final bounds.
                    // Finder menus can be slow to populate (Quick Actions, extensions …).
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    // Re-measure — the menu may have grown during its animation
                    if let Some(refreshed) = find_context_menu_near_click(click.x, click.y, &capture_window.app_name) {
                        found = Some(refreshed);
                    }
                    break;
                }
            }
            found
        } else {
            None
        };

        if resolved_window_title.is_empty() {
            if is_sheet_dialog {
                resolved_window_title = "Dialog".to_string();
            } else if is_popup_menu || context_menu_bounds.is_some() {
                resolved_window_title = "Menu".to_string();
            } else {
                resolved_window_title = "Window".to_string();
            }
        }

        // For popup menus or right-clicks with context menu: use region capture that includes both
        let (use_region_capture, mut actual_bounds) = if is_popup_menu {
            // Calculate union of main window and menu bounds
            let main = &window_info.bounds;
            let menu = &capture_window.bounds;

            let union_x = main.x.min(menu.x);
            let union_y = main.y.min(menu.y);
            let union_right = (main.x + main.width as i32).max(menu.x + menu.width as i32);
            let union_bottom = (main.y + main.height as i32).max(menu.y + menu.height as i32);

            let union_bounds = super::window_info::WindowBounds {
                x: union_x,
                y: union_y,
                width: (union_right - union_x) as u32,
                height: (union_bottom - union_y) as u32,
            };

            if cfg!(debug_assertions) {
                eprintln!(
                    "Popup menu detected - using region capture for window+menu union: ({}, {}, {}x{})",
                    union_bounds.x, union_bounds.y,
                    union_bounds.width, union_bounds.height
                );
            }
            (true, union_bounds)
        } else if let Some(ref menu_bounds) = context_menu_bounds {
            // Right-click with context menu found - include both window and menu.
            // Add generous padding around the menu to account for macOS drop shadows
            // (~20-30px) and items that may render beyond reported window bounds.
            const MENU_PAD: i32 = 50;
            let main = &capture_window.bounds;

            let union_x = main.x.min(menu_bounds.x - MENU_PAD);
            let union_y = main.y.min(menu_bounds.y - MENU_PAD);
            let union_right =
                (main.x + main.width as i32).max(menu_bounds.x + menu_bounds.width as i32 + MENU_PAD);
            let union_bottom =
                (main.y + main.height as i32).max(menu_bounds.y + menu_bounds.height as i32 + MENU_PAD);

            let union_bounds = super::window_info::WindowBounds {
                x: union_x,
                y: union_y,
                width: (union_right - union_x) as u32,
                height: (union_bottom - union_y) as u32,
            };

            if cfg!(debug_assertions) {
                eprintln!(
                    "Right-click with context menu - using union: ({}, {}, {}x{})",
                    union_bounds.x, union_bounds.y,
                    union_bounds.width, union_bounds.height
                );
            }
            (true, union_bounds)
        } else {
            (false, capture_window.bounds.clone())
        };

        // Dialog sheet fallback for parent-window captures:
        // prefer AX window bounds when available, because CGWindow can point to the parent.
        let is_button_click = clicked_ax
            .as_ref()
            .map(|ax| {
                ax.role == accessibility_sys::kAXButtonRole
                    || ax.role == accessibility_sys::kAXPopUpButtonRole
            })
            .unwrap_or(false);
        let is_dialog_marker = clicked_ax
            .as_ref()
            .map(|ax| {
                ax.window_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.window_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.window_subrole.as_deref() == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.top_level_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.top_level_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.top_level_subrole.as_deref() == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.parent_dialog_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.parent_dialog_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.parent_dialog_subrole.as_deref() == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.is_cancel_button
                    || ax.is_default_button
            })
            .unwrap_or(false);
        let ax_window_bounds = clicked_ax.as_ref().and_then(|ax| {
            ax.parent_dialog_bounds
                .clone()
                .or_else(|| ax.top_level_bounds.clone())
                .or_else(|| ax.window_bounds.clone())
        });

        if !is_popup_menu
            && context_menu_bounds.is_none()
            && capture_window.window_id == window_info.window_id
            && is_button_click
        {
            if let Some(ax_bounds) = ax_window_bounds {
                let click_in_ax = click.x >= ax_bounds.x
                    && click.x < ax_bounds.x + ax_bounds.width as i32
                    && click.y >= ax_bounds.y
                    && click.y < ax_bounds.y + ax_bounds.height as i32;

                let ax_left = ax_bounds.x;
                let ax_top = ax_bounds.y;
                let ax_right = ax_bounds.x + ax_bounds.width as i32;
                let ax_bottom = ax_bounds.y + ax_bounds.height as i32;
                let main_left = window_info.bounds.x;
                let main_top = window_info.bounds.y;
                let main_right = window_info.bounds.x + window_info.bounds.width as i32;
                let main_bottom = window_info.bounds.y + window_info.bounds.height as i32;
                let inter_left = ax_left.max(main_left);
                let inter_top = ax_top.max(main_top);
                let inter_right = ax_right.min(main_right);
                let inter_bottom = ax_bottom.min(main_bottom);
                let inter_w = (inter_right - inter_left).max(0) as i64;
                let inter_h = (inter_bottom - inter_top).max(0) as i64;
                let inter_area = inter_w * inter_h;
                let ax_area = (ax_bounds.width as i64) * (ax_bounds.height as i64);
                let overlap_ratio = if ax_area > 0 {
                    inter_area as f32 / ax_area as f32
                } else {
                    0.0
                };
                let larger_than_main =
                    ax_bounds.width > window_info.bounds.width + 24
                        || ax_bounds.height > window_info.bounds.height + 24;
                let should_use_ax_bounds =
                    click_in_ax && overlap_ratio >= 0.25 && (is_dialog_marker || larger_than_main);

                if should_use_ax_bounds {
                    let (display_x, display_y, display_w, display_h) =
                        get_display_bounds_for_click(click.x, click.y);
                    let left = ax_bounds.x.max(display_x);
                    let top = ax_bounds.y.max(display_y);
                    let right = (ax_bounds.x + ax_bounds.width as i32).min(display_x + display_w);
                    let bottom = (ax_bounds.y + ax_bounds.height as i32).min(display_y + display_h);
                    if right > left && bottom > top {
                        debug_log(
                            session,
                            &format!(
                                "ax_window_bounds_override: old=({}, {}, {}x{}) new=({}, {}, {}x{}) overlap={:.2} dialog_marker={} larger={}",
                                actual_bounds.x,
                                actual_bounds.y,
                                actual_bounds.width,
                                actual_bounds.height,
                                left,
                                top,
                                right - left,
                                bottom - top,
                                overlap_ratio,
                                is_dialog_marker,
                                larger_than_main
                            ),
                        );
                        actual_bounds.x = left;
                        actual_bounds.y = top;
                        actual_bounds.width = (right - left) as u32;
                        actual_bounds.height = (bottom - top) as u32;
                        if is_dialog_marker && resolved_window_title == window_info.window_title {
                            resolved_window_title = "Dialog".to_string();
                        }
                    }
                }
            } else if is_dialog_marker {
                // Reserve fallback when AX has dialog markers but no geometry.
                let (display_x, display_y, display_w, display_h) =
                    get_display_bounds_for_click(click.x, click.y);
                let left = (actual_bounds.x - 180).max(display_x);
                let top = (actual_bounds.y - 80).max(display_y);
                let right = (actual_bounds.x + actual_bounds.width as i32 + 180)
                    .min(display_x + display_w);
                let bottom = (actual_bounds.y + actual_bounds.height as i32 + 140)
                    .min(display_y + display_h);
                if right > left && bottom > top {
                    debug_log(
                        session,
                        &format!(
                            "sheet_expand_fallback: old=({}, {}, {}x{}) new=({}, {}, {}x{})",
                            actual_bounds.x,
                            actual_bounds.y,
                            actual_bounds.width,
                            actual_bounds.height,
                            left,
                            top,
                            right - left,
                            bottom - top
                        ),
                    );
                    actual_bounds.x = left;
                    actual_bounds.y = top;
                    actual_bounds.width = (right - left) as u32;
                    actual_bounds.height = (bottom - top) as u32;
                    if resolved_window_title == window_info.window_title {
                        resolved_window_title = "Dialog".to_string();
                    }
                }
            }
        }

        // For normal window captures (no popup/context menu), try window-ID capture first.
        // This avoids race conditions where the window closes before the region capture
        // (e.g., clicking a close button). Fall back to region capture on failure.
        let mut capture_ok = false;
        let mut used_fallback = false;
        let mut last_capture_err: Option<String> = None;

        if !use_region_capture && context_menu_bounds.is_none() && capture_window.window_id > 0 {
            if cfg!(debug_assertions) {
                eprintln!(
                    "Trying window-ID capture: id={} bounds=({}, {}, {}x{})",
                    capture_window.window_id,
                    actual_bounds.x, actual_bounds.y,
                    actual_bounds.width, actual_bounds.height,
                );
            }
            match capture_window_cg(capture_window.window_id, &screenshot_path) {
                Ok(()) if validate_screenshot(&screenshot_path) => {
                    debug_log(
                        session,
                        &format!("window_id_capture ok: id={}", capture_window.window_id),
                    );
                    capture_ok = true;
                }
                Ok(()) => {
                    debug_log(
                        session,
                        &format!("window_id_capture produced empty file, falling back to region"),
                    );
                    last_capture_err = Some("window capture produced empty file".to_string());
                }
                Err(err) => {
                    if cfg!(debug_assertions) {
                        eprintln!("Window-ID capture failed ({err}), falling back to region");
                    }
                    debug_log(
                        session,
                        &format!("window_id_capture failed: {err}, falling back to region"),
                    );
                    last_capture_err = Some(format!("{err}"));
                }
            }
        }

        if !capture_ok {
            if cfg!(debug_assertions) {
                eprintln!(
                    "Region capture: bounds=({}, {}, {}x{}) popup={}",
                    actual_bounds.x, actual_bounds.y,
                    actual_bounds.width, actual_bounds.height,
                    use_region_capture
                );
            }
            match capture_region_best(
                session,
                actual_bounds.x,
                actual_bounds.y,
                actual_bounds.width as i32,
                actual_bounds.height as i32,
                &screenshot_path,
            ) {
                Ok(()) if validate_screenshot(&screenshot_path) => {
                    if last_capture_err.is_some() {
                        used_fallback = true;
                    }
                    capture_ok = true;
                }
                Ok(()) => {
                    debug_log(session, "region_capture produced empty file");
                    last_capture_err = Some(last_capture_err.unwrap_or_default()
                        + "; region capture produced empty file");
                }
                Err(e) => {
                    debug_log(session, &format!("region_capture failed: {e}"));
                    let msg = format!("{e}");
                    last_capture_err = Some(
                        last_capture_err
                            .map(|prev| format!("{prev}; {msg}"))
                            .unwrap_or(msg),
                    );
                }
            }
        }

        // Record capture outcome
        if capture_ok && used_fallback {
            final_capture_status = CaptureStatus::Fallback;
            final_capture_error = last_capture_err.clone();
            session.diagnostics.captures_fallback += 1;
            if let Some(ref reason) = last_capture_err {
                session.diagnostics.failure_reasons.push(reason.clone());
            }
        } else if !capture_ok {
            final_capture_status = CaptureStatus::Failed;
            final_capture_error = last_capture_err.clone();
            session.diagnostics.captures_failed += 1;
            if let Some(ref reason) = last_capture_err {
                session.diagnostics.failure_reasons.push(reason.clone());
            }
        }

        if cfg!(debug_assertions) {
            eprintln!(
                "Click calc: click=({}, {}), capture_bounds=(x={}, y={}, w={}, h={})",
                click.x, click.y,
                actual_bounds.x, actual_bounds.y,
                actual_bounds.width, actual_bounds.height
            );
        }

        // Calculate click position relative to the CAPTURED window bounds
        let x_pct = calculate_click_percent(
            click.x,
            actual_bounds.x,
            actual_bounds.width as i32,
        );
        let y_pct = calculate_click_percent(
            click.y,
            actual_bounds.y,
            actual_bounds.height as i32,
        );

        if cfg!(debug_assertions) {
            eprintln!("Click percent: x={x_pct}%, y={y_pct}%");
        }

        (x_pct, y_pct)
    } else {
        // No valid window - check if click is in menubar/dropdown region or use fullscreen
        let (screen_width, screen_height) = get_main_screen_size();

        // Menubar and dropdown region (top portion of screen)
        // This covers: menubar clicks, dropdown menus, status menu popups
        const MENUBAR_REGION_HEIGHT: i32 = 500;

        if click.y < MENUBAR_REGION_HEIGHT {
            // Use fixed region size and center on click (in global coordinates)
            // Global coordinates can be negative for displays left of primary
            let region_width = 800;
            let region_height = MENUBAR_REGION_HEIGHT.min(screen_height);

            // Center horizontally on click (can be negative for multi-monitor)
            let region_x = click.x - region_width / 2;

            if cfg!(debug_assertions) {
                eprintln!(
                    "Top-region click at ({}, {}), using focused capture ({}, 0, {}x{})",
                    click.x, click.y, region_x, region_width, region_height
                );
            }

            capture_region_best(
                session,
                region_x,
                0,
                region_width,
                region_height,
                &screenshot_path,
            )
                .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

            let x_pct = if region_width > 0 {
                ((click.x - region_x) as f64 / region_width as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                50.0
            };
            let y_pct = if region_height > 0 {
                (click.y as f64 / region_height as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                50.0
            };
            (x_pct, y_pct)
        } else {
            // Fullscreen capture for clicks in lower screen area without window
            if cfg!(debug_assertions) {
                eprintln!("No valid window_id, using fullscreen capture");
            }
            capture_region_best(session, 0, 0, screen_width, screen_height, &screenshot_path)
                .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

            let x_pct = if screen_width > 0 {
                (click.x as f64 / screen_width as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                50.0
            };
            let y_pct = if screen_height > 0 {
                (click.y as f64 / screen_height as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                50.0
            };
            (x_pct, y_pct)
        }
    };

    if resolved_window_title.is_empty() {
        if is_sheet_dialog {
            resolved_window_title = "Dialog".to_string();
        } else {
            resolved_window_title = "Window".to_string();
        }
    }

    if !is_auth_dialog {
        if let Some(ax_label) = clicked_ax {
            let role = ax_label.role.as_str();
            let label = ax_label.label;
            let is_menu_item = role == accessibility_sys::kAXMenuItemRole;
            let is_menu_container = role == accessibility_sys::kAXMenuBarItemRole
                || role == accessibility_sys::kAXMenuButtonRole;
            let is_button = role == accessibility_sys::kAXButtonRole
                || role == accessibility_sys::kAXPopUpButtonRole;

            if is_menu_container && !is_menu_item {
                debug_log(
                    session,
                    &format!("ignored menu open: role={role} label='{label}'"),
                );
                return Err(PipelineError::IgnoredMenuOpen);
            }

            if is_menu_item {
                resolved_window_title = format!("Menu - {label}");
            } else if is_button {
                if resolved_window_title == "Window" || resolved_window_title == "Menu" {
                    resolved_window_title = format!("Button - {label}");
                } else if !resolved_window_title.contains(&label) {
                    resolved_window_title = format!("{resolved_window_title} - {label}");
                }
            }
        }
    }

    // 6. Determine action type based on click count and button
    use super::click_event::MouseButton;
    let action = match (click.button, click.click_count) {
        (MouseButton::Right, _) => ActionType::RightClick,
        (MouseButton::Left, 2) => ActionType::DoubleClick,
        (MouseButton::Left, n) if n >= 3 => ActionType::DoubleClick, // Triple+ as double
        _ => ActionType::Click,
    };

    // 7. Create step
    let screenshot = if final_capture_status == CaptureStatus::Failed {
        None
    } else {
        Some(screenshot_path.to_string_lossy().to_string())
    };
    let step = Step {
        id: step_id,
        ts: click.timestamp_ms,
        action,
        x: click.x,
        y: click.y,
        click_x_percent: click_x_percent as f32,
        click_y_percent: click_y_percent as f32,
        app: actual_app_name,
        window_title: resolved_window_title,
        screenshot_path: screenshot,
        note: None,
        capture_status: Some(final_capture_status),
        capture_error: final_capture_error,
    };

    // 8. Add to session
    session.add_step(step.clone());

    Ok(step)
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
fn calculate_click_percent(click_coord: i32, window_offset: i32, window_size: i32) -> f64 {
    if window_size <= 0 {
        return 0.0;
    }
    let relative = click_coord - window_offset;
    let percent = (relative as f64 / window_size as f64) * 100.0;
    percent.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::click_event::{ClickEvent, MouseButton};

    #[test]
    fn calculate_click_percent_at_origin() {
        // Click at window origin should be 0%
        let percent = calculate_click_percent(100, 100, 800);
        assert!((percent - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_at_end() {
        // Click at window edge should be 100%
        let percent = calculate_click_percent(900, 100, 800);
        assert!((percent - 100.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_in_middle() {
        // Click in middle should be 50%
        let percent = calculate_click_percent(500, 100, 800);
        assert!((percent - 50.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_handles_zero_size() {
        // Zero window size should return 0
        let percent = calculate_click_percent(100, 50, 0);
        assert!((percent - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_handles_negative_size() {
        // Negative window size should return 0
        let percent = calculate_click_percent(100, 50, -10);
        assert!((percent - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_clamps_below_zero() {
        // Click before window should clamp to 0%
        let percent = calculate_click_percent(50, 100, 800);
        assert!((percent - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_clamps_above_hundred() {
        // Click after window should clamp to 100%
        let percent = calculate_click_percent(1000, 100, 800);
        assert!((percent - 100.0).abs() < 0.001);
    }

    #[test]
    fn pipeline_error_displays_correctly() {
        let err = PipelineError::WindowInfoFailed("no app".to_string());
        assert!(err.to_string().contains("window info failed"));
        assert!(err.to_string().contains("no app"));

        let err = PipelineError::ScreenshotFailed("capture failed".to_string());
        assert!(err.to_string().contains("screenshot failed"));
        assert!(err.to_string().contains("capture failed"));
    }

    // Note: is_click_on_own_app uses the Accessibility API and requires
    // actual UI elements to test, so we can't easily unit test it.
    // It's tested manually by running the app.

    // --- TrayRect::contains ---

    #[test]
    fn tray_rect_contains_point_inside() {
        let rect = TrayRect { x: 100, y: 0, width: 30, height: 22 };
        assert!(rect.contains(115, 11));
    }

    #[test]
    fn tray_rect_contains_top_left_corner() {
        let rect = TrayRect { x: 100, y: 0, width: 30, height: 22 };
        assert!(rect.contains(100, 0));
    }

    #[test]
    fn tray_rect_excludes_bottom_right_edge() {
        let rect = TrayRect { x: 100, y: 0, width: 30, height: 22 };
        // x < x + width, y < y + height (exclusive)
        assert!(!rect.contains(130, 22));
    }

    #[test]
    fn tray_rect_excludes_point_outside() {
        let rect = TrayRect { x: 100, y: 0, width: 30, height: 22 };
        assert!(!rect.contains(50, 10));
        assert!(!rect.contains(200, 10));
        assert!(!rect.contains(115, 30));
    }

    // --- PanelRect::contains ---

    #[test]
    fn panel_rect_contains_point_inside() {
        let rect = PanelRect { x: 50, y: 30, width: 340, height: 554 };
        assert!(rect.contains(200, 300));
    }

    #[test]
    fn panel_rect_excludes_point_outside() {
        let rect = PanelRect { x: 50, y: 30, width: 340, height: 554 };
        assert!(!rect.contains(0, 300));
        assert!(!rect.contains(400, 300));
        assert!(!rect.contains(200, 600));
    }

    // --- is_debounced ---

    #[test]
    fn first_click_is_not_debounced() {
        let mut ps = PipelineState::new();
        let (debounced, upgrade) = is_debounced(&mut ps, 1000, 100, 200, 1);
        assert!(!debounced);
        assert!(!upgrade);
    }

    #[test]
    fn same_position_within_threshold_is_debounced() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1050, 102, 201, 1);
        assert!(debounced);
        assert!(!upgrade);
    }

    #[test]
    fn same_position_after_threshold_is_not_debounced() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1200, 102, 201, 1);
        assert!(!debounced);
        assert!(!upgrade);
    }

    #[test]
    fn different_position_within_threshold_is_not_debounced() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1050, 200, 300, 1);
        assert!(!debounced);
        assert!(!upgrade);
    }

    #[test]
    fn double_click_upgrades_previous() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1100, 101, 201, 2);
        assert!(!debounced);
        assert!(upgrade);
    }

    #[test]
    fn double_click_at_different_position_does_not_upgrade() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1100, 200, 300, 2);
        assert!(!debounced);
        assert!(!upgrade);
    }

    #[test]
    fn double_click_after_timeout_does_not_upgrade() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1600, 101, 201, 2);
        assert!(!debounced);
        assert!(!upgrade);
    }

    // --- PipelineState::reset ---

    #[test]
    fn pipeline_state_reset_clears_all() {
        let mut ps = PipelineState::new();
        ps.last_click = Some((1000, 100, 200, 1));
        ps.last_auth_click_ms = Some(500);
        ps.last_tray_click = Some(TrayClick {
            rect: TrayRect { x: 0, y: 0, width: 30, height: 22 },
            timestamp_ms: 999,
        });
        ps.panel_state.visible = true;
        ps.panel_state.rect = Some(PanelRect { x: 50, y: 30, width: 340, height: 554 });
        ps.last_auth_prompt = Some((42, 1000));

        ps.reset();

        assert!(ps.last_click.is_none());
        assert!(ps.last_auth_click_ms.is_none());
        assert!(ps.last_tray_click.is_none());
        assert!(!ps.panel_state.visible);
        assert!(ps.panel_state.rect.is_none());
        assert!(ps.last_auth_prompt.is_none());
    }

    // --- Negative coordinates (multi-monitor) ---

    #[test]
    fn calculate_click_percent_negative_offsets() {
        // Secondary monitor left of primary: window at x=-1440, click at x=-720
        let percent = calculate_click_percent(-720, -1440, 1440);
        assert!((percent - 50.0).abs() < 0.001);
    }

    #[test]
    fn debounce_handles_negative_coords() {
        let mut ps = PipelineState::new();
        let (d, u) = is_debounced(&mut ps, 1000, -500, -200, 1);
        assert!(!d);
        assert!(!u);
        // Same position within threshold
        let (d2, _) = is_debounced(&mut ps, 1050, -498, -199, 1);
        assert!(d2);
    }

    // --- should_filter_tray_click with PipelineState ---

    #[test]
    fn filter_tray_click_within_window() {
        let mut ps = PipelineState::new();
        ps.last_tray_click = Some(TrayClick {
            rect: TrayRect { x: 100, y: 0, width: 30, height: 22 },
            timestamp_ms: 1000,
        });
        let click = ClickEvent {
            x: 115,
            y: 11,
            button: MouseButton::Left,
            click_count: 1,
            timestamp_ms: 1500,
        };
        assert!(should_filter_tray_click(&ps, &click));
    }

    #[test]
    fn filter_tray_click_expired() {
        let mut ps = PipelineState::new();
        ps.last_tray_click = Some(TrayClick {
            rect: TrayRect { x: 100, y: 0, width: 30, height: 22 },
            timestamp_ms: 1000,
        });
        let click = ClickEvent {
            x: 115,
            y: 11,
            button: MouseButton::Left,
            click_count: 1,
            timestamp_ms: 3000, // > 1s after tray click
        };
        assert!(!should_filter_tray_click(&ps, &click));
    }

    // --- should_filter_panel_click with PipelineState ---

    #[test]
    fn filter_panel_click_visible() {
        let mut ps = PipelineState::new();
        ps.panel_state.visible = true;
        ps.panel_state.rect = Some(PanelRect { x: 50, y: 30, width: 340, height: 554 });
        let click = ClickEvent {
            x: 200,
            y: 300,
            button: MouseButton::Left,
            click_count: 1,
            timestamp_ms: 1000,
        };
        assert!(should_filter_panel_click(&ps, &click));
    }

    #[test]
    fn filter_panel_click_hidden() {
        let mut ps = PipelineState::new();
        ps.panel_state.visible = false;
        ps.panel_state.rect = Some(PanelRect { x: 50, y: 30, width: 340, height: 554 });
        let click = ClickEvent {
            x: 200,
            y: 300,
            button: MouseButton::Left,
            click_count: 1,
            timestamp_ms: 1000,
        };
        assert!(!should_filter_panel_click(&ps, &click));
    }

    // --- should_emit_auth_prompt dedup ---

    #[test]
    fn auth_prompt_first_emission() {
        let mut ps = PipelineState::new();
        assert!(should_emit_auth_prompt(&mut ps, 42, 1000));
        assert_eq!(ps.last_auth_prompt, Some((42, 1000)));
    }

    #[test]
    fn auth_prompt_dedup_same_window_within_cooldown() {
        let mut ps = PipelineState::new();
        assert!(should_emit_auth_prompt(&mut ps, 42, 1000));
        // Same window, within AUTH_PROMPT_DEDUP_MS (5000ms)
        assert!(!should_emit_auth_prompt(&mut ps, 42, 3000));
    }

    #[test]
    fn auth_prompt_emits_after_cooldown() {
        let mut ps = PipelineState::new();
        assert!(should_emit_auth_prompt(&mut ps, 42, 1000));
        // Same window, after AUTH_PROMPT_DEDUP_MS
        assert!(should_emit_auth_prompt(&mut ps, 42, 7000));
    }

    #[test]
    fn auth_prompt_emits_for_different_window() {
        let mut ps = PipelineState::new();
        assert!(should_emit_auth_prompt(&mut ps, 42, 1000));
        // Different window ID, even within cooldown
        assert!(should_emit_auth_prompt(&mut ps, 99, 2000));
    }

    // --- validate_screenshot ---

    #[test]
    fn validate_screenshot_nonexistent() {
        assert!(!validate_screenshot(Path::new("/tmp/definitely_not_a_real_file_xyz.png")));
    }

    #[test]
    fn validate_screenshot_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.png");
        std::fs::write(&path, b"").unwrap();
        assert!(!validate_screenshot(&path));
    }

    #[test]
    fn validate_screenshot_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("valid.png");
        std::fs::write(&path, b"PNG data here").unwrap();
        assert!(validate_screenshot(&path));
    }
}
