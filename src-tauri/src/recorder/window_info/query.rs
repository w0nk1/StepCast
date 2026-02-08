//! Window query functions: find windows by click position, frontmost app, or PID.
#![allow(dead_code)]

use super::types::{WindowBounds, WindowError, WindowInfo};

/// Get the window that contains the click point.
/// This properly handles modal dialogs by finding the topmost window containing the click.
#[cfg(target_os = "macos")]
pub fn get_window_at_click(click_x: i32, click_y: i32) -> Result<WindowInfo, WindowError> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;
    use objc2_app_kit::NSWorkspace;

    // Get frontmost app
    let workspace = NSWorkspace::sharedWorkspace();
    let frontmost = workspace.frontmostApplication()
        .ok_or(WindowError::NoFrontmostApp)?;

    let app_name = frontmost.localizedName()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let pid = frontmost.processIdentifier();

    // Get windows for this app (ordered front-to-back)
    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };

    if window_list.is_null() {
        return Err(WindowError::NoWindows);
    }

    let windows: Vec<CFDictionaryRef> = unsafe {
        let count = core_foundation::array::CFArrayGetCount(window_list as _);
        (0..count)
            .map(|i| core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef)
            .collect()
    };

    // Find the topmost window of this app that contains the click point
    for window_dict in windows {
        let dict = unsafe { core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(window_dict) };

        // Check if window belongs to frontmost app
        let owner_pid_key = CFString::new("kCGWindowOwnerPID");
        if let Some(owner_pid) = dict.find(owner_pid_key) {
            let owner_pid: CFNumber = unsafe { CFNumber::wrap_under_get_rule(owner_pid.as_CFTypeRef() as _) };
            if let Some(owner_pid_val) = owner_pid.to_i32() {
                if owner_pid_val != pid {
                    continue;
                }
            }
        }

        // Get window bounds first to check if click is inside
        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = dict.find(bounds_key)
            .map(|v| {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(v.as_CFTypeRef() as _) };

                let x = bounds_dict.find(CFString::new("X"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let y = bounds_dict.find(CFString::new("Y"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let width = bounds_dict.find(CFString::new("Width"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;
                let height = bounds_dict.find(CFString::new("Height"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;

                WindowBounds { x, y, width, height }
            });

        let bounds = match bounds {
            Some(b) => b,
            None => continue,
        };

        // Check if click is inside this window's bounds
        let inside_x = click_x >= bounds.x && click_x < bounds.x + bounds.width as i32;
        let inside_y = click_y >= bounds.y && click_y < bounds.y + bounds.height as i32;

        if !inside_x || !inside_y {
            continue;
        }

        // Get window ID
        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict.find(window_id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);

        // Get window title
        let title_key = CFString::new("kCGWindowName");
        let window_title = dict.find(title_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_default();

        if cfg!(debug_assertions) {
            eprintln!(
                "Found window containing click: '{window_title}' id={window_id} bounds=({}, {}, {}x{})",
                bounds.x, bounds.y, bounds.width, bounds.height
            );
        }

        // Return the first (topmost) window that contains the click
        return Ok(WindowInfo {
            app_name,
            window_title,
            window_id,
            bounds,
        });
    }

    // Fallback: return app info without specific window
    Ok(WindowInfo {
        app_name,
        window_title: String::new(),
        window_id: 0,
        bounds: WindowBounds { x: 0, y: 0, width: 800, height: 600 },
    })
}

/// Get the main (largest) window of the frontmost app.
/// This is used for screenshot capture and click position calculation.
/// Using the largest window ensures we get the parent window, not a modal/sheet.
#[cfg(target_os = "macos")]
pub fn get_frontmost_window() -> Result<WindowInfo, WindowError> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;
    use objc2_app_kit::NSWorkspace;

    // Get frontmost app
    let workspace = NSWorkspace::sharedWorkspace();
    let frontmost = workspace.frontmostApplication()
        .ok_or(WindowError::NoFrontmostApp)?;

    let app_name = frontmost.localizedName()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let pid = frontmost.processIdentifier();

    // Get windows for this app
    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };

    if window_list.is_null() {
        return Err(WindowError::NoWindows);
    }

    let windows: Vec<CFDictionaryRef> = unsafe {
        let count = core_foundation::array::CFArrayGetCount(window_list as _);
        (0..count)
            .map(|i| core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef)
            .collect()
    };

    // Find the largest window of this app (the main window, not modals/sheets)
    let mut best_window: Option<WindowInfo> = None;
    let mut best_area: u64 = 0;

    for window_dict in windows {
        let dict = unsafe { core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(window_dict) };

        // Check if window belongs to frontmost app
        let owner_pid_key = CFString::new("kCGWindowOwnerPID");
        if let Some(owner_pid) = dict.find(owner_pid_key) {
            let owner_pid: CFNumber = unsafe { CFNumber::wrap_under_get_rule(owner_pid.as_CFTypeRef() as _) };
            if let Some(owner_pid_val) = owner_pid.to_i32() {
                if owner_pid_val != pid {
                    continue;
                }
            }
        }

        // Get window ID
        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict.find(window_id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);

        if window_id == 0 {
            continue;
        }

        // Get window title
        let title_key = CFString::new("kCGWindowName");
        let window_title = dict.find(title_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_default();

        // Get window bounds
        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = dict.find(bounds_key)
            .map(|v| {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(v.as_CFTypeRef() as _) };

                let x = bounds_dict.find(CFString::new("X"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let y = bounds_dict.find(CFString::new("Y"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let width = bounds_dict.find(CFString::new("Width"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;
                let height = bounds_dict.find(CFString::new("Height"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;

                WindowBounds { x, y, width, height }
            })
            .unwrap_or(WindowBounds { x: 0, y: 0, width: 0, height: 0 });

        let area = bounds.width as u64 * bounds.height as u64;

        // Keep track of the largest window
        if area > best_area {
            best_area = area;
            best_window = Some(WindowInfo {
                app_name: app_name.clone(),
                window_title,
                window_id,
                bounds,
            });
        }
    }

    if let Some(window) = best_window {
        if cfg!(debug_assertions) {
            eprintln!(
                "Main window: '{}' id={} bounds=({}, {}, {}x{})",
                window.window_title, window.window_id,
                window.bounds.x, window.bounds.y,
                window.bounds.width, window.bounds.height
            );
        }
        return Ok(window);
    }

    // Fallback: return app info without specific window
    Ok(WindowInfo {
        app_name,
        window_title: String::new(),
        window_id: 0,
        bounds: WindowBounds { x: 0, y: 0, width: 800, height: 600 },
    })
}

/// Find the largest on-screen window belonging to a specific PID.
/// Returns None if the process has no visible windows.
pub fn get_main_window_for_pid(pid: i32, app_name: &str) -> Option<WindowInfo> {
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
            .map(|i| core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef)
            .collect()
    };

    let mut best_window: Option<WindowInfo> = None;
    let mut best_area: u64 = 0;

    for window_dict in windows {
        let dict = unsafe { core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(window_dict) };

        let owner_pid_key = CFString::new("kCGWindowOwnerPID");
        if let Some(owner_pid) = dict.find(owner_pid_key) {
            let owner_pid: CFNumber = unsafe { CFNumber::wrap_under_get_rule(owner_pid.as_CFTypeRef() as _) };
            if let Some(owner_pid_val) = owner_pid.to_i32() {
                if owner_pid_val != pid {
                    continue;
                }
            }
        }

        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict.find(window_id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);

        if window_id == 0 {
            continue;
        }

        let title_key = CFString::new("kCGWindowName");
        let window_title = dict.find(title_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_default();

        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = dict.find(bounds_key)
            .map(|v| {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(v.as_CFTypeRef() as _) };

                let x = bounds_dict.find(CFString::new("X"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let y = bounds_dict.find(CFString::new("Y"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let width = bounds_dict.find(CFString::new("Width"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;
                let height = bounds_dict.find(CFString::new("Height"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;

                WindowBounds { x, y, width, height }
            })
            .unwrap_or(WindowBounds { x: 0, y: 0, width: 0, height: 0 });

        let area = bounds.width as u64 * bounds.height as u64;

        if area > best_area {
            best_area = area;
            best_window = Some(WindowInfo {
                app_name: app_name.to_string(),
                window_title,
                window_id,
                bounds,
            });
        }
    }

    best_window
}
