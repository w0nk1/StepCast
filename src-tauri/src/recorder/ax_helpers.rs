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
        let result = AXUIElementCopyElementAtPosition(system_wide.as_type(), x, y, &mut element);
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
    pub subrole: Option<String>,
    /// Human readable role label from Accessibility (often localized).
    pub role_description: Option<String>,
    /// Best-effort identifier (when apps set AXIdentifier).
    pub identifier: Option<String>,
    pub label: String,
    /// Bounds of the selected element in screen coordinates (may be missing).
    pub element_bounds: Option<WindowBounds>,
    /// First meaningful container role found in the parent chain (helps when the
    /// clicked element is a static text inside a list row, etc.).
    pub container_role: Option<String>,
    pub container_subrole: Option<String>,
    pub container_identifier: Option<String>,
    pub window_role: Option<String>,
    pub window_subrole: Option<String>,
    pub window_bounds: Option<WindowBounds>,
    pub top_level_role: Option<String>,
    pub top_level_subrole: Option<String>,
    pub top_level_bounds: Option<WindowBounds>,
    pub parent_dialog_role: Option<String>,
    pub parent_dialog_subrole: Option<String>,
    pub parent_dialog_bounds: Option<WindowBounds>,
    pub is_checked: Option<bool>,
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

fn ax_copy_bool_attr(element: accessibility_sys::AXUIElementRef, attr_name: &str) -> Option<bool> {
    use accessibility_sys::AXUIElementCopyAttributeValue;
    use core_foundation::base::{CFGetTypeID, CFTypeRef, TCFType};
    use core_foundation::boolean::{CFBoolean, CFBooleanGetTypeID};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;

    unsafe {
        let attr = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let result = AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value);
        if result != 0 {
            return None;
        }
        let guard = CfRef::wrap(value as *mut _)?;
        let raw = guard.as_ptr() as *const _;
        let ty = CFGetTypeID(raw);

        if ty == CFBooleanGetTypeID() {
            let b: CFBoolean = CFBoolean::wrap_under_get_rule(raw as _);
            return Some(bool::from(b));
        }

        if ty == CFNumber::type_id() {
            let num: CFNumber = CFNumber::wrap_under_get_rule(raw as _);
            if let Some(v) = num.to_i32() {
                return Some(v != 0);
            }
        }

        if ty == CFString::type_id() {
            let s = CFString::wrap_under_get_rule(raw as _).to_string();
            let t = s.trim().to_lowercase();
            return match t.as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" => Some(false),
                _ => None,
            };
        }

        None
    }
}

fn ax_best_label_for_element(element: accessibility_sys::AXUIElementRef) -> Option<String> {
    use accessibility_sys::{
        kAXDescriptionAttribute, kAXRoleAttribute, kAXTitleAttribute, kAXValueAttribute,
        kAXValueDescriptionAttribute,
    };

    let role = ax_copy_string_attr(element, kAXRoleAttribute)
        .unwrap_or_default()
        .to_lowercase();

    // Role-specific label extraction:
    // - For text fields: never use AXValue (can contain user-entered text).
    // - For static text: AXValue is often the visible string.
    // - Otherwise: prefer Title/ValueDescription/Description.
    if role.contains("textfield") || role.contains("text field") || role.contains("textarea") {
        return ax_copy_string_attr(element, kAXTitleAttribute)
            .or_else(|| ax_copy_string_attr(element, kAXValueDescriptionAttribute))
            .or_else(|| ax_copy_string_attr(element, kAXDescriptionAttribute));
    }

    if role.contains("statictext") || role.contains("static text") {
        return ax_copy_string_attr(element, kAXValueAttribute)
            .or_else(|| ax_copy_string_attr(element, kAXTitleAttribute))
            .or_else(|| ax_copy_string_attr(element, kAXDescriptionAttribute))
            .or_else(|| ax_copy_string_attr(element, kAXValueDescriptionAttribute));
    }

    ax_copy_string_attr(element, kAXTitleAttribute)
        .or_else(|| ax_copy_string_attr(element, kAXValueDescriptionAttribute))
        .or_else(|| ax_copy_string_attr(element, kAXDescriptionAttribute))
}

