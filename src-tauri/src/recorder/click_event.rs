use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickEvent {
    pub x: i32,
    pub y: i32,
    pub timestamp_ms: i64,
    pub button: MouseButton,
}

impl ClickEvent {
    pub fn new(x: i32, y: i32, button: MouseButton) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Self {
            x,
            y,
            timestamp_ms,
            button,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_event_creates_with_timestamp() {
        let event = ClickEvent::new(100, 200, MouseButton::Left);
        assert_eq!(event.x, 100);
        assert_eq!(event.y, 200);
        assert!(event.timestamp_ms > 0);
    }
}
