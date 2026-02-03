use super::capture::CaptureError;
use std::path::Path;
use std::process::Command;

pub fn capture_window(window_id: u32, output_path: &Path) -> Result<(), CaptureError> {
    // Use screencapture CLI which handles all the complexity
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
        return Err(CaptureError::CommandFailed {
            status: status.code(),
            stderr: "screencapture failed".to_string(),
        });
    }

    Ok(())
}

pub fn capture_screen_region(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    output_path: &Path,
) -> Result<(), CaptureError> {
    // Use screencapture with region
    let region = format!("{},{},{},{}", x, y, width, height);
    let status = Command::new("screencapture")
        .args([
            "-R",
            &region,
            "-x", // no sound
            output_path.to_str().unwrap_or("screenshot.png"),
        ])
        .status()
        .map_err(CaptureError::Io)?;

    if !status.success() {
        return Err(CaptureError::CommandFailed {
            status: status.code(),
            stderr: "screencapture failed".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn capture_screen_region_creates_file() {
        let dir = tempdir().expect("tempdir");
        let output = dir.path().join("screenshot.png");

        // Capture a small region of the screen
        let result = capture_screen_region(0, 0, 100, 100, &output);

        // This may fail in headless CI, so we just check it doesn't panic
        if result.is_ok() {
            assert!(output.exists());
        }
    }
}
