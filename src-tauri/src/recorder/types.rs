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
#[serde(rename_all = "lowercase")]
pub enum DescriptionSource {
    Ai,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DescriptionStatus {
    Idle,
    Generating,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxClickInfo {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    pub label: String,
    /// Bounds of the clicked element within the captured screenshot (percent, origin top-left).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_bounds: Option<BoundsPercent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_level_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_level_subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_dialog_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_dialog_subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_checked: Option<bool>,
    pub is_cancel_button: bool,
    pub is_default_button: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundsPercent {
    pub x_percent: f32,
    pub y_percent: f32,
    pub width_percent: f32,
    pub height_percent: f32,
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
    /// Optional enhanced description (e.g. Apple Intelligence). When absent, exporters fall back to templates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_source: Option<DescriptionSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_status: Option<DescriptionStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_error: Option<String>,
    /// Best-effort Accessibility metadata for grounding descriptions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ax: Option<AxClickInfo>,
    /// How the screenshot capture resolved.  `None` for legacy steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_status: Option<CaptureStatus>,
    /// Human-readable reason when capture_status is Fallback or Failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_error: Option<String>,
    /// Optional non-destructive crop region within the screenshot (percent, origin top-left).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop_region: Option<BoundsPercent>,
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
            description: None,
            description_source: None,
            description_status: None,
            description_error: None,
            ax: None,
            capture_status: None,
            capture_error: None,
            crop_region: None,
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
