// Some functions are kept for potential future use
#![allow(dead_code, unused_imports)]

mod auth;
mod query;
mod topmost;
mod types;

pub use auth::{find_auth_dialog_window, get_security_agent_window};
pub use query::{
    get_frontmost_window, get_main_window_for_pid, get_window_at_click, get_window_for_pid_at_click,
};
pub use topmost::{find_attached_dialog_window, get_topmost_window_at_point};
pub use types::{WindowBounds, WindowError, WindowInfo};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_info_serializes() {
        let info = WindowInfo::sample();
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("Finder"));
        assert!(json.contains("Downloads"));
    }

    // Regression: get_topmost_window_at_point must skip system UI windows (Dock, etc.)
    // so overlay windows like GIF pickers are found instead of being shadowed.
    // We can't call get_topmost_window_at_point in a unit test (needs live CGWindows),
    // but we verify the filter function correctly identifies system UI processes.
    #[test]
    fn system_ui_filter_catches_dock() {
        assert!(super::super::ax_helpers::is_system_ui_process("Dock"));
        assert!(super::super::ax_helpers::is_system_ui_process("dock"));
        assert!(super::super::ax_helpers::is_system_ui_process(
            "WindowServer"
        ));
        assert!(super::super::ax_helpers::is_system_ui_process(
            "SystemUIServer"
        ));
        assert!(super::super::ax_helpers::is_system_ui_process(
            "ControlCenter"
        ));
    }

    #[test]
    fn system_ui_filter_allows_normal_apps() {
        assert!(!super::super::ax_helpers::is_system_ui_process("WhatsApp"));
        assert!(!super::super::ax_helpers::is_system_ui_process("Safari"));
        assert!(!super::super::ax_helpers::is_system_ui_process("Finder"));
    }
}
