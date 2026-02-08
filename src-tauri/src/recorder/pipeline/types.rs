//! Pipeline types, state, and error definitions.

use std::fmt;

use super::super::capture::CaptureError;
use super::super::window_info::WindowError;

/// Minimum time between clicks to avoid duplicates (milliseconds)
pub const DEBOUNCE_MS: i64 = 150;

/// Cooldown after auth dialog clicks to prevent phantom clicks when dialog closes (milliseconds)
/// This is longer than normal debounce because the phantom click appears at a DIFFERENT position
/// and can occur with significant delay as the dialog animates closed
pub const AUTH_DIALOG_COOLDOWN_MS: i64 = 800;

pub const TRAY_CLICK_WINDOW_MS: i64 = 1_000;
pub const AUTH_PROMPT_DEDUP_MS: i64 = 5_000;

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
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width
            && y >= self.y
            && y < self.y + self.height
    }
}

impl PanelRect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width
            && y >= self.y
            && y < self.y + self.height
    }
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
