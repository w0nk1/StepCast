//! Accessibility API helpers for querying UI elements at screen positions.
//!
//! Uses macOS Accessibility (AX) and CoreFoundation APIs to introspect
//! clicked elements, resolve window/dialog roles, and identify processes.

use super::window_info::WindowBounds;

/// RAII guard for CoreFoundation objects. Calls `CFRelease` on drop.
struct CfRef(*mut std::ffi::c_void);

impl CfRef {
    /// Wrap a raw CF pointer. Returns `None` if null.
    fn wrap(ptr: *mut std::ffi::c_void) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }

    /// Get the raw pointer (e.g. to pass to CF/AX functions).
    fn as_ptr(&self) -> *mut std::ffi::c_void {
        self.0
    }

    /// Reinterpret as a specific CF type pointer.
    fn as_type<T>(&self) -> *mut T {
        self.0 as *mut T
    }
}

impl Drop for CfRef {
    fn drop(&mut self) {
        unsafe {
            core_foundation::base::CFRelease(self.0 as *const _);
        }
    }
}

/// Get the PID of the UI element at the given screen position using Accessibility API.
/// Returns None if no element found or on error.
pub(super) fn get_pid_at_position(x: f32, y: f32) -> Option<i32> {
    use accessibility_sys::{
        AXUIElementCopyElementAtPosition, AXUIElementCreateSystemWide, AXUIElementGetPid,
    };

    unsafe {
        let system_wide = CfRef::wrap(AXUIElementCreateSystemWide() as *mut _)?;

        let mut element: accessibility_sys::AXUIElementRef = std::ptr::null_mut();
        let result =
            AXUIElementCopyElementAtPosition(system_wide.as_type(), x, y, &mut element);
        if result != 0 {
            return None;
        }
        let element = CfRef::wrap(element as *mut _)?;

        let mut pid: i32 = 0;
        let pid_result = AXUIElementGetPid(element.as_type(), &mut pid);

        if pid_result == 0 {
            Some(pid)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct AxElementLabel {
    pub role: String,
    pub label: String,
    pub window_role: Option<String>,
    pub window_subrole: Option<String>,
    pub window_bounds: Option<WindowBounds>,
    pub top_level_role: Option<String>,
    pub top_level_subrole: Option<String>,
    pub top_level_bounds: Option<WindowBounds>,
    pub parent_dialog_role: Option<String>,
    pub parent_dialog_subrole: Option<String>,
    pub parent_dialog_bounds: Option<WindowBounds>,
    pub is_cancel_button: bool,
    pub is_default_button: bool,
}

fn ax_copy_string_attr(
    element: accessibility_sys::AXUIElementRef,
    attr_name: &str,
) -> Option<String> {
    use accessibility_sys::AXUIElementCopyAttributeValue;
    use core_foundation::base::{CFGetTypeID, CFTypeRef, TCFType};
    use core_foundation::string::CFString;

    unsafe {
        let attr = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let result = AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value);
        if result != 0 {
            return None;
        }
        let guard = CfRef::wrap(value as *mut _)?;

        if CFGetTypeID(guard.as_ptr() as _) == CFString::type_id() {
            // CfRef owns the reference; wrap_under_get_rule borrows it with a temporary retain
            let s = CFString::wrap_under_get_rule(guard.as_ptr() as _).to_string();
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                return None;
            }
            return Some(trimmed);
        }

        None
    }
}

fn ax_copy_element_attr(
    element: accessibility_sys::AXUIElementRef,
    attr_name: &str,
) -> Option<CfRef> {
    use accessibility_sys::AXUIElementCopyAttributeValue;
    use core_foundation::base::{CFTypeRef, TCFType};
    use core_foundation::string::CFString;

    unsafe {
        let attr = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let result = AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value);
        if result != 0 {
            return None;
        }
        CfRef::wrap(value as *mut _)
    }
}

fn ax_copy_value_attr(
    element: accessibility_sys::AXUIElementRef,
    attr_name: &str,
) -> Option<CfRef> {
    use accessibility_sys::AXUIElementCopyAttributeValue;
    use core_foundation::base::{CFTypeRef, TCFType};
    use core_foundation::string::CFString;

    unsafe {
        let attr = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let result = AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value);
        if result != 0 {
            return None;
        }
        CfRef::wrap(value as *mut _)
    }
}

