use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionType {
    Click,
    Shortcut,
    Note,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub ts: i64,
    pub action: ActionType,
    pub x: i32,
    pub y: i32,
    pub app: String,
    pub window_title: String,
    pub screenshot_path: Option<String>,
    pub note: Option<String>,
}

impl Step {
    #[cfg(test)]
    pub fn sample() -> Self {
        Self {
            id: "step-1".to_string(),
            ts: 0,
            action: ActionType::Click,
            x: 10,
            y: 20,
            app: "Finder".to_string(),
            window_title: "Downloads".to_string(),
            screenshot_path: Some("screenshots/step-001.png".to_string()),
            note: None,
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