fn ax_copy_children(element: accessibility_sys::AXUIElementRef) -> Vec<CfRef> {
    use accessibility_sys::{kAXChildrenAttribute, AXUIElementCopyAttributeValue};
    use core_foundation::array::{
        CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex, CFArrayRef,
    };
    use core_foundation::base::{CFGetTypeID, CFRetain, CFTypeRef, TCFType};
    use core_foundation::string::CFString;

    unsafe {
        let attr = CFString::new(kAXChildrenAttribute);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let result = AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value);
        if result != 0 {
            return Vec::new();
        }

        let Some(guard) = CfRef::wrap(value as *mut _) else {
            return Vec::new();
        };

        if CFGetTypeID(guard.as_ptr() as _) != CFArrayGetTypeID() {
            return Vec::new();
        }

        let arr: CFArrayRef = guard.as_ptr() as _;
        let count = CFArrayGetCount(arr);
        if count <= 0 {
            return Vec::new();
        }

        // Keep this bounded: some elements have huge child lists (tables, etc.).
        let limit = count.min(32);
        let mut out: Vec<CfRef> = Vec::with_capacity(limit as usize);

        for i in 0..limit {
            let ptr = CFArrayGetValueAtIndex(arr, i);
            if ptr.is_null() {
                continue;
            }
            let retained = CFRetain(ptr);
            if retained.is_null() {
                continue;
            }
            if let Some(child) = CfRef::wrap(retained as *mut _) {
                out.push(child);
            }
        }

        out
    }
}

fn ax_copy_action_names(element: accessibility_sys::AXUIElementRef) -> Vec<String> {
    use accessibility_sys::AXUIElementCopyActionNames;
    use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    unsafe {
        let mut names: CFArrayRef = std::ptr::null_mut();
        let result = AXUIElementCopyActionNames(element, &mut names);
        if result != 0 || names.is_null() {
            return Vec::new();
        }
        let Some(guard) = CfRef::wrap(names as *mut _) else {
            return Vec::new();
        };
        let arr = guard.as_ptr() as CFArrayRef;
        let count = CFArrayGetCount(arr as _);
        if count <= 0 {
            return Vec::new();
        }

        let limit = count.min(32);
        let mut out: Vec<String> = Vec::with_capacity(limit as usize);

        for i in 0..limit {
            let value = CFArrayGetValueAtIndex(arr as _, i);
            if value.is_null() {
                continue;
            }
            let s = CFString::wrap_under_get_rule(value as _).to_string();
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            out.push(trimmed);
        }

        out
    }
}

fn ax_actions_has_press(actions: &[String]) -> bool {
    actions.iter().any(|a| a == "AXPress" || a == "AXConfirm")
}

fn ax_actions_has_menu(actions: &[String]) -> bool {
    actions.iter().any(|a| a == "AXShowMenu")
}

fn ax_role_score(role: &str, subrole: &str) -> i32 {
    let r = role.to_lowercase();
    let s = subrole.to_lowercase();
    if r.contains("button") && s.contains("close") {
        return 260;
    }
    if r.contains("button") {
        return 240;
    }
    if r.contains("textfield")
        || r.contains("text field")
        || r.contains("textarea")
        || s.contains("searchfield")
    {
        return 230;
    }
    if r.contains("menuitem") {
        return 220;
    }
    if r.contains("menubar") || r.contains("menubaritem") {
        return 210;
    }
    if r.contains("checkbox") || r.contains("radiobutton") {
        return 200;
    }
    if r.contains("tab") {
        return 190;
    }
    if r.contains("row")
        || r.contains("cell")
        || r.contains("outline")
        || r.contains("table")
        || r.contains("list")
    {
        return 170;
    }
    if r.contains("statictext") || r.contains("static text") {
        return 120;
    }
    0
}

fn ax_candidate_score(
    role: &str,
    subrole: &str,
    actions: &[String],
    label: &str,
    depth: usize,
) -> i32 {
    let mut score = ax_role_score(role, subrole);
    if ax_actions_has_press(actions) {
        // Some apps implement "buttons" as pressable groups; prefer pressable elements
        // over nearby static text labels to avoid "App in App" descriptions.
        score = score.max(220);
    }
    if ax_actions_has_menu(actions) {
        score = score.max(200);
    }
    if !label.trim().is_empty() {
        score += 50;
    }
    // Prefer the closest element in the chain when scores are similar.
    score -= (depth as i32) * 4;
    score
}

