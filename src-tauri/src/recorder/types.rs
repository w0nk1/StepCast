use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionType {
    Click,
    DoubleClick,
    RightClick,
    Shortcut,
    Note,
}

/// Status of the screenshot capture for a step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CaptureStatus {
    /// Capture succeeded normally.
    Ok,
    /// Primary capture failed, but fallback succeeded.
    Fallback,
    /// All capture attempts failed – step recorded without a screenshot.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub ts: i64,
    pub action: ActionType,
    pub x: i32,
    pub y: i32,
    pub click_x_percent: f32,
    pub click_y_percent: f32,
    pub app: String,
    pub window_title: String,
    pub screenshot_path: Option<String>,
    pub note: Option<String>,
    /// How the screenshot capture resolved.  `None` for legacy steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_status: Option<CaptureStatus>,
    /// Human-readable reason when capture_status is Fallback or Failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_error: Option<String>,
}

#[cfg(test)]
impl Step {
    pub fn sample() -> Self {
        Self {
            id: "step-1".to_string(),
            ts: 0,
            action: ActionType::Click,
            x: 10,
            y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".to_string(),
            window_title: "Downloads".to_string(),
            screenshot_path: Some("screenshots/step-001.png".to_string()),
            note: None,
            capture_status: None,
            capture_error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_roundtrip_json() {
        let step = Step::sample();
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }
}
