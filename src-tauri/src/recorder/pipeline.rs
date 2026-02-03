//! Capture pipeline that orchestrates click → window info → screenshot → step creation.
//!
//! This module connects all the pieces of the recording flow:
//! - Receives a click event from the click listener
//! - Gets the frontmost window info
//! - Captures a screenshot of that window
//! - Creates a Step with the click position as percentages within the window

use super::capture::CaptureError;
use super::click_event::ClickEvent;
use super::macos_screencapture::{capture_full_screen, capture_window};
use super::session::Session;
use super::types::{ActionType, Step};
use super::window_info::{get_frontmost_window, WindowError};

use std::fmt;
use std::sync::Mutex;

/// Minimum time between clicks to avoid duplicates (milliseconds)
const DEBOUNCE_MS: i64 = 150;

/// Track last click for debouncing
static LAST_CLICK: Mutex<Option<(i64, i32, i32)>> = Mutex::new(None);

/// Get the PID of the UI element at the given screen position using Accessibility API.
/// Returns None if no element found or on error.
fn get_pid_at_position(x: f32, y: f32) -> Option<i32> {
    use accessibility_sys::{
        AXUIElementCopyElementAtPosition, AXUIElementCreateSystemWide, AXUIElementGetPid,
    };

    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return None;
        }

        let mut element: accessibility_sys::AXUIElementRef = std::ptr::null_mut();
        let result = AXUIElementCopyElementAtPosition(system_wide, x, y, &mut element);

        // Release system_wide
        core_foundation::base::CFRelease(system_wide as *const _);

        if result != 0 || element.is_null() {
            return None;
        }

        let mut pid: i32 = 0;
        let pid_result = AXUIElementGetPid(element, &mut pid);

        // Release element
        core_foundation::base::CFRelease(element as *const _);

        if pid_result == 0 {
            Some(pid)
        } else {
            None
        }
    }
}

/// Get process name for a PID using ps command
fn get_process_name(pid: i32) -> Option<String> {
    use std::process::Command;
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

/// Get a friendly app name from a process path or name
fn get_friendly_app_name(proc_path: &str) -> String {
    // Extract app name from path like "/System/Library/CoreServices/Dock.app/Contents/MacOS/Dock"
    if let Some(app_part) = proc_path.split('/').find(|s| s.ends_with(".app")) {
        return app_part.trim_end_matches(".app").to_string();
    }
    // Fallback: just use the last component
    proc_path.split('/').last().unwrap_or(proc_path).to_string()
}

/// Get the PID and app name of the element at click position
fn get_clicked_element_info(x: i32, y: i32) -> Option<(i32, String)> {
    let pid = get_pid_at_position(x as f32, y as f32)?;
    let proc_name = get_process_name(pid)?;
    let friendly_name = get_friendly_app_name(&proc_name);
    Some((pid, friendly_name))
}

/// Get main screen dimensions
fn get_main_screen_size() -> (i32, i32) {
    use core_graphics::display::CGDisplay;
    let main = CGDisplay::main();
    (main.pixels_wide() as i32, main.pixels_high() as i32)
}

/// Check if the click position is over our own app
fn is_click_on_own_app(x: i32, y: i32) -> bool {
    let our_pid = std::process::id() as i32;

    if let Some(clicked_pid) = get_pid_at_position(x as f32, y as f32) {
        // Direct PID match
        if clicked_pid == our_pid {
            if cfg!(debug_assertions) {
                eprintln!("Click at ({}, {}): PID {} matches our PID", x, y, clicked_pid);
            }
            return true;
        }

        // Check if process name contains "stepcast" (for child processes like WebView)
        if let Some(proc_name) = get_process_name(clicked_pid) {
            let proc_lower = proc_name.to_lowercase();
            if proc_lower.contains("stepcast") {
                if cfg!(debug_assertions) {
                    eprintln!("Click at ({}, {}): PID {} is '{}' (our child process)", x, y, clicked_pid, proc_name);
                }
                return true;
            }
            if cfg!(debug_assertions) {
                eprintln!("Click at ({}, {}): PID {} is '{}' (not our app)", x, y, clicked_pid, proc_name);
            }
        }
    }

    false
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
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineError::WindowInfoFailed(msg) => write!(f, "window info failed: {}", msg),
            PipelineError::ScreenshotFailed(msg) => write!(f, "screenshot failed: {}", msg),
            PipelineError::OwnAppClick => write!(f, "click on own app"),
            PipelineError::DebouncedClick => write!(f, "click debounced (too fast)"),
        }
    }
}

impl std::error::Error for PipelineError {}