fn ax_find_label_in_tree(
    element: accessibility_sys::AXUIElementRef,
    max_depth: usize,
    nodes_left: &mut usize,
) -> Option<String> {
    if max_depth == 0 || *nodes_left == 0 {
        return None;
    }

    let children = ax_copy_children(element);
    if children.is_empty() {
        return None;
    }

    for child in &children {
        if *nodes_left == 0 {
            return None;
        }
        *nodes_left -= 1;
        if let Some(label) = ax_best_label_for_element(child.as_type()) {
            return Some(label);
        }
    }

    if max_depth <= 1 {
        return None;
    }

    for child in children {
        if *nodes_left == 0 {
            return None;
        }
        if let Some(label) =
            ax_find_label_in_tree(child.as_type(), max_depth.saturating_sub(1), nodes_left)
        {
            return Some(label);
        }
    }

    None
}

fn ax_find_static_text_label_in_tree(
    element: accessibility_sys::AXUIElementRef,
    max_depth: usize,
    nodes_left: &mut usize,
) -> Option<String> {
    use accessibility_sys::kAXRoleAttribute;

    if max_depth == 0 || *nodes_left == 0 {
        return None;
    }

    let children = ax_copy_children(element);
    if children.is_empty() {
        return None;
    }

    for child in &children {
        if *nodes_left == 0 {
            return None;
        }
        *nodes_left -= 1;
        let role = ax_copy_string_attr(child.as_type(), kAXRoleAttribute)
            .unwrap_or_default()
            .to_lowercase();
        if !(role.contains("statictext") || role.contains("static text")) {
            continue;
        }
        if let Some(label) = ax_best_label_for_element(child.as_type()) {
            return Some(label);
        }
    }

    if max_depth <= 1 {
        return None;
    }

    for child in children {
        if *nodes_left == 0 {
            return None;
        }
        if let Some(label) = ax_find_static_text_label_in_tree(
            child.as_type(),
            max_depth.saturating_sub(1),
            nodes_left,
        ) {
            return Some(label);
        }
    }

    None
}

fn ax_copy_title_ui_label(element: accessibility_sys::AXUIElementRef) -> Option<String> {
    let title_el = ax_copy_element_attr(element, "AXTitleUIElement")?;
    ax_best_label_for_element(title_el.as_type())
}

fn ax_copy_placeholder_value(element: accessibility_sys::AXUIElementRef) -> Option<String> {
    ax_copy_string_attr(element, "AXPlaceholderValue")
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
        kAXPositionAttribute, kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize,
        AXValueGetType, AXValueGetValue,
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
) -> (Option<String>, Option<String>, Option<WindowBounds>) {
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

fn is_container_role(role: &str) -> bool {
    matches!(
        role,
        "AXRow"
            | "AXCell"
            | "AXTable"
            | "AXOutline"
            | "AXList"
            | "AXScrollArea"
            | "AXGroup"
            | "AXToolbar"
            | "AXSplitGroup"
    )
}

fn ax_find_container_parent(
    element: accessibility_sys::AXUIElementRef,
) -> (Option<String>, Option<String>, Option<String>) {
    use accessibility_sys::{kAXParentAttribute, kAXRoleAttribute, kAXSubroleAttribute};

    let mut current_raw = element;
    let mut current_guard: Option<CfRef> = None;

    fn container_score(role: &str, subrole: Option<&str>, identifier: Option<&str>) -> i32 {
        let r = role;
        let s = subrole.unwrap_or("").to_lowercase();
        let i = identifier.unwrap_or("").to_lowercase();

        let mut score = match r {
            "AXOutline" | "AXTable" | "AXList" => 100,
            "AXScrollArea" | "AXSplitGroup" => 80,
            "AXGroup" => 60,
            "AXRow" | "AXCell" => 20,
            "AXToolbar" => 10,
            _ => 0,
        };

        if s.contains("sourcelist") || s.contains("sidebar") || i.contains("sidebar") {
            score += 80;
        }

        score
    }

    let mut best: (i32, String, Option<String>, Option<String>) = (0, String::new(), None, None);

    for _ in 0..12 {
        let Some(parent) = ax_copy_element_attr(current_raw, kAXParentAttribute) else {
            break;
        };
        current_raw = parent.as_type();
        current_guard = Some(parent);

        let role = ax_copy_string_attr(current_raw, kAXRoleAttribute);
        if let Some(ref r) = role {
            if is_container_role(r) {
                let subrole = ax_copy_string_attr(current_raw, kAXSubroleAttribute);
                let ident = ax_copy_string_attr(current_raw, "AXIdentifier");
                let score = container_score(r, subrole.as_deref(), ident.as_deref());

                if score > best.0 {
                    best = (score, r.clone(), subrole.clone(), ident.clone());
                }

                // If we find a strong semantic signal (source list), stop early.
                if score >= 180 {
                    drop(current_guard);
                    return (Some(best.1), best.2, best.3);
                }
            }
        }
    }
    drop(current_guard);

    if best.0 > 0 {
        (Some(best.1), best.2, best.3)
    } else {
        (None, None, None)
    }
}

fn ax_copy_element_array_attr(
    element: accessibility_sys::AXUIElementRef,
    attr_name: &str,
) -> Vec<CfRef> {
    use accessibility_sys::AXUIElementCopyAttributeValue;
    use core_foundation::array::{
        CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex, CFArrayRef,
    };
    use core_foundation::base::{CFGetTypeID, CFRetain, CFTypeRef, TCFType};
    use core_foundation::string::CFString;

    unsafe {
        let attr = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let result = AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value);
        if result != 0 {
            return Vec::new();
        }

        let Some(guard) = CfRef::wrap(value as *mut _) else {
            return Vec::new();
        };

        if CFGetTypeID(guard.as_ptr() as _) != CFArrayGetTypeID() {
            return Vec::new();
        }

        let arr: CFArrayRef = guard.as_ptr() as _;
        let count = CFArrayGetCount(arr);
        if count <= 0 {
            return Vec::new();
        }

        // Keep this bounded: some apps expose huge selection arrays.
        let limit = count.min(8);
        let mut out: Vec<CfRef> = Vec::with_capacity(limit as usize);
        for i in 0..limit {
            let ptr = CFArrayGetValueAtIndex(arr, i);
            if ptr.is_null() {
                continue;
            }
            let retained = CFRetain(ptr);
            if retained.is_null() {
                continue;
            }
            if let Some(child) = CfRef::wrap(retained as *mut _) {
                out.push(child);
            }
        }
        out
    }
}