fn ax_copy_window_bounds(
    window_element: accessibility_sys::AXUIElementRef,
) -> Option<WindowBounds> {
    use accessibility_sys::{
        AXValueGetType, AXValueGetValue, kAXPositionAttribute, kAXSizeAttribute,
        kAXValueTypeCGPoint, kAXValueTypeCGSize,
    };
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::ffi::c_void;

    unsafe {
        let pos_guard = ax_copy_value_attr(window_element, kAXPositionAttribute)?;
        let size_guard = ax_copy_value_attr(window_element, kAXSizeAttribute)?;

        let pos_ref = pos_guard.as_type::<accessibility_sys::__AXValue>();
        let size_ref = size_guard.as_type::<accessibility_sys::__AXValue>();

        if AXValueGetType(pos_ref) != kAXValueTypeCGPoint
            || AXValueGetType(size_ref) != kAXValueTypeCGSize
        {
            return None;
        }

        let mut pos = CGPoint::new(0.0, 0.0);
        let mut size = CGSize::new(0.0, 0.0);
        let ok_pos = AXValueGetValue(
            pos_ref,
            kAXValueTypeCGPoint,
            (&mut pos as *mut CGPoint).cast::<c_void>(),
        );
        let ok_size = AXValueGetValue(
            size_ref,
            kAXValueTypeCGSize,
            (&mut size as *mut CGSize).cast::<c_void>(),
        );

        if !ok_pos || !ok_size {
            return None;
        }

        let width = size.width.round() as i32;
        let height = size.height.round() as i32;
        if width <= 1 || height <= 1 {
            return None;
        }

        Some(WindowBounds {
            x: pos.x.round() as i32,
            y: pos.y.round() as i32,
            width: width as u32,
            height: height as u32,
        })
    }
}

fn ax_element_matches_attr_element(
    container: accessibility_sys::AXUIElementRef,
    attr_name: &str,
    element: accessibility_sys::AXUIElementRef,
) -> bool {
    let Some(candidate) = ax_copy_element_attr(container, attr_name) else {
        return false;
    };
    unsafe {
        core_foundation::base::CFEqual(candidate.as_ptr() as *const _, element as *const _) != 0
    }
}

fn ax_find_dialog_parent(
    element: accessibility_sys::AXUIElementRef,
) -> (
    Option<String>,
    Option<String>,
    Option<WindowBounds>,
) {
    use accessibility_sys::{kAXParentAttribute, kAXRoleAttribute, kAXSubroleAttribute};

    let mut current_raw = element;
    let mut current_guard: Option<CfRef> = None;

    for _ in 0..10 {
        let role = ax_copy_string_attr(current_raw, kAXRoleAttribute);
        let subrole = ax_copy_string_attr(current_raw, kAXSubroleAttribute);
        let is_dialog = role.as_deref() == Some(accessibility_sys::kAXSheetRole)
            || subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
            || subrole.as_deref() == Some(accessibility_sys::kAXSystemDialogSubrole);

        if is_dialog {
            let bounds = ax_copy_window_bounds(current_raw);
            return (role, subrole, bounds);
        }

        let Some(parent) = ax_copy_element_attr(current_raw, kAXParentAttribute) else {
            break;
        };
        current_raw = parent.as_type();
        // Previous guard is dropped automatically, new one takes ownership
        current_guard = Some(parent);
    }
    drop(current_guard);

    (None, None, None)
}

