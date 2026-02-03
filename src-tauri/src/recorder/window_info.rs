use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug)]
pub enum WindowError {
    NoFrontmostApp,
    NoWindows,
    WindowInfoFailed,
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowError::NoFrontmostApp => write!(f, "no frontmost application"),
            WindowError::NoWindows => write!(f, "no windows found"),
            WindowError::WindowInfoFailed => write!(f, "failed to get window info"),
        }
    }
}

impl std::error::Error for WindowError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub window_id: u32,
    pub bounds: WindowBounds,
}

impl WindowInfo {
    #[cfg(test)]
    pub fn sample() -> Self {
        Self {
            app_name: "Finder".to_string(),
            window_title: "Downloads".to_string(),
            window_id: 12345,
            bounds: WindowBounds {
                x: 100,
                y: 100,
                width: 800,
                height: 600,
            },
        }
    }
}

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

    for window_dict in windows {
        let dict = unsafe { core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(window_dict) };

        // Check if window belongs to frontmost app
        let owner_pid_key = CFString::new("kCGWindowOwnerPID");
        if let Some(owner_pid) = dict.find(&owner_pid_key) {
            let owner_pid: CFNumber = unsafe { CFNumber::wrap_under_get_rule(owner_pid.as_CFTypeRef() as _) };
            if let Some(owner_pid_val) = owner_pid.to_i32() {
                if owner_pid_val != pid {
                    continue;
                }
            }
        }

        // Get window ID
        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict.find(&window_id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);

        // Get window title
        let title_key = CFString::new("kCGWindowName");
        let window_title = dict.find(&title_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_default();

        // Get window bounds
        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = dict.find(&bounds_key)
            .map(|v| {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(v.as_CFTypeRef() as _) };

                let x = bounds_dict.find(&CFString::new("X"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let y = bounds_dict.find(&CFString::new("Y"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0);
                let width = bounds_dict.find(&CFString::new("Width"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;
                let height = bounds_dict.find(&CFString::new("Height"))
                    .and_then(|n| n.to_i32())
                    .unwrap_or(0) as u32;

                WindowBounds { x, y, width, height }
            })
            .unwrap_or(WindowBounds { x: 0, y: 0, width: 800, height: 600 });

        // Skip windows with no title (menu bar, etc)
        if window_title.is_empty() {
            continue;
        }

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
}
