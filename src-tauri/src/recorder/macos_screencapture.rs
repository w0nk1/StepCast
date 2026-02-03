use std::process::Command;

use super::capture::{CaptureBackend, CaptureError};

pub struct MacOsScreencapture;

pub fn build_args(x: i32, y: i32, w: i32, h: i32, output: &str) -> Vec<String> {
    vec![
        "-x".to_string(),
        "-R".to_string(),
        format!("{},{},{},{}", x, y, w, h),
        output.to_string(),
    ]
}

impl CaptureBackend for MacOsScreencapture {
    fn capture_region(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        output: &str,
    ) -> Result<(), CaptureError> {
        if w <= 0 || h <= 0 {
            return Err(CaptureError::InvalidRegion { x, y, w, h });
        }

        let output = Command::new("screencapture")
            .args(build_args(x, y, w, h, output))
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(CaptureError::CommandFailed {
                status: output.status.code(),
                stderr,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_region_args() {
        let args = build_args(10, 20, 300, 200, "/tmp/a.png");
        assert!(args.contains(&"-R".to_string()));
    }
}