fn is_listish_role(role: &str) -> bool {
    let r = role.to_lowercase();
    r.contains("outline") || r.contains("table") || r.contains("list")
}

fn label_is_suspicious_for_list(label: &str) -> bool {
    let t = label.trim();
    if t.is_empty() {
        return true;
    }
    if t.len() > 56 {
        return true;
    }
    let mut digits = 0usize;
    let mut commas = 0usize;
    for ch in t.chars() {
        if ch.is_ascii_digit() {
            digits += 1;
        }
        if ch == ',' {
            commas += 1;
        }
    }
    if digits >= 10 && t.len() >= 28 {
        return true;
    }
    if commas >= 2 && t.len() >= 34 {
        return true;
    }

    let lower = t.to_lowercase();
    // Common "view mode" / structural labels. Keep small; this is cross-app enough for AppKit UI.
    if lower.contains("darstellung")
        || lower.contains("ansicht")
        || lower.ends_with(" view")
        || lower == "sidebar"
        || lower == "seitenleiste"
        || lower == "source list"
        || lower == "list"
    {
        return true;
    }

    if label_looks_like_size_or_time(&lower) {
        return true;
    }

    false
}

fn looks_like_filename_label(label: &str) -> bool {
    let t = label.trim();
    if t.len() < 3 || t.len() > 96 {
        return false;
    }
    let Some((base, ext)) = t.rsplit_once('.') else {
        return false;
    };
    if base.trim().is_empty() {
        return false;
    }
    if ext.is_empty() || ext.len() > 8 {
        return false;
    }
    ext.chars().all(|c| c.is_ascii_alphanumeric())
}

fn selected_label_is_probably_ui_target(label: &str) -> bool {
    let t = label.trim();
    if t.is_empty() {
        return false;
    }
    if t.contains('\n') || t.contains('\r') || t.contains('\t') {
        return false;
    }
    if looks_like_filename_label(t) {
        return true;
    }
    // Avoid accidentally surfacing long free-form text (e.g. chat message previews).
    if t.len() > 40 {
        return false;
    }
    let lower = t.to_lowercase();
    if label_looks_like_size_or_time(&lower) {
        return false;
    }
    if lower.contains("http://") || lower.contains("https://") || lower.contains("www.") {
        return false;
    }

    // Allow simple punctuation, but reject labels that look like sentences.
    let mut punct = 0usize;
    for ch in t.chars() {
        if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '_' || ch == '-' {
            continue;
        }
        if ch == '.' {
            // Non-filename: allow a single dot (e.g. "v1.0"), but count it.
            punct += 1;
            continue;
        }
        punct += 1;
    }
    punct <= 2
}

