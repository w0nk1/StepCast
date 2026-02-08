use crate::recorder::types::{ActionType, Step};
use base64::Engine;
use std::fs;
use std::io::Read as _;

/// Check if a step represents an authentication placeholder
pub fn is_auth_placeholder(step: &Step) -> bool {
    step.window_title == "Authentication dialog (secure)"
        || step.app.to_lowercase() == "authentication"
}

/// Human-readable description of what happened in a step
pub fn action_description(step: &Step) -> String {
    if is_auth_placeholder(step) {
        return "Authentication required (secure dialog)".to_string();
    }

    match step.action {
        ActionType::Note => "Note".to_string(),
        _ => {
            let verb = match step.action {
                ActionType::DoubleClick => "Double-clicked in",
                ActionType::RightClick => "Right-clicked in",
                ActionType::Shortcut => "Used keyboard shortcut in",
                _ => "Clicked in",
            };
            format!("{} {} \u{2014} \"{}\"", verb, step.app, step.window_title)
        }
    }
}

/// Load a screenshot file and return its base64-encoded content
pub fn load_screenshot_base64(path: &str) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(&buffer))
}

/// Escape HTML special characters
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Convert a title to a filesystem-safe slug
#[allow(dead_code)]
pub fn slugify_title(title: &str) -> String {
    slug::slugify(title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::types::{ActionType, Step};

    fn sample_step() -> Step {
        Step {
            id: "s1".into(),
            ts: 0,
            action: ActionType::Click,
            x: 10,
            y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".into(),
            window_title: "Downloads".into(),
            screenshot_path: None,
            note: None,
            capture_status: None,
            capture_error: None,
        }
    }

    #[test]
    fn action_description_click() {
        let s = sample_step();
        assert_eq!(action_description(&s), "Clicked in Finder \u{2014} \"Downloads\"");
    }

    #[test]
    fn action_description_double_click() {
        let mut s = sample_step();
        s.action = ActionType::DoubleClick;
        assert_eq!(action_description(&s), "Double-clicked in Finder \u{2014} \"Downloads\"");
    }

    #[test]
    fn action_description_right_click() {
        let mut s = sample_step();
        s.action = ActionType::RightClick;
        assert_eq!(action_description(&s), "Right-clicked in Finder \u{2014} \"Downloads\"");
    }

    #[test]
    fn action_description_shortcut() {
        let mut s = sample_step();
        s.action = ActionType::Shortcut;
        assert_eq!(action_description(&s), "Used keyboard shortcut in Finder \u{2014} \"Downloads\"");
    }

    #[test]
    fn action_description_note() {
        let mut s = sample_step();
        s.action = ActionType::Note;
        s.note = Some("Remember to save".into());
        assert_eq!(action_description(&s), "Note");
    }

    #[test]
    fn action_description_auth_placeholder() {
        let mut s = sample_step();
        s.window_title = "Authentication dialog (secure)".into();
        assert_eq!(action_description(&s), "Authentication required (secure dialog)");
    }

    #[test]
    fn action_description_auth_by_app() {
        let mut s = sample_step();
        s.app = "Authentication".into();
        assert_eq!(action_description(&s), "Authentication required (secure dialog)");
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn html_escape_quotes() {
        assert_eq!(html_escape(r#"say "hello""#), "say &quot;hello&quot;");
    }

    #[test]
    fn slugify_title_basic() {
        assert_eq!(slugify_title("My Guide Title"), "my-guide-title");
    }

    #[test]
    fn slugify_title_umlauts() {
        let result = slugify_title("\u{00c4}rger mit \u{00d6}lf\u{00f6}rderung");
        assert!(!result.contains('\u{00e4}'));
        assert!(!result.contains('\u{00f6}'));
        assert!(result.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn slugify_title_special_chars() {
        assert_eq!(slugify_title("Hello World! (2026)"), "hello-world-2026");
    }

    #[test]
    fn is_auth_placeholder_checks() {
        let mut s = sample_step();
        assert!(!is_auth_placeholder(&s));

        s.window_title = "Authentication dialog (secure)".into();
        assert!(is_auth_placeholder(&s));

        s.window_title = "Normal".into();
        s.app = "Authentication".into();
        assert!(is_auth_placeholder(&s));
    }
}
