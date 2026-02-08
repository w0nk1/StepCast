// Some functions are kept for potential future use
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum WindowError {
    NoFrontmostApp,
    NoWindows,
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowError::NoFrontmostApp => write!(f, "no frontmost application"),
            WindowError::NoWindows => write!(f, "no windows found"),
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

#[derive(Debug, Clone)]
struct AuthHeuristicConfig {
    layer_modal: i32,
    layer_status: i32,
    layer_popup: i32,
    min_area_ratio: f32,
    max_area_ratio: f32,
    max_center_dist_ratio: f32,
    min_width: u32,
    min_height: u32,
    min_aspect: f32,
    max_aspect: f32,
    score_threshold: i32,
    recent_window_ms: i64,
}

impl Default for AuthHeuristicConfig {
    fn default() -> Self {
        Self {
            layer_modal: 0,
            layer_status: 0,
            layer_popup: 0,
            min_area_ratio: 0.01,
            max_area_ratio: 0.30,
            max_center_dist_ratio: 0.35,
            min_width: 120,
            min_height: 80,
            min_aspect: 0.6,
            max_aspect: 2.2,
            score_threshold: 6,
            recent_window_ms: 500,
        }
    }
}

const DEFAULT_LAYER_MODAL: i32 = 8;
const DEFAULT_LAYER_STATUS: i32 = 25;
const DEFAULT_LAYER_POPUP: i32 = 101;

#[derive(Debug, Clone)]
struct AuthWindowCandidate {
    info: WindowInfo,
    layer: i32,
    alpha: f32,
    area_ratio: f32,
    center_dist_ratio: f32,
    title_empty: bool,
    click_inside: bool,
    is_recent: bool,
    score: i32,
}

#[derive(Debug)]
struct WindowRecencyCache {
    initialized: bool,
    last_seen: HashMap<u32, i64>,
}

static WINDOW_RECENCY_CACHE: OnceLock<Mutex<WindowRecencyCache>> = OnceLock::new();

fn window_recency_cache() -> &'static Mutex<WindowRecencyCache> {
    WINDOW_RECENCY_CACHE.get_or_init(|| {
        Mutex::new(WindowRecencyCache {
            initialized: false,
            last_seen: HashMap::new(),
        })
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn window_is_recent(cache: &WindowRecencyCache, window_id: u32, now_ms: i64, cfg: &AuthHeuristicConfig) -> bool {
    if !cache.initialized {
        return false;
    }
    match cache.last_seen.get(&window_id) {
        None => true,
        Some(ts) => now_ms - *ts <= cfg.recent_window_ms,
    }
}

fn score_auth_candidate(candidate: &mut AuthWindowCandidate, cfg: &AuthHeuristicConfig, clicked_info_missing: bool) -> i32 {
    let mut score = 0;

    if candidate.layer >= cfg.layer_modal {
        score += 2;
    }
    if candidate.layer >= cfg.layer_status {
        score += 1;
    }
    if candidate.title_empty {
        score += 1;
    }
    if candidate.center_dist_ratio <= 0.25 {
        score += 1;
    }
    if candidate.area_ratio >= 0.01 && candidate.area_ratio <= 0.20 {
        score += 1;
    }
    if clicked_info_missing {
        score += 1;
    }
    if candidate.click_inside {
        score += 1;
    }
    if candidate.is_recent {
        score += 2;
    }

    candidate.score = score;
    score
}

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

#[derive(Debug, Clone)]
struct WindowCandidate {
    info: WindowInfo,
    layer: i32,
    area: u64,
}

fn is_security_agent_name(app_name: &str) -> bool {
    let name = app_name.to_lowercase();
    name.contains("securityagent")
        || name.contains("coreauth")
        || name.contains("coreauthuia")
        || name.contains("coreauthui")
        || name.contains("coreauthd")
        || name.contains("coreautha")
}

/// Find a system authentication dialog window (Touch ID / SecurityAgent).
#[cfg(target_os = "macos")]
pub fn get_security_agent_window() -> Result<Option<WindowInfo>, WindowError> {
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
        return Err(WindowError::NoWindows);
    }

    let windows: Vec<CFDictionaryRef> = unsafe {
        let count = core_foundation::array::CFArrayGetCount(window_list as _);
        (0..count)
            .map(|i| core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef)
            .collect()
    };

    let mut best: Option<WindowCandidate> = None;

    for window_dict in windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                window_dict,
            )
        };

        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict
            .find(window_id_key)
            .and_then(|v| {
                let num: CFNumber =
                    unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);

        if window_id == 0 {
            continue;
        }

        let owner_name_key = CFString::new("kCGWindowOwnerName");
        let app_name = dict
            .find(owner_name_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_else(|| "Unknown".to_string());

        if !is_security_agent_name(&app_name) {
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

        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = dict
            .find(bounds_key)
            .map(|v| {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe {
                        core_foundation::dictionary::CFDictionary::wrap_under_get_rule(
                            v.as_CFTypeRef() as _,
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

                WindowBounds { x, y, width, height }
            })
            .unwrap_or(WindowBounds { x: 0, y: 0, width: 0, height: 0 });

        if bounds.width == 0 || bounds.height == 0 {
            continue;
        }

        let layer_key = CFString::new("kCGWindowLayer");
        let layer = dict
            .find(layer_key)
            .and_then(|v| {
                let num: CFNumber =
                    unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32()
            })
            .unwrap_or(0);

        let area = bounds.width as u64 * bounds.height as u64;

        let candidate = WindowCandidate {
            info: WindowInfo {
                app_name,
                window_title,
                window_id,
                bounds,
            },
            layer,
            area,
        };

        // Keep the window with highest layer, or largest area if same layer
        match best {
            None => best = Some(candidate),
            Some(ref current) => {
                let replace = if candidate.layer > current.layer {
                    true
                } else if candidate.layer == current.layer {
                    candidate.area > current.area
                } else {
                    false
                };
                if replace {
                    best = Some(candidate);
                }
            }
        }
    }

    if let Some(ref c) = best {
        if cfg!(debug_assertions) {
            eprintln!(
                "Found security agent window: '{}' id={} bounds=({}, {}, {}x{})",
                c.info.app_name, c.info.window_id,
                c.info.bounds.x, c.info.bounds.y,
                c.info.bounds.width, c.info.bounds.height
            );
        }
    }

    Ok(best.map(|c| c.info))
}

#[cfg(target_os = "macos")]
fn get_auth_heuristic_config() -> AuthHeuristicConfig {
    AuthHeuristicConfig {
        layer_modal: DEFAULT_LAYER_MODAL,
        layer_status: DEFAULT_LAYER_STATUS,
        layer_popup: DEFAULT_LAYER_POPUP,
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn get_main_screen_size_points() -> (i32, i32) {
    use core_graphics::display::CGDisplay;
    let main = CGDisplay::main();
    let bounds = main.bounds();
    (bounds.size.width as i32, bounds.size.height as i32)
}

/// Find a likely authentication dialog window using heuristics (layer, geometry, timing).
#[cfg(target_os = "macos")]
pub fn find_auth_dialog_window(
    click_x: i32,
    click_y: i32,
    clicked_info_missing: bool,
) -> Result<Option<WindowInfo>, WindowError> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;

    let cfg = get_auth_heuristic_config();
    let (screen_w, screen_h) = get_main_screen_size_points();
    if screen_w <= 0 || screen_h <= 0 {
        return Ok(None);
    }

    let screen_area = (screen_w as f32) * (screen_h as f32);
    let screen_center_x = screen_w as f32 / 2.0;
    let screen_center_y = screen_h as f32 / 2.0;
    let center_denominator = (screen_w.min(screen_h) as f32).max(1.0);

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

    let now = now_ms();
    let mut candidates: Vec<AuthWindowCandidate> = Vec::new();
    let mut current_ids: HashSet<u32> = HashSet::new();

    let mut cache = window_recency_cache().lock().unwrap();
    let initialized = cache.initialized;

    for window_dict in windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                window_dict,
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

        if window_id == 0 {
            continue;
        }

        current_ids.insert(window_id);

        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = match dict.find(bounds_key) {
            Some(v) => {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(v.as_CFTypeRef() as _) };

                let x = bounds_dict.find(CFString::new("X")).and_then(|n| n.to_i32()).unwrap_or(0);
                let y = bounds_dict.find(CFString::new("Y")).and_then(|n| n.to_i32()).unwrap_or(0);
                let width = bounds_dict.find(CFString::new("Width")).and_then(|n| n.to_i32()).unwrap_or(0) as u32;
                let height = bounds_dict.find(CFString::new("Height")).and_then(|n| n.to_i32()).unwrap_or(0) as u32;

                WindowBounds { x, y, width, height }
            }
            None => continue,
        };

        if bounds.width < cfg.min_width || bounds.height < cfg.min_height {
            continue;
        }

        let area = (bounds.width as f32) * (bounds.height as f32);
        let area_ratio = area / screen_area;
        if area_ratio < cfg.min_area_ratio || area_ratio > cfg.max_area_ratio {
            continue;
        }

        let aspect = bounds.width as f32 / bounds.height.max(1) as f32;
        if aspect < cfg.min_aspect || aspect > cfg.max_aspect {
            continue;
        }

        let center_x = bounds.x as f32 + bounds.width as f32 / 2.0;
        let center_y = bounds.y as f32 + bounds.height as f32 / 2.0;
        let dx = center_x - screen_center_x;
        let dy = center_y - screen_center_y;
        let center_dist = (dx * dx + dy * dy).sqrt();
        let center_dist_ratio = center_dist / center_denominator;
        if center_dist_ratio > cfg.max_center_dist_ratio {
            continue;
        }

        let layer_key = CFString::new("kCGWindowLayer");
        let layer = dict
            .find(layer_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32()
            })
            .unwrap_or(0);

        // Skip desktop-level windows
        if layer < 0 {
            continue;
        }

        let alpha_key = CFString::new("kCGWindowAlpha");
        let alpha = dict
            .find(alpha_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32().map(|n| n as f32)
            })
            .unwrap_or(1.0);

        if alpha <= 0.01 {
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
        let title_empty = window_title.is_empty();

        let owner_name_key = CFString::new("kCGWindowOwnerName");
        let app_name = dict
            .find(owner_name_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                s.to_string()
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let click_inside = click_x >= bounds.x
            && click_x < bounds.x + bounds.width as i32
            && click_y >= bounds.y
            && click_y < bounds.y + bounds.height as i32;

        let is_recent = window_is_recent(&cache, window_id, now, &cfg);

        let mut candidate = AuthWindowCandidate {
            info: WindowInfo {
                app_name,
                window_title,
                window_id,
                bounds,
            },
            layer,
            alpha,
            area_ratio,
            center_dist_ratio,
            title_empty,
            click_inside,
            is_recent,
            score: 0,
        };

        score_auth_candidate(&mut candidate, &cfg, clicked_info_missing);

        if candidate.score >= cfg.score_threshold {
            candidates.push(candidate);
        }
    }

    if !current_ids.is_empty() {
        for window_id in current_ids {
            cache.last_seen.insert(window_id, now);
        }
        cache
            .last_seen
            .retain(|_, ts| now - *ts <= cfg.recent_window_ms * 10);
        cache.initialized = true;
    } else if !initialized {
        cache.initialized = true;
    }

    let mut best: Option<AuthWindowCandidate> = None;
    for candidate in candidates {
        let replace = match best {
            None => true,
            Some(ref current) => {
                if candidate.score > current.score {
                    true
                } else if candidate.score == current.score {
                    if candidate.layer > current.layer {
                        true
                    } else if candidate.layer == current.layer {
                        let cand_area = candidate.info.bounds.width as u64 * candidate.info.bounds.height as u64;
                        let cur_area = current.info.bounds.width as u64 * current.info.bounds.height as u64;
                        cand_area > cur_area
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        };
        if replace {
            best = Some(candidate);
        }
    }

    if let Some(ref c) = best {
        if cfg!(debug_assertions) {
            eprintln!(
                "Auth dialog heuristic: '{}' '{}' id={} layer={} score={} area_ratio={:.3} center_ratio={:.3} recent={} click_inside={} alpha={:.2}",
                c.info.app_name,
                c.info.window_title,
                c.info.window_id,
                c.layer,
                c.score,
                c.area_ratio,
                c.center_dist_ratio,
                c.is_recent,
                c.click_inside,
                c.alpha
            );
        }
    }

    Ok(best.map(|c| c.info))
}

#[cfg(not(target_os = "macos"))]
pub fn find_auth_dialog_window(
    _click_x: i32,
    _click_y: i32,
    _clicked_info_missing: bool,
) -> Result<Option<WindowInfo>, WindowError> {
    Ok(None)
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
            .map(|i| core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef)
            .collect()
    };

    // Windows are returned front-to-back when using kCGWindowListOptionOnScreenOnly
    // So we return the first window that contains the click point
    for window_dict in windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(window_dict)
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
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(v.as_CFTypeRef() as _) };

                let x = bounds_dict.find(CFString::new("X")).and_then(|n| n.to_i32()).unwrap_or(0);
                let y = bounds_dict.find(CFString::new("Y")).and_then(|n| n.to_i32()).unwrap_or(0);
                let width = bounds_dict.find(CFString::new("Width")).and_then(|n| n.to_i32()).unwrap_or(0) as u32;
                let height = bounds_dict.find(CFString::new("Height")).and_then(|n| n.to_i32()).unwrap_or(0) as u32;

                WindowBounds { x, y, width, height }
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
        let owner_pid = dict
            .find(owner_pid_key)
            .and_then(|v| {
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
                        let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                        s.to_string()
                    })
                    .unwrap_or_else(|| "Unknown".to_string())
            })
        } else {
            let owner_name_key = CFString::new("kCGWindowOwnerName");
            dict.find(owner_name_key)
                .map(|v| {
                    let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                    s.to_string()
                })
                .unwrap_or_else(|| "Unknown".to_string())
        };

        // Skip system UI windows (Dock, Spotlight, etc.) — they have full-screen
        // overlay windows at high layers that shadow real app windows beneath.
        if super::ax_helpers::is_system_ui_process(&app_name) {
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
            .map(|i| core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef)
            .collect()
    };

    let main = &main_window.bounds;
    let main_area = (main.width as i64) * (main.height as i64);
    let main_left = main.x;
    let main_top = main.y;
    let main_right = main.x + main.width as i32;
    let main_bottom = main.y + main.height as i32;

    for window_dict in windows {
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(window_dict)
        };

        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = match dict.find(bounds_key) {
            Some(v) => {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(v.as_CFTypeRef() as _) };

                let x = bounds_dict.find(CFString::new("X")).and_then(|n| n.to_i32()).unwrap_or(0);
                let y = bounds_dict.find(CFString::new("Y")).and_then(|n| n.to_i32()).unwrap_or(0);
                let width = bounds_dict.find(CFString::new("Width")).and_then(|n| n.to_i32()).unwrap_or(0) as u32;
                let height = bounds_dict.find(CFString::new("Height")).and_then(|n| n.to_i32()).unwrap_or(0) as u32;

                WindowBounds { x, y, width, height }
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
        let owner_pid = dict
            .find(owner_pid_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32()
            });

        let app_name = if let Some(pid) = owner_pid {
            get_process_name_by_pid(pid).unwrap_or_else(|| {
                let owner_name_key = CFString::new("kCGWindowOwnerName");
                dict.find(owner_name_key)
                    .map(|v| {
                        let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                        s.to_string()
                    })
                    .unwrap_or_else(|| "Unknown".to_string())
            })
        } else {
            let owner_name_key = CFString::new("kCGWindowOwnerName");
            dict.find(owner_name_key)
                .map(|v| {
                    let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                    s.to_string()
                })
                .unwrap_or_else(|| "Unknown".to_string())
        };

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
) -> Option<WindowInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn window_info_serializes() {
        let info = WindowInfo::sample();
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("Finder"));
        assert!(json.contains("Downloads"));
    }

    #[test]
    fn auth_candidate_scoring_balanced() {
        let mut cfg = AuthHeuristicConfig::default();
        cfg.layer_modal = 10;
        cfg.layer_status = 5;
        cfg.score_threshold = 6;

        let mut candidate = AuthWindowCandidate {
            info: WindowInfo::sample(),
            layer: 12,
            alpha: 1.0,
            area_ratio: 0.05,
            center_dist_ratio: 0.2,
            title_empty: true,
            click_inside: true,
            is_recent: true,
            score: 0,
        };

        let score = score_auth_candidate(&mut candidate, &cfg, true);
        assert!(score >= cfg.score_threshold);
        assert_eq!(candidate.score, score);
    }

    #[test]
    fn window_recency_logic() {
        let mut cfg = AuthHeuristicConfig::default();
        cfg.recent_window_ms = 500;

        let cache = WindowRecencyCache {
            initialized: true,
            last_seen: HashMap::from([(1, 900), (2, 100)]),
        };

        assert!(window_is_recent(&cache, 1, 1200, &cfg));
        assert!(!window_is_recent(&cache, 2, 1200, &cfg));
        assert!(window_is_recent(&cache, 3, 1200, &cfg));
    }

    // Regression: get_topmost_window_at_point must skip system UI windows (Dock, etc.)
    // so overlay windows like GIF pickers are found instead of being shadowed.
    // We can't call get_topmost_window_at_point in a unit test (needs live CGWindows),
    // but we verify the filter function correctly identifies system UI processes.
    #[test]
    fn system_ui_filter_catches_dock() {
        assert!(super::super::ax_helpers::is_system_ui_process("Dock"));
        assert!(super::super::ax_helpers::is_system_ui_process("dock"));
        assert!(super::super::ax_helpers::is_system_ui_process("WindowServer"));
        assert!(super::super::ax_helpers::is_system_ui_process("SystemUIServer"));
        assert!(super::super::ax_helpers::is_system_ui_process("ControlCenter"));
    }

    #[test]
    fn system_ui_filter_allows_normal_apps() {
        assert!(!super::super::ax_helpers::is_system_ui_process("WhatsApp"));
        assert!(!super::super::ax_helpers::is_system_ui_process("Safari"));
        assert!(!super::super::ax_helpers::is_system_ui_process("Finder"));
    }
}
