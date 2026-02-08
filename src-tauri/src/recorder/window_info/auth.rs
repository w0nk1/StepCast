//! Authentication dialog detection via heuristics (layer, geometry, timing).
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::{WindowBounds, WindowError, WindowInfo};

// --- Types ---

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

// --- Helpers ---

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

// --- Security agent detection ---

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

// --- Heuristic auth dialog detection ---

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

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

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
}
