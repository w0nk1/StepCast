use std::{fmt, io};

#[derive(Debug)]
pub enum CaptureError {
    InvalidRegion { x: i32, y: i32, w: i32, h: i32 },
    CommandFailed { status: Option<i32>, stderr: String },
    Io(io::Error),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaptureError::InvalidRegion { x, y, w, h } => {
                write!(formatter, "invalid region x={x} y={y} w={w} h={h}")
            }
            CaptureError::CommandFailed { status, stderr } => {
                let code = status.map_or("unknown".to_string(), |value| value.to_string());
                write!(formatter, "screencapture failed (status={code}) {stderr}")
            }
            CaptureError::Io(error) => write!(formatter, "io error: {error}"),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<io::Error> for CaptureError {
    fn from(error: io::Error) -> Self {
        CaptureError::Io(error)
    }
}

#[allow(dead_code)]
pub trait CaptureBackend {
    fn capture_region(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        output: &str,
    ) -> Result<(), CaptureError>;
}