fn label_looks_like_size_or_time(lower_trimmed: &str) -> bool {
    let t = lower_trimmed.trim();
    if t.is_empty() {
        return false;
    }

    let looks_like_time = {
        let bytes = t.as_bytes();
        bytes.len() == 5
            && bytes[0].is_ascii_digit()
            && bytes[1].is_ascii_digit()
            && bytes[2] == b':'
            && bytes[3].is_ascii_digit()
            && bytes[4].is_ascii_digit()
    };
    if looks_like_time {
        return true;
    }

    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    let size_suffix = t.ends_with(" kb")
        || t.ends_with(" mb")
        || t.ends_with(" gb")
        || t.ends_with(" tb")
        || t.ends_with(" bytes")
        || t.ends_with(" byte")
        || t.ends_with(" b");
    if has_digit && size_suffix {
        return true;
    }

    false
}

fn ax_copy_selected_label(element: accessibility_sys::AXUIElementRef) -> Option<String> {
    // Try common AX selection attributes across AppKit and custom UIs.
    // These are best-effort; we keep it small and bounded.
    const ATTRS: [&str; 4] = [
        "AXSelectedRows",
        "AXSelectedChildren",
        "AXSelectedItems",
        "AXSelectedCells",
    ];

    let mut fallback: Option<String> = None;

    for attr in ATTRS {
        let items = ax_copy_element_array_attr(element, attr);
        if items.is_empty() {
            continue;
        }
        for item in items {
            let el: accessibility_sys::AXUIElementRef = item.as_type();
            // Prefer a direct label.
            if let Some(label) = ax_best_label_for_element(el) {
                let trimmed = label.trim();
                if selected_label_is_probably_ui_target(trimmed) {
                    return Some(trimmed.to_string());
                }
                if fallback.is_none() && !trimmed.is_empty() {
                    fallback = Some(trimmed.to_string());
                }
            }
            // Otherwise, try to find a nearby static text label in the subtree.
            let mut nodes_left = 64usize;
            if let Some(label) = ax_find_label_in_tree(el, 2, &mut nodes_left) {
                let trimmed = label.trim();
                if selected_label_is_probably_ui_target(trimmed) {
                    return Some(trimmed.to_string());
                }
                if fallback.is_none() && !trimmed.is_empty() {
                    fallback = Some(trimmed.to_string());
                }
            }
        }
    }
    fallback
}

