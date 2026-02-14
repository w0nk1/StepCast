//! Overlay and topmost window detection at a given screen point.
#![allow(dead_code)]

use super::types::{WindowBounds, WindowInfo};

/// Get the process name for a PID (language-independent).
/// Returns the executable name, not the localized display name.
fn get_process_name_by_pid(pid: i32) -> Option<String> {
    use std::process::Command;
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            // Extract just the executable name from path
            return Some(name.split('/').next_back().unwrap_or(&name).to_string());
        }
    }
    None
}

fn normalize_app_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn app_names_match(left: &str, right: &str) -> bool {
    let left_norm = normalize_app_name(left);
    let right_norm = normalize_app_name(right);
    !left_norm.is_empty() && left_norm == right_norm
}

/// Get the topmost on-screen window at the given click point.
/// This checks ALL windows (not just the frontmost app) to properly capture
/// popup menus, context menus, and other overlay windows.
#[cfg(target_os = "macos")]
pub fn get_topmost_window_at_point(click_x: i32, click_y: i32) -> Option<WindowInfo> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;

    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };

    if window_list.is_null() {
        return None;
    }

    let windows: Vec<CFDictionaryRef> = unsafe {
        let count = core_foundation::array::CFArrayGetCount(window_list as _);
        (0..count)
            .map(|i| {
                core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i)
                    as CFDictionaryRef
            })
            .collect()
    };

    // Windows are returned front-to-back when using kCGWindowListOptionOnScreenOnly
    // So we return the first window that contains the click point
    for window_dict in windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                window_dict,
            )
        };

        // Get window layer - skip desktop/wallpaper level windows
        let layer_key = CFString::new("kCGWindowLayer");
        let layer = dict
            .find(layer_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32()
            })
            .unwrap_or(0);

        // Skip windows at desktop level or below (layer < 0 typically means desktop)
        if layer < 0 {
            continue;
        }

        // Get window bounds
        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = match dict.find(bounds_key) {
            Some(v) => {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> = unsafe {
                    core_foundation::dictionary::CFDictionary::wrap_under_get_rule(
                        v.as_CFTypeRef() as _
                    )
                };

                let x = bounds_dict
                    .find(CFString::new("X"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let y = bounds_dict
                    .find(CFString::new("Y"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let width = bounds_dict
                    .find(CFString::new("Width"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;
                let height = bounds_dict
                    .find(CFString::new("Height"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;

                WindowBounds {
                    x,
                    y,
                    width,
                    height,
                }
            }
            None => continue,
        };

        // Skip tiny or invisible windows
        if bounds.width < 10 || bounds.height < 10 {
            continue;
        }

        // Check if click is inside this window's bounds
        let inside_x = click_x >= bounds.x && click_x < bounds.x + bounds.width as i32;
        let inside_y = click_y >= bounds.y && click_y < bounds.y + bounds.height as i32;

        if !inside_x || !inside_y {
            continue;
        }

        // Get window ID
        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict
            .find(window_id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);

        if window_id == 0 {
            continue;
        }

        // Get owner PID and use it to get the actual process name (not localized)
        let owner_pid_key = CFString::new("kCGWindowOwnerPID");
        let owner_pid = dict.find(owner_pid_key).and_then(|v| {
            let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
            num.to_i32()
        });

        // Get process name via PID (language-independent) or fall back to localized name
        let app_name = if let Some(pid) = owner_pid {
            get_process_name_by_pid(pid).unwrap_or_else(|| {
                // Fallback to localized name from window info
                let owner_name_key = CFString::new("kCGWindowOwnerName");
                dict.find(owner_name_key)
                    .map(|v| {
                        let s: CFString =
                            unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                        s.to_string()
                    })
                    .unwrap_or_else(|| "Unknown".to_string())
            })
        } else {
            let owner_name_key = CFString::new("kCGWindowOwnerName");
            dict.find(owner_name_key)
                .map(|v| {
                    let s: CFString =
                        unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                    s.to_string()
                })
                .unwrap_or_else(|| "Unknown".to_string())
        };

        // Skip system UI windows (Dock, Spotlight, etc.) — they have full-screen
        // overlay windows at high layers that shadow real app windows beneath.
        if super::super::ax_helpers::is_system_ui_process(&app_name) {
            if cfg!(debug_assertions) {
                eprintln!(
                    "Skipping system UI window at click: '{app_name}' id={window_id} layer={layer} bounds=({}, {}, {}x{})",
                    bounds.x, bounds.y, bounds.width, bounds.height
                );
            }
            continue;
        }

        // Get window title
        let title_key = CFString::new("kCGWindowName");
        let window_title = dict
            .find(title_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_default();

        if cfg!(debug_assertions) {
            eprintln!(
                "Topmost window at click: '{app_name}' - '{window_title}' id={window_id} layer={layer} bounds=({}, {}, {}x{})",
                bounds.x, bounds.y, bounds.width, bounds.height
            );
        }

        return Some(WindowInfo {
            app_name,
            window_title,
            window_id,
            bounds,
        });
    }

    None
}

/// Find an attached dialog/sheet window at the click point.
/// Sheets are typically smaller than the main window and overlap it heavily.
#[cfg(target_os = "macos")]
pub fn find_attached_dialog_window(
    click_x: i32,
    click_y: i32,
    main_window: &WindowInfo,
    expected_owner: Option<(i32, &str)>,
) -> Option<WindowInfo> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;

    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };

    if window_list.is_null() {
        return None;
    }

    let windows: Vec<CFDictionaryRef> = unsafe {
        let count = core_foundation::array::CFArrayGetCount(window_list as _);
        (0..count)
            .map(|i| {
                core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i)
                    as CFDictionaryRef
            })
            .collect()
    };

    let main = &main_window.bounds;
    let main_area = (main.width as i64) * (main.height as i64);
    let main_left = main.x;
    let main_top = main.y;
    let main_right = main.x + main.width as i32;
    let main_bottom = main.y + main.height as i32;
    let expected_owner_pid = expected_owner.map(|(pid, _)| pid);
    let expected_owner_name = expected_owner.map(|(_, name)| name);

    let mut main_owner_pid: Option<i32> = None;
    for window_dict in &windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                *window_dict,
            )
        };
        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict
            .find(window_id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);
        if window_id != main_window.window_id {
            continue;
        }
        let owner_pid_key = CFString::new("kCGWindowOwnerPID");
        main_owner_pid = dict.find(owner_pid_key).and_then(|v| {
            let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
            num.to_i32()
        });
        break;
    }

    for window_dict in windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                window_dict,
            )
        };

        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = match dict.find(bounds_key) {
            Some(v) => {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> = unsafe {
                    core_foundation::dictionary::CFDictionary::wrap_under_get_rule(
                        v.as_CFTypeRef() as _
                    )
                };

                let x = bounds_dict
                    .find(CFString::new("X"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let y = bounds_dict
                    .find(CFString::new("Y"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let width = bounds_dict
                    .find(CFString::new("Width"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;
                let height = bounds_dict
                    .find(CFString::new("Height"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;

                WindowBounds {
                    x,
                    y,
                    width,
                    height,
                }
            }
            None => continue,
        };

        if bounds.width < 50 || bounds.height < 20 {
            continue;
        }

        let inside_x = click_x >= bounds.x && click_x < bounds.x + bounds.width as i32;
        let inside_y = click_y >= bounds.y && click_y < bounds.y + bounds.height as i32;
        if !inside_x || !inside_y {
            continue;
        }

        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict
            .find(window_id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);

        if window_id == 0 || window_id == main_window.window_id {
            continue;
        }

        let area = (bounds.width as i64) * (bounds.height as i64);
        if area >= main_area {
            continue;
        }
        let area_ratio = if main_area > 0 {
            area as f32 / main_area as f32
        } else {
            0.0
        };
        if !(0.04..0.95).contains(&area_ratio) {
            continue;
        }

        let right = bounds.x + bounds.width as i32;
        let bottom = bounds.y + bounds.height as i32;

        let inter_left = bounds.x.max(main_left);
        let inter_top = bounds.y.max(main_top);
        let inter_right = right.min(main_right);
        let inter_bottom = bottom.min(main_bottom);
        let inter_w = (inter_right - inter_left).max(0) as i64;
        let inter_h = (inter_bottom - inter_top).max(0) as i64;
        let inter_area = inter_w * inter_h;
        let overlap_ratio = if area > 0 {
            inter_area as f32 / area as f32
        } else {
            0.0
        };

        let contained = bounds.x >= main_left - 6
            && bounds.y >= main_top - 6
            && right <= main_right + 6
            && bottom <= main_bottom + 6;

        if overlap_ratio < 0.6 && !contained {
            continue;
        }

        let owner_pid_key = CFString::new("kCGWindowOwnerPID");
        let owner_pid = dict.find(owner_pid_key).and_then(|v| {
            let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
            num.to_i32()
        });

        let app_name = if let Some(pid) = owner_pid {
            get_process_name_by_pid(pid).unwrap_or_else(|| {
                let owner_name_key = CFString::new("kCGWindowOwnerName");
                dict.find(owner_name_key)
                    .map(|v| {
                        let s: CFString =
                            unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                        s.to_string()
                    })
                    .unwrap_or_else(|| "Unknown".to_string())
            })
        } else {
            let owner_name_key = CFString::new("kCGWindowOwnerName");
            dict.find(owner_name_key)
                .map(|v| {
                    let s: CFString =
                        unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                    s.to_string()
                })
                .unwrap_or_else(|| "Unknown".to_string())
        };

        if super::super::ax_helpers::is_system_ui_process(&app_name) {
            continue;
        }

        let expected_pid = expected_owner_pid.or(main_owner_pid);
        if let Some(pid) = expected_pid {
            if owner_pid != Some(pid) {
                continue;
            }
        } else if let Some(name) = expected_owner_name {
            if !app_names_match(&app_name, name) {
                continue;
            }
        } else if !app_names_match(&app_name, &main_window.app_name) {
            continue;
        }

        let title_key = CFString::new("kCGWindowName");
        let window_title = dict
            .find(title_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_default();

        return Some(WindowInfo {
            app_name,
            window_title,
            window_id,
            bounds,
        });
    }

    None
}

#[cfg(not(target_os = "macos"))]
pub fn find_attached_dialog_window(
    _click_x: i32,
    _click_y: i32,
    _main_window: &WindowInfo,
    _expected_owner: Option<(i32, &str)>,
) -> Option<WindowInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::app_names_match;

    #[test]
    fn app_name_match_normalizes_hidden_chars() {
        assert!(app_names_match("‎WhatsApp", "WhatsApp"));
        assert!(app_names_match("WireGuard", "wireguard"));
    }

    #[test]
    fn app_name_match_rejects_different_names() {
        assert!(!app_names_match("Finder", "Preview"));
    }
}
