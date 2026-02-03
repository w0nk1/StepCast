pub mod capture;
pub mod click_event;
pub mod macos_screencapture;
pub mod state;
pub mod storage;
pub mod types;
pub mod window_info;

pub struct Recorder;

impl Recorder {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::Recorder;

    #[test]
    fn recorder_new_constructs() {
        let _recorder = Recorder::new();
    }
}