/// Get role + label of the UI element at the given screen position using Accessibility API.
pub(super) fn get_clicked_element_label(x: f32, y: f32) -> Option<AxElementLabel> {
    use accessibility_sys::{
        AXUIElementCopyElementAtPosition, AXUIElementCreateSystemWide, kAXDescriptionAttribute,
        kAXCancelButtonAttribute, kAXDefaultButtonAttribute,
        kAXTopLevelUIElementAttribute,
        kAXRoleAttribute, kAXSubroleAttribute, kAXTitleAttribute, kAXValueAttribute,
        kAXValueDescriptionAttribute,
        kAXWindowAttribute,
    };

    unsafe {
        let system_wide = CfRef::wrap(AXUIElementCreateSystemWide() as *mut _)?;

        let mut raw_element: accessibility_sys::AXUIElementRef = std::ptr::null_mut();
        let result =
            AXUIElementCopyElementAtPosition(system_wide.as_type(), x, y, &mut raw_element);
        if result != 0 {
            return None;
        }
        let element = CfRef::wrap(raw_element as *mut _)?;
        let el: accessibility_sys::AXUIElementRef = element.as_type();

        let role = ax_copy_string_attr(el, kAXRoleAttribute);
        let label = ax_copy_string_attr(el, kAXTitleAttribute)
            .or_else(|| ax_copy_string_attr(el, kAXValueDescriptionAttribute))
            .or_else(|| ax_copy_string_attr(el, kAXValueAttribute))
            .or_else(|| ax_copy_string_attr(el, kAXDescriptionAttribute));

        let (window_role, window_subrole, window_bounds, is_cancel_button, is_default_button) =
            ax_copy_element_attr(el, kAXWindowAttribute)
                .map(|window_guard| {
                    let w: accessibility_sys::AXUIElementRef = window_guard.as_type();
                    let role = ax_copy_string_attr(w, kAXRoleAttribute);
                    let subrole = ax_copy_string_attr(w, kAXSubroleAttribute);
                    let bounds = ax_copy_window_bounds(w);
                    let is_cancel =
                        ax_element_matches_attr_element(w, kAXCancelButtonAttribute, el);
                    let is_default =
                        ax_element_matches_attr_element(w, kAXDefaultButtonAttribute, el);
                    (role, subrole, bounds, is_cancel, is_default)
                })
                .unwrap_or((None, None, None, false, false));

        let (top_level_role, top_level_subrole, top_level_bounds, top_level_cancel, top_level_default) =
            ax_copy_element_attr(el, kAXTopLevelUIElementAttribute)
                .map(|top_guard| {
                    let t: accessibility_sys::AXUIElementRef = top_guard.as_type();
                    let role = ax_copy_string_attr(t, kAXRoleAttribute);
                    let subrole = ax_copy_string_attr(t, kAXSubroleAttribute);
                    let bounds = ax_copy_window_bounds(t);
                    let is_cancel =
                        ax_element_matches_attr_element(t, kAXCancelButtonAttribute, el);
                    let is_default =
                        ax_element_matches_attr_element(t, kAXDefaultButtonAttribute, el);
                    (role, subrole, bounds, is_cancel, is_default)
                })
                .unwrap_or((None, None, None, false, false));

        let (parent_dialog_role, parent_dialog_subrole, parent_dialog_bounds) =
            ax_find_dialog_parent(el);

        match (role, label) {
            (Some(role), Some(label)) => Some(AxElementLabel {
                role,
                label,
                window_role,
                window_subrole,
                window_bounds,
                top_level_role,
                top_level_subrole,
                top_level_bounds,
                parent_dialog_role,
                parent_dialog_subrole,
                parent_dialog_bounds,
                is_cancel_button: is_cancel_button || top_level_cancel,
                is_default_button: is_default_button || top_level_default,
            }),
            _ => None,
        }
    }
}

