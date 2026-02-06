use std::{fmt, io};

#[derive(Debug)]
pub enum CaptureError {
    Io(io::Error),
    CgImage(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaptureError::Io(error) => write!(formatter, "io error: {error}"),
            CaptureError::CgImage(message) => write!(formatter, "capture error: {message}"),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<io::Error> for CaptureError {
    fn from(error: io::Error) -> Self {
        CaptureError::Io(error)
    }
}
