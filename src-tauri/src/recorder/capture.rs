use std::io;

pub enum CaptureError {
    InvalidRegion { x: i32, y: i32, w: i32, h: i32 },
    CommandFailed { status: Option<i32>, stderr: String },
    Io(io::Error),
}

impl From<io::Error> for CaptureError {
    fn from(error: io::Error) -> Self {
        CaptureError::Io(error)
    }
}

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
