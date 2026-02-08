use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug)]
pub enum WindowError {
    NoFrontmostApp,
    NoWindows,
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowError::NoFrontmostApp => write!(f, "no frontmost application"),
            WindowError::NoWindows => write!(f, "no windows found"),
        }
    }
}

impl std::error::Error for WindowError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub window_id: u32,
    pub bounds: WindowBounds,
}

impl WindowInfo {
    #[cfg(test)]
    pub fn sample() -> Self {
        Self {
            app_name: "Finder".to_string(),
            window_title: "Downloads".to_string(),
            window_id: 12345,
            bounds: WindowBounds {
                x: 100,
                y: 100,
                width: 800,
                height: 600,
            },
        }
    }
}