/// Check if click should be debounced (too close in time/position to previous)
fn is_debounced(timestamp_ms: i64, x: i32, y: i32) -> bool {
    let mut last = LAST_CLICK.lock().unwrap();

    if let Some((last_ts, last_x, last_y)) = *last {
        let time_diff = timestamp_ms - last_ts;
        let same_position = (x - last_x).abs() < 5 && (y - last_y).abs() < 5;

        // Debounce if click is within threshold time AND at nearly same position
        if time_diff < DEBOUNCE_MS && same_position {
            return true;
        }
    }

    // Update last click
    *last = Some((timestamp_ms, x, y));
    false
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
pub fn process_click(click: &ClickEvent, session: &mut Session) -> Result<Step, PipelineError> {
    // 0a. Get info about the actual clicked element
    let clicked_info = get_clicked_element_info(click.x, click.y);

    // 0b. Filter clicks on our own app using Accessibility API
    if let Some((clicked_pid, ref clicked_app)) = clicked_info {
        let our_pid = std::process::id() as i32;
        let is_own_app = clicked_pid == our_pid || clicked_app.to_lowercase().contains("stepcast");

        if is_own_app {
            if cfg!(debug_assertions) {
                eprintln!("Filtered own app click at ({}, {}): {} (PID {})", click.x, click.y, clicked_app, clicked_pid);
            }
            return Err(PipelineError::OwnAppClick);
        }
    }

    // 0c. Debounce rapid duplicate clicks
    if is_debounced(click.timestamp_ms, click.x, click.y) {
        if cfg!(debug_assertions) {
            eprintln!("Debounced click at ({}, {})", click.x, click.y);
        }
        return Err(PipelineError::DebouncedClick);
    }

    // 1. Get the main (largest) window of the frontmost app for screenshot
    let window_info =
        get_frontmost_window().map_err(|e| PipelineError::WindowInfoFailed(format!("{}", e)))?;

    // Use clicked element's app name if available, otherwise frontmost window's app
    let (actual_app_name, actual_window_title) = if let Some((_, ref clicked_app)) = clicked_info {
        // If click was on a different app than frontmost (e.g., Dock), use clicked app's name
        if clicked_app.to_lowercase() != window_info.app_name.to_lowercase() {
            (clicked_app.clone(), format!("Click on {}", clicked_app))
        } else {
            (window_info.app_name.clone(), window_info.window_title.clone())
        }
    } else {
        (window_info.app_name.clone(), window_info.window_title.clone())
    };

    if cfg!(debug_assertions) {
        eprintln!("Recording click on: {} - {}", actual_app_name, actual_window_title);
    }

    // 2. Generate step ID and screenshot path
    let step_id = session.next_step_id();
    let screenshot_path = session.screenshot_path(&step_id);

    // 3. Capture screenshot - window if available, fullscreen as fallback (e.g., dock clicks)
    let (click_x_percent, click_y_percent) = if window_info.window_id > 0 {
        // Window capture - captures the main window including any modals/sheets on top
        if cfg!(debug_assertions) {
            eprintln!(
                "Capturing window_id={} bounds=({}, {}, {}x{})",
                window_info.window_id,
                window_info.bounds.x, window_info.bounds.y,
                window_info.bounds.width, window_info.bounds.height
            );
        }
        capture_window(window_info.window_id, &screenshot_path)
            .map_err(|e| PipelineError::ScreenshotFailed(format!("{}", e)))?;

        if cfg!(debug_assertions) {
            eprintln!(
                "Click calc: click=({}, {}), window_bounds=(x={}, y={}, w={}, h={})",
                click.x, click.y,
                window_info.bounds.x, window_info.bounds.y,
                window_info.bounds.width, window_info.bounds.height
            );
        }

        let x_pct = calculate_click_percent(
            click.x,
            window_info.bounds.x,
            window_info.bounds.width as i32,
        );
        let y_pct = calculate_click_percent(
            click.y,
            window_info.bounds.y,
            window_info.bounds.height as i32,
        );

        if cfg!(debug_assertions) {
            eprintln!("Click percent: x={}%, y={}%", x_pct, y_pct);
        }

        (x_pct, y_pct)
    } else {
        // Fullscreen capture for dock/menu bar clicks - click position relative to screen
        if cfg!(debug_assertions) {
            eprintln!("No valid window_id, using fullscreen capture");
        }
        capture_full_screen(&screenshot_path)
            .map_err(|e| PipelineError::ScreenshotFailed(format!("{}", e)))?;

        let (screen_width, screen_height) = get_main_screen_size();
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
    };

    // 6. Create step
    let step = Step {
        id: step_id,
        ts: click.timestamp_ms,
        action: ActionType::Click,
        x: click.x,
        y: click.y,
        click_x_percent: click_x_percent as f32,
        click_y_percent: click_y_percent as f32,
        app: actual_app_name,
        window_title: actual_window_title,
        screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
        note: None,
    };

    // 7. Add to session
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
}