/// Get role + label of the UI element at the given screen position using Accessibility API.
pub(super) fn get_clicked_element_label(x: f32, y: f32) -> Option<AxElementLabel> {
    use accessibility_sys::{
        kAXCancelButtonAttribute, kAXDefaultButtonAttribute, kAXParentAttribute, kAXRoleAttribute,
        kAXSubroleAttribute, kAXTopLevelUIElementAttribute, kAXWindowAttribute,
        AXUIElementCopyElementAtPosition, AXUIElementCreateSystemWide,
    };

    unsafe {
        let system_wide = CfRef::wrap(AXUIElementCreateSystemWide() as *mut _)?;

        let mut raw_element: accessibility_sys::AXUIElementRef = std::ptr::null_mut();
        let result =
            AXUIElementCopyElementAtPosition(system_wide.as_type(), x, y, &mut raw_element);
        if result != 0 {
            return None;
        }
        // Collect a short parent chain and pick the "best" interactive element.
        // This avoids vague labels like the app name when the hit-test returns a child/group.
        fn is_text_field_role(role: &str, subrole: &str) -> bool {
            let r = role.to_lowercase();
            let s = subrole.to_lowercase();
            r.contains("textfield")
                || r.contains("text field")
                || r.contains("textarea")
                || s.contains("searchfield")
        }

        let root = CfRef::wrap(raw_element as *mut _)?;
        let mut chain: Vec<CfRef> = Vec::with_capacity(8);
        chain.push(root);

        let mut current_raw: accessibility_sys::AXUIElementRef = chain[0].as_type();
        for _ in 0..7 {
            let Some(parent) = ax_copy_element_attr(current_raw, kAXParentAttribute) else {
                break;
            };
            current_raw = parent.as_type();
            chain.push(parent);
        }

        // Find the best list/table/outline container in the parent chain.
        // Prefer containers that actually contain the click point and are closest
        // to the clicked element in the chain.
        let mut list_container: Option<accessibility_sys::AXUIElementRef> = None;
        let mut list_container_score = i32::MIN;
        for (idx, guard) in chain.iter().enumerate() {
            let cand: accessibility_sys::AXUIElementRef = guard.as_type();
            let Some(role) = ax_copy_string_attr(cand, kAXRoleAttribute) else {
                continue;
            };
            if !is_listish_role(&role) {
                continue;
            }

            let mut score = 0i32;
            let depth_bonus = (90i32 - (idx as i32 * 8)).max(0);
            score += depth_bonus;

            if let Some(bounds) = ax_copy_window_bounds(cand) {
                let inside = (x as i32) >= bounds.x
                    && (x as i32) < bounds.x + bounds.width as i32
                    && (y as i32) >= bounds.y
                    && (y as i32) < bounds.y + bounds.height as i32;
                if inside {
                    score += 300;
                    // Prefer the tighter matching container when multiple contain the click.
                    let area = (bounds.width as i64).saturating_mul(bounds.height as i64);
                    let area_penalty = (area / 20_000) as i32;
                    score = score.saturating_sub(area_penalty.min(120));
                }
            }

            if score > list_container_score {
                list_container_score = score;
                list_container = Some(cand);
            }
        }

        let mut best_idx: usize = 0;
        let mut best_score: i32 = i32::MIN;
        let mut best_label: String = String::new();

        for (idx, guard) in chain.iter().enumerate() {
            let cand: accessibility_sys::AXUIElementRef = guard.as_type();
            let Some(role) = ax_copy_string_attr(cand, kAXRoleAttribute) else {
                continue;
            };
            let sub = ax_copy_string_attr(cand, kAXSubroleAttribute).unwrap_or_default();
            let actions = ax_copy_action_names(cand);
            let has_press = ax_actions_has_press(&actions);

            // Base label, role-aware.
            let mut label = ax_best_label_for_element(cand);

            let r_lower = role.to_lowercase();
            if label.is_none() && has_press {
                let mut nodes_left = 64usize;
                label = ax_find_static_text_label_in_tree(cand, 2, &mut nodes_left);
            }
            if label.is_none()
                && (r_lower.contains("row")
                    || r_lower.contains("cell")
                    || r_lower.contains("group"))
            {
                let mut nodes_left = 64usize;
                label = ax_find_label_in_tree(cand, 2, &mut nodes_left);
            }
            if label.is_none() && r_lower.contains("button") {
                let mut nodes_left = 64usize;
                label = ax_find_static_text_label_in_tree(cand, 2, &mut nodes_left);
            }
            if label.is_none() && is_text_field_role(&role, &sub) {
                label = ax_copy_title_ui_label(cand).or_else(|| ax_copy_placeholder_value(cand));
            }
            if label.is_none() {
                label = ax_copy_title_ui_label(cand);
            }
            let label = label.unwrap_or_default();
            let score = ax_candidate_score(&role, &sub, &actions, &label, idx);

            if score > best_score {
                best_score = score;
                best_idx = idx;
                best_label = label;
            }
        }

        let el: accessibility_sys::AXUIElementRef = chain[best_idx].as_type();

        let role = ax_copy_string_attr(el, kAXRoleAttribute);
        let subrole = ax_copy_string_attr(el, kAXSubroleAttribute);
        let role_description = ax_copy_string_attr(el, "AXRoleDescription");
        let identifier = ax_copy_string_attr(el, "AXIdentifier");
        let label = {
            let mut l = best_label;
            let best_role_is_listish = role.as_ref().map(|r| is_listish_role(r)).unwrap_or(false);
            // Improve list clicks by using the selected row/item label when the hit-test label is
            // structural ("List view") or metadata-heavy (timestamps, message status, etc.).
            //
            // We intentionally key this on *presence of a list container in the chain* (not just
            // the best element role), because many apps model list rows as pressable groups/buttons.
            if let Some(container) = list_container {
                let should_try_selected = label_is_suspicious_for_list(&l) || best_role_is_listish;
                if should_try_selected {
                    let mut selected = ax_copy_selected_label(container);

                    // Finder/AppKit list selection can lag by a few ms on first click.
                    // Retry once when selection still equals the old container-like label.
                    if let Some(ref current_selected) = selected {
                        if best_role_is_listish
                            && current_selected.trim().eq_ignore_ascii_case(l.trim())
                        {
                            std::thread::sleep(std::time::Duration::from_millis(14));
                            if let Some(retried) = ax_copy_selected_label(container) {
                                if !retried.trim().eq_ignore_ascii_case(current_selected.trim()) {
                                    selected = Some(retried);
                                }
                            }
                        }
                    }

                    if let Some(selected) = selected {
                        let selected_trim = selected.trim();
                        if selected_trim.len() <= 64
                            && selected_label_is_probably_ui_target(selected_trim)
                            && !selected_trim.eq_ignore_ascii_case(l.trim())
                        {
                            l = selected_trim.to_string();
                        }
                    }
                }
            }
            l
        };
        let element_bounds = ax_copy_window_bounds(el);

        let (container_role, container_subrole, container_identifier) =
            ax_find_container_parent(el);

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

        let (
            top_level_role,
            top_level_subrole,
            top_level_bounds,
            top_level_cancel,
            top_level_default,
        ) = ax_copy_element_attr(el, kAXTopLevelUIElementAttribute)
            .map(|top_guard| {
                let t: accessibility_sys::AXUIElementRef = top_guard.as_type();
                let role = ax_copy_string_attr(t, kAXRoleAttribute);
                let subrole = ax_copy_string_attr(t, kAXSubroleAttribute);
                let bounds = ax_copy_window_bounds(t);
                let is_cancel = ax_element_matches_attr_element(t, kAXCancelButtonAttribute, el);
                let is_default = ax_element_matches_attr_element(t, kAXDefaultButtonAttribute, el);
                (role, subrole, bounds, is_cancel, is_default)
            })
            .unwrap_or((None, None, None, false, false));

        let (parent_dialog_role, parent_dialog_subrole, parent_dialog_bounds) =
            ax_find_dialog_parent(el);
        let is_checked = ax_copy_bool_attr(el, "AXValue");

        // Return best-effort metadata even when the label is missing.
        role.map(|role| AxElementLabel {
            role,
            subrole,
            role_description,
            identifier,
            label,
            element_bounds,
            container_role,
            container_subrole,
            container_identifier,
            window_role,
            window_subrole,
            window_bounds,
            top_level_role,
            top_level_subrole,
            top_level_bounds,
            parent_dialog_role,
            parent_dialog_subrole,
            parent_dialog_bounds,
            is_checked,
            is_cancel_button: is_cancel_button || top_level_cancel,
            is_default_button: is_default_button || top_level_default,
        })
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
    proc_path
        .split('/')
        .next_back()
        .unwrap_or(proc_path)
        .to_string()
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
        let name =
            get_friendly_app_name("/System/Library/CoreServices/Dock.app/Contents/MacOS/Dock");
        assert_eq!(name, "Dock");
    }

    #[test]
    fn friendly_name_from_applications_path() {
        let name = get_friendly_app_name("/Applications/Safari.app/Contents/MacOS/Safari");
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

    // --- ax_candidate_score ---

    #[test]
    fn pressable_group_scores_higher_than_static_text() {
        let static_text = ax_candidate_score("AXStaticText", "", &[], "RustDesk", 0);
        let pressable_group = ax_candidate_score("AXGroup", "", &[String::from("AXPress")], "", 1);
        assert!(pressable_group > static_text);
    }

    #[test]
    fn close_button_scores_higher_than_pressable_group() {
        let close_btn = ax_candidate_score(
            "AXButton",
            "AXCloseButton",
            &[String::from("AXPress")],
            "",
            0,
        );
        let pressable_group = ax_candidate_score("AXGroup", "", &[String::from("AXPress")], "", 0);
        assert!(close_btn > pressable_group);
    }

    #[test]
    fn suspicious_list_labels_detect_sidebar_and_sizes() {
        assert!(label_is_suspicious_for_list("Seitenleiste"));
        assert!(label_is_suspicious_for_list("sidebar"));
        assert!(label_is_suspicious_for_list("2,4 MB"));
        assert!(label_is_suspicious_for_list("12:34"));
    }

    #[test]
    fn selected_label_rejects_size_values_but_accepts_filenames() {
        assert!(!selected_label_is_probably_ui_target("2,4 MB"));
        assert!(!selected_label_is_probably_ui_target("12:34"));
        assert!(selected_label_is_probably_ui_target("test.pdf"));
    }
}
