// Legacy screencapture module - kept for potential future use
#![allow(dead_code)]

use super::capture::CaptureError;
use std::path::Path;
use std::process::Command;

/// Capture the entire screen (main display)
pub fn capture_full_screen(output_path: &Path) -> Result<(), CaptureError> {
    let status = Command::new("screencapture")
        .args([
            "-x", // no sound
            output_path.to_str().unwrap_or("screenshot.png"),
        ])
        .status()
        .map_err(CaptureError::Io)?;

    if !status.success() {
        return Err(CaptureError::CgImage("screencapture failed".to_string()));
    }

    Ok(())
}

pub fn capture_window(window_id: u32, output_path: &Path) -> Result<(), CaptureError> {
    let status = Command::new("screencapture")
        .args([
            "-l",
            &window_id.to_string(),
            "-o", // no shadow
            "-x", // no sound
            output_path.to_str().unwrap_or("screenshot.png"),
        ])
        .status()
        .map_err(CaptureError::Io)?;

    if !status.success() {
        return Err(CaptureError::CgImage("screencapture failed".to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn capture_full_screen_creates_file() {
        let dir = tempdir().expect("tempdir");
        let output = dir.path().join("screenshot.png");

        let result = capture_full_screen(&output);

        // This may fail in headless CI, so we just check it doesn't panic
        if result.is_ok() {
            assert!(output.exists());
        }
    }
}
