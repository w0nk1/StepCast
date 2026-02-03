//! Capture pipeline that orchestrates click → window info → screenshot → step creation.
//!
//! This module connects all the pieces of the recording flow:
//! - Receives a click event from the click listener
//! - Gets the frontmost window info
//! - Captures a screenshot of that window
//! - Creates a Step with the click position as percentages within the window

use super::capture::CaptureError;
use super::click_event::ClickEvent;
use super::macos_screencapture::capture_window;
use super::session::Session;
use super::types::{ActionType, Step};
use super::window_info::{get_frontmost_window, WindowError};

use std::fmt;

/// Errors that can occur during the capture pipeline.
#[derive(Debug)]
pub enum PipelineError {
    /// Failed to get information about the frontmost window.
    WindowInfoFailed(String),
    /// Failed to capture a screenshot.
    ScreenshotFailed(String),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineError::WindowInfoFailed(msg) => write!(f, "window info failed: {}", msg),
            PipelineError::ScreenshotFailed(msg) => write!(f, "screenshot failed: {}", msg),
        }
    }
}

impl std::error::Error for PipelineError {}

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
    // 1. Get frontmost window
    let window_info =
        get_frontmost_window().map_err(|e| PipelineError::WindowInfoFailed(format!("{}", e)))?;

    // 2. Generate step ID and screenshot path
    let step_id = session.next_step_id();
    let screenshot_path = session.screenshot_path(&step_id);

    // 3. Capture window screenshot
    capture_window(window_info.window_id, &screenshot_path)
        .map_err(|e| PipelineError::ScreenshotFailed(format!("{}", e)))?;

    // 4. Calculate click position as percentage within window bounds
    let click_x_percent = calculate_click_percent(
        click.x,
        window_info.bounds.x,
        window_info.bounds.width as i32,
    );
    let click_y_percent = calculate_click_percent(
        click.y,
        window_info.bounds.y,
        window_info.bounds.height as i32,
    );

    // 5. Create step
    let step = Step {
        id: step_id,
        ts: click.timestamp_ms,
        action: ActionType::Click,
        x: click.x,
        y: click.y,
        click_x_percent: click_x_percent as f32,
        click_y_percent: click_y_percent as f32,
        app: window_info.app_name,
        window_title: window_info.window_title,
        screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
        note: None,
    };

    // 6. Add to session
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
}
