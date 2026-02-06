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
    /// Click count from CGEvent (1 = single, 2 = double, 3 = triple)
    pub click_count: i64,
}

impl ClickEvent {
    pub fn new(x: i32, y: i32, button: MouseButton, click_count: i64) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Self {
            x,
            y,
            timestamp_ms,
            button,
            click_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_event_creates_with_timestamp() {
        let event = ClickEvent::new(100, 200, MouseButton::Left, 1);
        assert_eq!(event.x, 100);
        assert_eq!(event.y, 200);
        assert_eq!(event.click_count, 1);
        assert!(event.timestamp_ms > 0);
    }

    #[test]
    fn click_event_double_click() {
        let event = ClickEvent::new(100, 200, MouseButton::Left, 2);
        assert_eq!(event.click_count, 2);
    }
}