/// Get process name for a PID using ps command
pub(super) fn get_process_name(pid: i32) -> Option<String> {
    use std::process::Command;
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

/// Get a friendly app name from a process path or name
pub(super) fn get_friendly_app_name(proc_path: &str) -> String {
    // Extract app name from path like "/System/Library/CoreServices/Dock.app/Contents/MacOS/Dock"
    if let Some(app_part) = proc_path.split('/').find(|s| s.ends_with(".app")) {
        return app_part.trim_end_matches(".app").to_string();
    }
    // Fallback: just use the last component
    proc_path.split('/').next_back().unwrap_or(proc_path).to_string()
}

/// Check if a process name belongs to a system authentication agent (Touch ID, password dialogs)
pub(super) fn is_security_agent_process(proc_name: &str) -> bool {
    let name = proc_name.to_lowercase();
    name.contains("securityagent")
        || name.contains("coreauth")
        || name.contains("userauthenticationdialog")
        || name.contains("localauthentication")
}

/// Check if a window belongs to system UI that should not be used as overlay capture.
/// Uses actual process names (from `ps`) which are language-independent.
pub(super) fn is_system_ui_process(process_name: &str) -> bool {
    let name = process_name.to_lowercase();

    // macOS system UI process names (executable names, NOT localized)
    name == "dock"
        || name == "spotlight"
        || name == "windowserver"
        || name == "systemuiserver"
        || name == "notificationcenterui"
        || name == "controlcenter"
        || name == "control center"  // Sometimes has space
        // Contains checks for variations
        || name.contains("systemuiserver")
        || name.contains("controlcenter")
        || name.contains("notificationcenter")
}

/// Get the PID and app name of the element at click position
pub(super) fn get_clicked_element_info(x: i32, y: i32) -> Option<(i32, String)> {
    let pid = get_pid_at_position(x as f32, y as f32)?;
    let proc_name = get_process_name(pid)?;
    let friendly_name = get_friendly_app_name(&proc_name);
    Some((pid, friendly_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- get_friendly_app_name ---

    #[test]
    fn friendly_name_from_app_bundle_path() {
        let name = get_friendly_app_name(
            "/System/Library/CoreServices/Dock.app/Contents/MacOS/Dock",
        );
        assert_eq!(name, "Dock");
    }

    #[test]
    fn friendly_name_from_applications_path() {
        let name = get_friendly_app_name(
            "/Applications/Safari.app/Contents/MacOS/Safari",
        );
        assert_eq!(name, "Safari");
    }

    #[test]
    fn friendly_name_from_bare_executable() {
        let name = get_friendly_app_name("/usr/bin/ssh");
        assert_eq!(name, "ssh");
    }

    #[test]
    fn friendly_name_from_plain_name() {
        let name = get_friendly_app_name("Finder");
        assert_eq!(name, "Finder");
    }

    #[test]
    fn friendly_name_from_nested_app_bundle() {
        let name = get_friendly_app_name(
            "/Applications/Xcode.app/Contents/Developer/Applications/Simulator.app/Contents/MacOS/Simulator",
        );
        // Should find the first .app component
        assert_eq!(name, "Xcode");
    }

    // --- is_security_agent_process ---

    #[test]
    fn detects_security_agent() {
        assert!(is_security_agent_process("SecurityAgent"));
        assert!(is_security_agent_process("/usr/libexec/SecurityAgent"));
    }

    #[test]
    fn detects_coreauth() {
        assert!(is_security_agent_process("CoreAuthUI"));
        assert!(is_security_agent_process("coreauthdaemon"));
    }

    #[test]
    fn detects_user_auth_dialog() {
        assert!(is_security_agent_process("UserAuthenticationDialog"));
    }

    #[test]
    fn detects_local_authentication() {
        assert!(is_security_agent_process("LocalAuthentication"));
    }

    #[test]
    fn case_insensitive_security_agent() {
        assert!(is_security_agent_process("securityagent"));
        assert!(is_security_agent_process("SECURITYAGENT"));
    }

    #[test]
    fn rejects_normal_apps() {
        assert!(!is_security_agent_process("Safari"));
        assert!(!is_security_agent_process("Finder"));
        assert!(!is_security_agent_process("Terminal"));
    }

    // --- is_system_ui_process ---

    #[test]
    fn detects_dock() {
        assert!(is_system_ui_process("Dock"));
        assert!(is_system_ui_process("dock"));
    }

    #[test]
    fn detects_spotlight() {
        assert!(is_system_ui_process("Spotlight"));
    }

    #[test]
    fn detects_control_center() {
        assert!(is_system_ui_process("ControlCenter"));
        assert!(is_system_ui_process("Control Center"));
    }

    #[test]
    fn detects_system_ui_server() {
        assert!(is_system_ui_process("SystemUIServer"));
    }

    #[test]
    fn detects_notification_center() {
        assert!(is_system_ui_process("NotificationCenterUI"));
    }

    #[test]
    fn detects_window_server() {
        assert!(is_system_ui_process("WindowServer"));
    }

    #[test]
    fn rejects_regular_apps() {
        assert!(!is_system_ui_process("Safari"));
        assert!(!is_system_ui_process("Xcode"));
        assert!(!is_system_ui_process("Terminal"));
        assert!(!is_system_ui_process("Finder"));
    }
}
