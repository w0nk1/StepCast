//! Capture pipeline that orchestrates click → window info → screenshot → step creation.
//!
//! This module connects all the pieces of the recording flow:
//! - Receives a click event from the click listener
//! - Gets the frontmost window info
//! - Captures a screenshot of that window
//! - Creates a Step with the click position as percentages within the window

mod helpers;
mod types;

pub use helpers::{handle_auth_prompt, record_panel_bounds, record_tray_click, set_panel_visible};
pub use types::*;

use super::cg_capture::capture_window_cg;
use super::click_event::ClickEvent;
use super::macos_screencapture::capture_window as capture_window_by_id;
use super::pre_click_buffer::PreClickFrameBuffer;
use super::session::Session;
use super::types::{ActionType, AxClickInfo, CaptureStatus, Step};
use super::window_info::{
    find_attached_dialog_window, get_frontmost_window, get_main_window_for_pid,
    get_security_agent_window, get_topmost_window_at_point, get_window_for_pid_at_click,
    WindowBounds,
};
use helpers::*;

use super::ax_helpers::{
    get_clicked_element_info, get_clicked_element_label, is_security_agent_process,
    is_system_ui_process,
};

use std::sync::Mutex;

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

fn bounds_overlap_ratio(a: &WindowBounds, b: &WindowBounds) -> f32 {
    let a_left = a.x;
    let a_top = a.y;
    let a_right = a.x + a.width as i32;
    let a_bottom = a.y + a.height as i32;

    let b_left = b.x;
    let b_top = b.y;
    let b_right = b.x + b.width as i32;
    let b_bottom = b.y + b.height as i32;

    let inter_left = a_left.max(b_left);
    let inter_top = a_top.max(b_top);
    let inter_right = a_right.min(b_right);
    let inter_bottom = a_bottom.min(b_bottom);
    let inter_w = (inter_right - inter_left).max(0) as i64;
    let inter_h = (inter_bottom - inter_top).max(0) as i64;
    let inter_area = inter_w * inter_h;
    let a_area = (a.width as i64) * (a.height as i64);
    if a_area <= 0 {
        return 0.0;
    }
    inter_area as f32 / a_area as f32
}

fn bounds_contained_with_margin(inner: &WindowBounds, outer: &WindowBounds, margin: i32) -> bool {
    let inner_right = inner.x + inner.width as i32;
    let inner_bottom = inner.y + inner.height as i32;
    let outer_right = outer.x + outer.width as i32;
    let outer_bottom = outer.y + outer.height as i32;
    inner.x >= outer.x - margin
        && inner.y >= outer.y - margin
        && inner_right <= outer_right + margin
        && inner_bottom <= outer_bottom + margin
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowControlKind {
    Close,
    Minimize,
    Zoom,
}

impl WindowControlKind {
    fn label(self) -> &'static str {
        match self {
            Self::Close => "Close",
            Self::Minimize => "Minimize",
            Self::Zoom => "Zoom",
        }
    }

    fn subrole(self) -> &'static str {
        match self {
            Self::Close => "AXCloseButton",
            Self::Minimize => "AXMinimizeButton",
            Self::Zoom => "AXZoomButton",
        }
    }

    fn role_description(self) -> &'static str {
        match self {
            Self::Close => "Close button",
            Self::Minimize => "Minimize button",
            Self::Zoom => "Zoom button",
        }
    }
}

fn infer_window_control_kind(
    click_x: i32,
    click_y: i32,
    bounds: &WindowBounds,
    ax_role: Option<&str>,
    ax_subrole: Option<&str>,
    ax_role_description: Option<&str>,
) -> Option<WindowControlKind> {
    let role = ax_role.unwrap_or("");
    let role_lower = role.to_lowercase();
    if role == accessibility_sys::kAXMenuBarItemRole
        || role == accessibility_sys::kAXMenuItemRole
        || role == accessibility_sys::kAXMenuRole
        || role == accessibility_sys::kAXMenuButtonRole
        || role_lower.contains("menubar")
        || role_lower.contains("menuitem")
    {
        return None;
    }

    let sub = ax_subrole.unwrap_or("").to_lowercase();
    if sub.contains("close") {
        return Some(WindowControlKind::Close);
    }
    if sub.contains("minimize") {
        return Some(WindowControlKind::Minimize);
    }
    if sub.contains("zoom") {
        return Some(WindowControlKind::Zoom);
    }

    let desc = ax_role_description.unwrap_or("").to_lowercase();
    if desc.contains("schlie") || desc.contains("close") {
        return Some(WindowControlKind::Close);
    }
    if desc.contains("minim") {
        return Some(WindowControlKind::Minimize);
    }
    if desc.contains("zoom") {
        return Some(WindowControlKind::Zoom);
    }

    // Geometry fallback for cases where hit-testing returns root/application instead
    // of the traffic-light button (common during close animations).
    let role_allows_geometry = role == accessibility_sys::kAXButtonRole
        || role == accessibility_sys::kAXPopUpButtonRole
        || role_lower.contains("application")
        || role_lower.contains("window")
        || role_lower.contains("group")
        || role.is_empty();

    if !role_allows_geometry {
        return None;
    }

    let rel_x = click_x - bounds.x;
    let rel_y = click_y - bounds.y;
    if !(0..=92).contains(&rel_x) || !(0..=42).contains(&rel_y) {
        return None;
    }

    if rel_x <= 30 {
        Some(WindowControlKind::Close)
    } else if rel_x <= 60 {
        Some(WindowControlKind::Minimize)
    } else {
        Some(WindowControlKind::Zoom)
    }
}

/// Process a click event and create a step with screenshot.
///
/// This function orchestrates the full capture pipeline:
/// 1. Get frontmost window info
/// 2. Capture screenshot of that window
/// 3. Calculate click position as percentage within window bounds
/// 4. Create and return Step
///
/// # Arguments
///
/// * `click` - The click event to process
/// * `session` - The current recording session (used for step IDs and screenshot paths)
///
/// # Returns
///
/// Returns the created Step on success, or a PipelineError if any step fails.
pub fn process_click(
    click: &ClickEvent,
    session: &mut Session,
    pipeline_state: &Mutex<PipelineState>,
    pre_click_buffer: Option<&PreClickFrameBuffer>,
) -> Result<Step, PipelineError> {
    debug_log(
        session,
        &format!(
            "click: x={} y={} button={:?} count={} ts={}",
            click.x, click.y, click.button, click.click_count, click.timestamp_ms
        ),
    );

    session.diagnostics.clicks_received += 1;

    // Filter clicks on our panel / tray icon
    {
        let ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        if should_filter_panel_click(&ps, click) {
            debug_log(session, "filtered: panel click");
            session.diagnostics.clicks_filtered += 1;
            return Err(PipelineError::OwnAppClick);
        }
        if should_filter_tray_click(&ps, click) {
            debug_log(session, "filtered: tray click");
            session.diagnostics.clicks_filtered += 1;
            return Err(PipelineError::OwnAppClick);
        }
    }

    // 0a. Get info about the actual clicked element
    let clicked_info = get_clicked_element_info(click.x, click.y);
    let clicked_ax = get_clicked_element_label(click.x as f32, click.y as f32);
    if let Some(ax) = clicked_ax.as_ref() {
        if ax.role == accessibility_sys::kAXMenuBarItemRole {
            let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
            ps.last_menu_bar_click_ms = Some(click.timestamp_ms);
        }
        debug_log(
            session,
            &format!(
                "ax_click: role={} label='{}' win_role={:?} win_subrole={:?} top_role={:?} top_subrole={:?} parent_role={:?} parent_subrole={:?} checked={:?} cancel={} default={}",
                ax.role,
                ax.label,
                ax.window_role,
                ax.window_subrole,
                ax.top_level_role,
                ax.top_level_subrole,
                ax.parent_dialog_role,
                ax.parent_dialog_subrole,
                ax.is_checked,
                ax.is_cancel_button,
                ax.is_default_button
            ),
        );
    }

    let mut ax_info: Option<AxClickInfo> = clicked_ax.as_ref().map(|ax| AxClickInfo {
        role: ax.role.clone(),
        subrole: ax.subrole.clone(),
        role_description: ax.role_description.clone(),
        identifier: ax.identifier.clone(),
        label: ax.label.clone(),
        element_bounds: None,
        container_role: ax.container_role.clone(),
        container_subrole: ax.container_subrole.clone(),
        container_identifier: ax.container_identifier.clone(),
        window_role: ax.window_role.clone(),
        window_subrole: ax.window_subrole.clone(),
        top_level_role: ax.top_level_role.clone(),
        top_level_subrole: ax.top_level_subrole.clone(),
        parent_dialog_role: ax.parent_dialog_role.clone(),
        parent_dialog_subrole: ax.parent_dialog_subrole.clone(),
        is_checked: ax.is_checked,
        is_cancel_button: ax.is_cancel_button,
        is_default_button: ax.is_default_button,
    });

    // 0b. Filter clicks on our own app using Accessibility API
    if let Some((clicked_pid, ref clicked_app)) = clicked_info {
        let our_pid = std::process::id() as i32;
        let is_own_app = clicked_pid == our_pid || clicked_app.to_lowercase().contains("stepcast");

        if is_own_app {
            debug_log(
                session,
                &format!("filtered: own app click {clicked_app} (PID {clicked_pid})"),
            );
            if cfg!(debug_assertions) {
                eprintln!(
                    "Filtered own app click at ({}, {}): {clicked_app} (PID {clicked_pid})",
                    click.x, click.y
                );
            }
            session.diagnostics.clicks_filtered += 1;
            return Err(PipelineError::OwnAppClick);
        }
    }

    // 0c. Debounce rapid duplicate clicks (but allow double-click upgrades)
    let (should_debounce, should_upgrade) = {
        let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        is_debounced(
            &mut ps,
            click.timestamp_ms,
            click.x,
            click.y,
            click.click_count,
        )
    };

    if should_upgrade {
        // This is a double-click - upgrade the previous step
        if let Some(last_step) = session.last_step_mut() {
            if last_step.action == ActionType::Click {
                last_step.action = ActionType::DoubleClick;
                debug_log(session, "upgraded previous step to DoubleClick");
                if cfg!(debug_assertions) {
                    eprintln!(
                        "Upgraded previous step to DoubleClick at ({}, {})",
                        click.x, click.y
                    );
                }
                return Err(PipelineError::UpgradedToDblClick);
            }
        }
    }

    if should_debounce {
        debug_log(session, "filtered: debounced click");
        if cfg!(debug_assertions) {
            eprintln!("Debounced click at ({}, {})", click.x, click.y);
        }
        session.diagnostics.clicks_filtered += 1;
        return Err(PipelineError::DebouncedClick);
    }

    // 0d. Check cooldown after auth dialog clicks (phantom click prevention)
    {
        let ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(last_ts) = ps.last_auth_click_ms {
            let time_since_auth = click.timestamp_ms - last_ts;
            if time_since_auth > 0 && time_since_auth < AUTH_DIALOG_COOLDOWN_MS {
                debug_log(
                    session,
                    &format!("filtered: phantom click {time_since_auth}ms after auth dialog"),
                );
                if cfg!(debug_assertions) {
                    eprintln!(
                        "Filtered phantom click at ({}, {}) - {time_since_auth}ms after auth dialog",
                        click.x, click.y
                    );
                }
                session.diagnostics.clicks_filtered += 1;
                return Err(PipelineError::DebouncedClick);
            }
        }
    }

    // Check if click is on a security agent (Touch ID, password dialog)
    // Primary: heuristic window detection, fallback: process name list
    let mut auth_window = if let Some((_, ref clicked_app)) = clicked_info {
        if is_security_agent_process(clicked_app) {
            find_security_auth_window(click.x, click.y, clicked_info.is_none())
        } else {
            None
        }
    } else {
        find_security_auth_window(click.x, click.y, true)
    };

    if auth_window.is_none() {
        if let Ok(Some(window)) = get_security_agent_window() {
            auth_window = Some(window);
            if cfg!(debug_assertions) {
                eprintln!("Auth dialog detected via security agent name fallback");
            }
        }
    }

    let is_auth_dialog = auth_window.is_some()
        || clicked_info
            .as_ref()
            .map(|(_, app)| is_security_agent_process(app))
            .unwrap_or(false);

    if cfg!(debug_assertions) && is_auth_dialog {
        eprintln!("Detected auth dialog click at ({}, {})", click.x, click.y);
    }
    debug_log(
        session,
        &format!(
            "auth_detected={} clicked_info={} auth_window_id={}",
            is_auth_dialog,
            if clicked_info.is_some() {
                "some"
            } else {
                "none"
            },
            auth_window.as_ref().map(|w| w.window_id).unwrap_or(0)
        ),
    );

    // Record auth dialog click timestamp for phantom click prevention
    if is_auth_dialog {
        let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
        ps.last_auth_click_ms = Some(click.timestamp_ms);
    }

    // Fast-path for sheet dialog button clicks: capture immediately around the click.
    // This reduces the chance of capturing the close animation frame.
    let is_sheet_button_click = clicked_ax
        .as_ref()
        .map(|ax| {
            let is_sheet = ax.window_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                || ax.window_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                || ax.window_subrole.as_deref() == Some(accessibility_sys::kAXSystemDialogSubrole)
                || ax.top_level_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                || ax.top_level_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                || ax.top_level_subrole.as_deref()
                    == Some(accessibility_sys::kAXSystemDialogSubrole)
                || ax.parent_dialog_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                || ax.parent_dialog_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                || ax.parent_dialog_subrole.as_deref()
                    == Some(accessibility_sys::kAXSystemDialogSubrole)
                || ax.is_cancel_button
                || ax.is_default_button;
            let is_button = ax.role == accessibility_sys::kAXButtonRole
                || ax.role == accessibility_sys::kAXPopUpButtonRole;
            is_sheet && is_button
        })
        .unwrap_or(false);

    if is_sheet_button_click && !is_auth_dialog {
        let step_id = session.next_step_id();
        let screenshot_path = session.screenshot_path(&step_id);
        let (display_x, display_y, display_w, display_h) =
            get_display_bounds_for_click(click.x, click.y);
        let preferred_dialog_bounds = clicked_ax.as_ref().and_then(|ax| {
            ax.parent_dialog_bounds
                .clone()
                .or_else(|| ax.top_level_bounds.clone())
                .or_else(|| ax.window_bounds.clone())
        });

        // Prefer AX-derived dialog bounds with tight margins for cleaner output.
        let (mut region_x, mut region_y, mut region_width, mut region_height, mut bounds_source) =
            if let Some(bounds) = preferred_dialog_bounds {
                let mut x = bounds.x - 40;
                let mut y = bounds.y - 52;
                let mut w = bounds.width as i32 + 80;
                let mut h = bounds.height as i32 + 104;

                x = x.max(display_x);
                y = y.max(display_y);
                let right = (x + w).min(display_x + display_w);
                let bottom = (y + h).min(display_y + display_h);
                w = (right - x).max(420);
                h = (bottom - y).max(280);

                (x, y, w, h, "ax_bounds")
            } else {
                let w = display_w.clamp(500, 980);
                let h = display_h.clamp(420, 700);
                // Button is on right/lower part of dialog; bias left/up.
                let mut x = click.x - (w * 3 / 4);
                let mut y = click.y - (h * 3 / 4);
                let max_x = display_x + display_w - w;
                let max_y = display_y + display_h - h;
                x = x.clamp(display_x, max_x.max(display_x));
                y = y.clamp(display_y, max_y.max(display_y));
                (x, y, w, h, "fallback")
            };

        if let Ok(parent_window) = get_frontmost_window() {
            let parent = parent_window.bounds;
            let region_bounds = WindowBounds {
                x: region_x,
                y: region_y,
                width: region_width as u32,
                height: region_height as u32,
            };
            let region_area = (region_width as i64) * (region_height as i64);
            let parent_area = (parent.width as i64) * (parent.height as i64);
            let overlap = bounds_overlap_ratio(&region_bounds, &parent);
            if parent_area > region_area && overlap >= 0.45 {
                let union_x = region_x.min(parent.x);
                let union_y = region_y.min(parent.y);
                let union_right = (region_x + region_width).max(parent.x + parent.width as i32);
                let union_bottom = (region_y + region_height).max(parent.y + parent.height as i32);
                region_x = union_x;
                region_y = union_y;
                region_width = (union_right - union_x).max(region_width);
                region_height = (union_bottom - union_y).max(region_height);
                bounds_source = "ax_bounds+parent_union";
            }
        }

        region_width = region_width.min(display_w.max(1));
        region_height = region_height.min(display_h.max(1));

        let max_x = display_x + display_w - region_width;
        let max_y = display_y + display_h - region_height;
        region_x = region_x.clamp(display_x, max_x.max(display_x));
        region_y = region_y.clamp(display_y, max_y.max(display_y));

        debug_log(
            session,
            &format!(
                "sheet_fast_path: role={} label='{}' window_role={:?} source={} region=({}, {}, {}x{})",
                clicked_ax.as_ref().map(|a| a.role.as_str()).unwrap_or(""),
                clicked_ax.as_ref().map(|a| a.label.as_str()).unwrap_or(""),
                clicked_ax
                    .as_ref()
                    .and_then(|a| a.window_role.clone())
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.window_subrole.clone()))
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.top_level_role.clone()))
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.top_level_subrole.clone()))
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.parent_dialog_role.clone()))
                    .or_else(|| clicked_ax.as_ref().and_then(|a| a.parent_dialog_subrole.clone())),
                bounds_source,
                region_x,
                region_y,
                region_width,
                region_height
            ),
        );

        let app_name_for_log = clicked_info
            .as_ref()
            .map(|(_, app)| app.as_str())
            .unwrap_or("Application");
        let title_for_log = clicked_ax
            .as_ref()
            .and_then(|ax| {
                if ax.label.is_empty() {
                    None
                } else {
                    Some(ax.label.as_str())
                }
            })
            .unwrap_or("Dialog");
        debug_log(
            session,
            &format!(
                "screenshot_path={} window_id=0 title='{}' app='{}' (sheet_fast_path)",
                screenshot_path.to_string_lossy(),
                title_for_log,
                app_name_for_log
            ),
        );

        capture_region_best(
            session,
            region_x,
            region_y,
            region_width,
            region_height,
            &screenshot_path,
        )
        .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

        let click_x_percent =
            ((click.x - region_x) as f64 / region_width as f64 * 100.0).clamp(0.0, 100.0);
        let click_y_percent =
            ((click.y - region_y) as f64 / region_height as f64 * 100.0).clamp(0.0, 100.0);
        let capture_bounds = super::window_info::WindowBounds {
            x: region_x,
            y: region_y,
            width: region_width as u32,
            height: region_height as u32,
        };

        use super::click_event::MouseButton;
        let action = match (click.button, click.click_count) {
            (MouseButton::Right, _) => ActionType::RightClick,
            (MouseButton::Left, 2) => ActionType::DoubleClick,
            (MouseButton::Left, n) if n >= 3 => ActionType::DoubleClick,
            _ => ActionType::Click,
        };

        let app_name = clicked_info
            .as_ref()
            .map(|(_, app)| app.clone())
            .unwrap_or_else(|| "Application".to_string());
        let mut window_title = "Dialog".to_string();
        if let Some(ref ax) = clicked_ax {
            if !ax.label.is_empty() {
                window_title = format!("Dialog - {}", ax.label);
            }
        }

        let mut ax_info_for_step = ax_info.clone();
        if let (Some(ref mut info), Some(ax_label)) =
            (ax_info_for_step.as_mut(), clicked_ax.as_ref())
        {
            info.element_bounds = ax_label
                .element_bounds
                .as_ref()
                .and_then(|b| bounds_percent_in_capture(b, &capture_bounds));
        }
        let auto_crop_region = if should_apply_focus_crop(&capture_bounds, display_w, display_h) {
            suggested_focus_crop_for_capture(
                &capture_bounds,
                click_x_percent,
                click_y_percent,
                ax_info_for_step
                    .as_ref()
                    .and_then(|info| info.element_bounds.as_ref()),
            )
        } else {
            None
        };

        let step = Step {
            id: step_id,
            ts: click.timestamp_ms,
            action,
            x: click.x,
            y: click.y,
            click_x_percent: click_x_percent as f32,
            click_y_percent: click_y_percent as f32,
            app: app_name,
            window_title,
            screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
            note: None,
            description: None,
            description_source: None,
            description_status: None,
            description_error: None,
            ax: ax_info_for_step,
            capture_status: Some(CaptureStatus::Ok),
            capture_error: None,
            crop_region: auto_crop_region,
        };

        session.add_step(step.clone());
        return Ok(step);
    }

    // 1. Get the main (largest) window of the frontmost app
    let window_info =
        get_frontmost_window().map_err(|e| PipelineError::WindowInfoFailed(format!("{e}")))?;

    // Detect traffic-light window controls early and capture immediately.
    // This avoids dark "closing animation" frames for close/minimize/zoom clicks.
    let inferred_window_control = if !is_auth_dialog {
        let control_bounds = clicked_ax
            .as_ref()
            .and_then(|ax| {
                ax.top_level_bounds
                    .as_ref()
                    .or(ax.window_bounds.as_ref())
                    .or(ax.parent_dialog_bounds.as_ref())
            })
            .unwrap_or(&window_info.bounds);
        infer_window_control_kind(
            click.x,
            click.y,
            control_bounds,
            clicked_ax.as_ref().map(|a| a.role.as_str()),
            clicked_ax.as_ref().and_then(|a| a.subrole.as_deref()),
            clicked_ax
                .as_ref()
                .and_then(|a| a.role_description.as_deref()),
        )
    } else {
        None
    };

    if let Some(control) = inferred_window_control {
        if let Some(ref mut info) = ax_info {
            info.role = accessibility_sys::kAXButtonRole.to_string();
            info.subrole = Some(control.subrole().to_string());
            info.role_description = Some(control.role_description().to_string());
            if info.label.trim().is_empty() {
                info.label = control.label().to_string();
            }
        }

        let step_id = session.next_step_id();
        let screenshot_path = session.screenshot_path(&step_id);
        let (display_x, display_y, display_w, display_h) =
            get_display_bounds_for_click(click.x, click.y);

        let mut capture_bounds = clicked_ax
            .as_ref()
            .and_then(|ax| {
                ax.top_level_bounds
                    .clone()
                    .or_else(|| ax.window_bounds.clone())
                    .or_else(|| ax.parent_dialog_bounds.clone())
            })
            .unwrap_or_else(|| window_info.bounds.clone());

        // Clamp to clicked display bounds.
        let left = capture_bounds.x.max(display_x);
        let top = capture_bounds.y.max(display_y);
        let right = (capture_bounds.x + capture_bounds.width as i32).min(display_x + display_w);
        let bottom = (capture_bounds.y + capture_bounds.height as i32).min(display_y + display_h);
        if right > left && bottom > top {
            capture_bounds.x = left;
            capture_bounds.y = top;
            capture_bounds.width = (right - left) as u32;
            capture_bounds.height = (bottom - top) as u32;
        }

        debug_log(
            session,
            &format!(
                "window_control_fast_path: control={:?} bounds=({}, {}, {}x{})",
                control,
                capture_bounds.x,
                capture_bounds.y,
                capture_bounds.width,
                capture_bounds.height
            ),
        );
        debug_log(
            session,
            &format!(
                "screenshot_path={} window_id={} title='{}' app='{}' (window_control_fast_path)",
                screenshot_path.to_string_lossy(),
                window_info.window_id,
                window_info.window_title,
                window_info.app_name
            ),
        );

        capture_region_best(
            session,
            capture_bounds.x,
            capture_bounds.y,
            capture_bounds.width as i32,
            capture_bounds.height as i32,
            &screenshot_path,
        )
        .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

        if let (Some(ref mut info), Some(ax_label)) = (ax_info.as_mut(), clicked_ax.as_ref()) {
            info.element_bounds = ax_label
                .element_bounds
                .as_ref()
                .and_then(|b| bounds_percent_in_capture(b, &capture_bounds));
        }

        use super::click_event::MouseButton;
        let action = match (click.button, click.click_count) {
            (MouseButton::Right, _) => ActionType::RightClick,
            (MouseButton::Left, 2) => ActionType::DoubleClick,
            (MouseButton::Left, n) if n >= 3 => ActionType::DoubleClick,
            _ => ActionType::Click,
        };

        let click_x_percent =
            calculate_click_percent(click.x, capture_bounds.x, capture_bounds.width as i32);
        let click_y_percent =
            calculate_click_percent(click.y, capture_bounds.y, capture_bounds.height as i32);
        let auto_crop_region = if should_apply_focus_crop(&capture_bounds, display_w, display_h) {
            suggested_focus_crop_for_capture(
                &capture_bounds,
                click_x_percent,
                click_y_percent,
                ax_info
                    .as_ref()
                    .and_then(|info| info.element_bounds.as_ref()),
            )
        } else {
            None
        };

        let step = Step {
            id: step_id,
            ts: click.timestamp_ms,
            action,
            x: click.x,
            y: click.y,
            click_x_percent: click_x_percent as f32,
            click_y_percent: click_y_percent as f32,
            app: clicked_info
                .as_ref()
                .map(|(_, app)| app.clone())
                .unwrap_or_else(|| window_info.app_name.clone()),
            window_title: if window_info.window_title.trim().is_empty() {
                "Window".to_string()
            } else {
                window_info.window_title.clone()
            },
            screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
            note: None,
            description: None,
            description_source: None,
            description_status: None,
            description_error: None,
            ax: ax_info,
            capture_status: Some(CaptureStatus::Ok),
            capture_error: None,
            crop_region: auto_crop_region,
        };

        session.add_step(step.clone());
        return Ok(step);
    }

    // 2. Check if click is on a popup/menu window (only for frontmost app's windows)
    //    We look for smaller overlay windows that belong to the same app
    let topmost_at_click = get_topmost_window_at_point(click.x, click.y);

    // Determine which window to use for capture:
    // - For auth dialogs, use the security agent window
    // - If click is on a regular window from the SAME app (with title), use that window
    // - For popup/menus (empty title, smaller), use the popup window
    // - For system UI (Dock, Spotlight), use the main window
    let mut is_sheet_dialog = false;
    let attached_dialog_owner = clicked_info.as_ref().map(|(pid, app)| (*pid, app.as_str()));
    let attached_dialog = if !is_auth_dialog {
        if let Some(ref topmost) = topmost_at_click {
            if topmost.window_id == window_info.window_id {
                find_attached_dialog_window(click.x, click.y, &window_info, attached_dialog_owner)
            } else {
                None
            }
        } else {
            find_attached_dialog_window(click.x, click.y, &window_info, attached_dialog_owner)
        }
    } else {
        None
    };

    if let Some(ref dialog) = attached_dialog {
        debug_log(
            session,
            &format!(
                "attached_dialog_window: id={} bounds=({}, {}, {}x{}) title='{}' owner='{}'",
                dialog.window_id,
                dialog.bounds.x,
                dialog.bounds.y,
                dialog.bounds.width,
                dialog.bounds.height,
                dialog.window_title,
                dialog.app_name
            ),
        );
    }

    // Track whether capture_window came from topmost overlay detection.
    // When true, don't replace it in the clicked-app reconciliation block —
    // the overlay (e.g. GIF picker, popup) is the correct capture target.
    let mut capture_from_topmost = false;

    let capture_window = if is_auth_dialog {
        // For auth dialogs, prefer heuristic window, fallback to named security agent window
        if let Some(auth_window) = auth_window.clone() {
            debug_log(
                session,
                &format!(
                    "capture_window: auth heuristic id={} bounds=({}, {}, {}x{})",
                    auth_window.window_id,
                    auth_window.bounds.x,
                    auth_window.bounds.y,
                    auth_window.bounds.width,
                    auth_window.bounds.height
                ),
            );
            if cfg!(debug_assertions) {
                eprintln!(
                    "Using heuristic auth window for capture: '{}' id={} bounds=({}, {}, {}x{})",
                    auth_window.app_name,
                    auth_window.window_id,
                    auth_window.bounds.x,
                    auth_window.bounds.y,
                    auth_window.bounds.width,
                    auth_window.bounds.height
                );
            }
            auth_window
        } else if let Ok(Some(auth_window)) = get_security_agent_window() {
            debug_log(
                session,
                &format!(
                    "capture_window: auth fallback id={} bounds=({}, {}, {}x{})",
                    auth_window.window_id,
                    auth_window.bounds.x,
                    auth_window.bounds.y,
                    auth_window.bounds.width,
                    auth_window.bounds.height
                ),
            );
            if cfg!(debug_assertions) {
                eprintln!(
                    "Using security agent window for capture: '{}' id={} bounds=({}, {}, {}x{})",
                    auth_window.app_name,
                    auth_window.window_id,
                    auth_window.bounds.x,
                    auth_window.bounds.y,
                    auth_window.bounds.width,
                    auth_window.bounds.height
                );
            }
            auth_window
        } else {
            // Fallback to window_info if no auth window found
            window_info.clone()
        }
    } else if let Some(dialog) = attached_dialog {
        is_sheet_dialog = true;
        dialog
    } else if let Some(ref topmost) = topmost_at_click {
        let topmost_area = topmost.bounds.width as u64 * topmost.bounds.height as u64;
        let main_area = window_info.bounds.width as u64 * window_info.bounds.height as u64;

        // List of system apps we should NOT use as overlay capture
        // Use process names (language-independent) instead of localized app names
        let is_system_ui = is_system_ui_process(&topmost.app_name);

        // Consider "same app" if the topmost window matches either the frontmost window
        // OR the clicked element's app (from AX API). This handles menu-bar apps where the
        // frontmost window belongs to a different app than the one being clicked.
        let same_app = app_names_match(&topmost.app_name, &window_info.app_name)
            || clicked_info
                .as_ref()
                .is_some_and(|(_, clicked_app)| app_names_match(&topmost.app_name, clicked_app));
        let is_reasonable_size = topmost.bounds.width >= 50 && topmost.bounds.height >= 20;
        let overlaps_main = bounds_overlap_ratio(&topmost.bounds, &window_info.bounds);
        let contained_in_main =
            bounds_contained_with_margin(&topmost.bounds, &window_info.bounds, 12);
        let has_ax_dialog_hint = clicked_ax
            .as_ref()
            .map(|ax| {
                ax.window_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.window_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.window_subrole.as_deref()
                        == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.top_level_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.top_level_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.top_level_subrole.as_deref()
                        == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.parent_dialog_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.parent_dialog_subrole.as_deref()
                        == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.parent_dialog_subrole.as_deref()
                        == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.is_cancel_button
                    || ax.is_default_button
            })
            .unwrap_or(false);
        let is_foreign_dialog_like = !same_app
            && !is_system_ui
            && topmost.window_id != window_info.window_id
            && main_area > 0
            && topmost_area < main_area
            && has_ax_dialog_hint
            && (overlaps_main >= 0.55 || contained_in_main);

        // Regular same-app window WITH a title: use it (handles multiple windows of same app)
        let is_regular_same_app_window = same_app && !topmost.window_title.is_empty();
        // Menu/popup: empty title, smaller than main window, and same app
        let is_menu_popup = same_app && topmost.window_title.is_empty() && topmost_area < main_area;

        if !is_system_ui
            && is_reasonable_size
            && (is_regular_same_app_window || is_menu_popup || is_foreign_dialog_like)
        {
            capture_from_topmost = true;
            if is_foreign_dialog_like {
                is_sheet_dialog = true;
                debug_log(
                    session,
                    &format!(
                        "foreign_dialog_topmost: id={} owner='{}' bounds=({}, {}, {}x{}) overlap={:.2}",
                        topmost.window_id,
                        topmost.app_name,
                        topmost.bounds.x,
                        topmost.bounds.y,
                        topmost.bounds.width,
                        topmost.bounds.height,
                        overlaps_main
                    ),
                );
            }
            if cfg!(debug_assertions) {
                eprintln!(
                    "Using clicked window for capture: '{}' - '{}' (id={}, {}x{})",
                    topmost.app_name,
                    topmost.window_title,
                    topmost.window_id,
                    topmost.bounds.width,
                    topmost.bounds.height
                );
            }
            topmost.clone()
        } else {
            if cfg!(debug_assertions) && topmost.window_id != window_info.window_id {
                eprintln!(
                    "Ignoring topmost window '{}' - '{}' (system_ui={}, same_app={}, area_ratio={:.1}%)",
                    topmost.app_name, topmost.window_title, is_system_ui, same_app,
                    (topmost_area as f64 / main_area as f64) * 100.0
                );
            }
            window_info.clone()
        }
    } else {
        window_info.clone()
    };

    // Use clicked element's app name if available, otherwise captured window's app.
    // When the click targets a different app than the frontmost, also resolve that app's
    // main window so the capture bounds and screenshot match the actual click target.
    // EXCEPTION: when capture_window came from topmost overlay detection (e.g. GIF picker,
    // popup panel), keep it — the overlay is the correct capture target.
    let mut capture_window = capture_window;
    let (actual_app_name, mut actual_window_title) = if let Some((clicked_pid, ref clicked_app)) =
        clicked_info
    {
        if !app_names_match(clicked_app, &capture_window.app_name) && !capture_from_topmost {
            // Only switch capture windows when we can resolve a concrete window
            // under the click for the clicked PID. Falling back to the "largest"
            // window can jump to unrelated apps/windows.
            if let Some(clicked_window) = get_window_for_pid_at_click(
                clicked_pid,
                clicked_app,
                click.x,
                click.y,
                Some(capture_window.window_id),
            ) {
                if cfg!(debug_assertions) {
                    eprintln!(
                            "Resolved clicked app window at click: '{}' - '{}' id={} bounds=({}, {}, {}x{})",
                            clicked_window.app_name,
                            clicked_window.window_title,
                            clicked_window.window_id,
                            clicked_window.bounds.x,
                            clicked_window.bounds.y,
                            clicked_window.bounds.width,
                            clicked_window.bounds.height
                        );
                }
                let title = if clicked_window.window_title.is_empty() {
                    format!("Click on {clicked_app}")
                } else {
                    clicked_window.window_title.clone()
                };
                capture_window = clicked_window;
                (clicked_app.clone(), title)
            } else {
                let title = if capture_window.window_title.is_empty() {
                    format!("Click on {clicked_app}")
                } else {
                    capture_window.window_title.clone()
                };
                (clicked_app.clone(), title)
            }
        } else if capture_from_topmost && !app_names_match(clicked_app, &capture_window.app_name) {
            // Topmost overlay from different app: keep capture window, use clicked_app for label
            if cfg!(debug_assertions) {
                eprintln!(
                    "Keeping topmost overlay for capture (clicked_app='{}' != capture='{}')",
                    clicked_app, capture_window.app_name
                );
            }
            let title = if capture_window.window_title.is_empty() {
                format!("Click on {clicked_app}")
            } else {
                capture_window.window_title.clone()
            };
            (clicked_app.clone(), title)
        } else {
            (
                capture_window.app_name.clone(),
                capture_window.window_title.clone(),
            )
        }
    } else {
        (
            capture_window.app_name.clone(),
            capture_window.window_title.clone(),
        )
    };

    let mut resolved_window_title = actual_window_title.clone();

    if cfg!(debug_assertions) {
        eprintln!("Recording click on: {actual_app_name} - {actual_window_title}");
    }

    // Ignore pure menu-open clicks early (before allocating step IDs/screenshot paths).
    // This prevents reusing IDs and overwriting screenshots when the next click is a menu item.
    if !is_auth_dialog {
        if let Some(ax_label) = clicked_ax.as_ref() {
            let role = ax_label.role.as_str();
            if role == accessibility_sys::kAXMenuButtonRole {
                debug_log(
                    session,
                    &format!("ignored menu open: role={role} label='{}'", ax_label.label),
                );
                return Err(PipelineError::IgnoredMenuOpen);
            }
        }
    }

    // 2. Generate step ID and screenshot path
    let step_id = session.next_step_id();
    let screenshot_path = session.screenshot_path(&step_id);
    debug_log(
        session,
        &format!(
            "screenshot_path={} window_id={} title='{}' app='{}'",
            screenshot_path.to_string_lossy(),
            capture_window.window_id,
            capture_window.window_title,
            capture_window.app_name
        ),
    );

    // Check if click is on Dock (system UI at bottom of screen)
    let is_dock_click = if let Some((_, ref clicked_app)) = clicked_info {
        clicked_app.to_lowercase() == "dock"
    } else {
        false
    };

    // Filter out clicks on StepCast itself
    let is_stepcast_click = if let Some((_, ref clicked_app)) = clicked_info {
        let app_lower = clicked_app.to_lowercase();
        app_lower.contains("stepcast") || app_lower.contains("step cast")
    } else {
        false
    };

    if is_stepcast_click {
        if cfg!(debug_assertions) {
            eprintln!("Filtered click on StepCast app");
        }
        session.diagnostics.clicks_filtered += 1;
        return Err(PipelineError::DebouncedClick);
    }

    // Track capture outcome across all branches
    let mut final_capture_status = CaptureStatus::Ok;
    let mut final_capture_error: Option<String> = None;
    let (click_display_x, click_display_y, click_display_w, click_display_h) =
        get_display_bounds_for_click(click.x, click.y);

    // 3. Capture screenshot.
    // Pixel-first strategy: for regular clicks, prefer the pre-click full-display frame.
    // This preserves transient UI (web overlays/menus/popups) at click-time across apps/sites.
    //
    // Keep right-click and auth dialogs on dedicated paths:
    // - right-click wants post-click context menu capture
    // - auth dialogs may require secure placeholders/window-ID capture semantics
    let is_right_click = matches!(click.button, super::click_event::MouseButton::Right);
    const PRECLICK_MAX_AGE_MS: i64 = 250;
    let pre_click_fullframe_capture = if !is_right_click && !is_auth_dialog {
        if let Some(buffer) = pre_click_buffer {
            match buffer.capture_for_click(click.x, click.y, click.timestamp_ms, &screenshot_path) {
                Ok(Some(pre)) if (0..=PRECLICK_MAX_AGE_MS).contains(&pre.frame_age_ms) => {
                    debug_log(
                        session,
                        &format!(
                            "preclick_fullframe_capture ok: age_ms={} bounds=({}, {}, {}x{})",
                            pre.frame_age_ms,
                            pre.bounds.x,
                            pre.bounds.y,
                            pre.bounds.width,
                            pre.bounds.height
                        ),
                    );
                    Some(pre)
                }
                Ok(Some(pre)) => {
                    debug_log(
                        session,
                        &format!(
                            "preclick_fullframe_capture stale: age_ms={} (max={})",
                            pre.frame_age_ms, PRECLICK_MAX_AGE_MS
                        ),
                    );
                    None
                }
                Ok(None) => {
                    debug_log(session, "preclick_fullframe_capture unavailable");
                    None
                }
                Err(err) => {
                    debug_log(
                        session,
                        &format!("preclick_fullframe_capture failed: {err}"),
                    );
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let (click_x_percent, click_y_percent, capture_bounds_for_step) = if let Some(pre) =
        pre_click_fullframe_capture
    {
        let x_pct = calculate_click_percent(click.x, pre.bounds.x, pre.bounds.width as i32);
        let y_pct = calculate_click_percent(click.y, pre.bounds.y, pre.bounds.height as i32);
        (x_pct, y_pct, pre.bounds)
    } else if is_dock_click {
        // Dock click - capture zoomed region, centered on the clicked icon
        use core_graphics::display::CGDisplay;

        // Find the display containing the click
        let displays = CGDisplay::active_displays().unwrap_or_default();
        let mut display_bounds = CGDisplay::main().bounds(); // fallback to main

        for &disp_id in &displays {
            let disp = CGDisplay::new(disp_id);
            let bounds = disp.bounds();
            let in_display = click.x >= bounds.origin.x as i32
                && click.x < (bounds.origin.x + bounds.size.width) as i32
                && click.y >= bounds.origin.y as i32
                && click.y < (bounds.origin.y + bounds.size.height) as i32;
            if in_display {
                display_bounds = bounds;
                break;
            }
        }

        let display_x = display_bounds.origin.x as i32;
        let display_y = display_bounds.origin.y as i32;
        let display_width = display_bounds.size.width as i32;
        let display_height = display_bounds.size.height as i32;

        let region_width = 800.min(display_width.max(1));
        let region_height = 150.min(display_height.max(1));

        // Calculate click position relative to the display
        let click_rel_x = click.x - display_x;

        // Center the region on the click (but clamp to display bounds)
        let max_rel_x = (display_width - region_width).max(0);
        let region_rel_x = (click_rel_x - region_width / 2).max(0).min(max_rel_x);
        let region_x = display_x + region_rel_x;
        let region_y = display_y + display_height - region_height;
        let capture_bounds = super::window_info::WindowBounds {
            x: region_x,
            y: region_y,
            width: region_width as u32,
            height: region_height as u32,
        };

        capture_region_best(
            session,
            region_x,
            region_y,
            region_width,
            region_height,
            &screenshot_path,
        )
        .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

        // Calculate click position within the captured region
        let x_pct =
            ((click_rel_x - region_rel_x) as f64 / region_width as f64 * 100.0).clamp(0.0, 100.0);
        let y_pct = ((click.y - region_y) as f64 / region_height as f64 * 100.0).clamp(0.0, 100.0);
        (x_pct, y_pct, capture_bounds)
    } else if is_auth_dialog && capture_window.window_id > 0 {
        // Auth dialogs (Touch ID, password, WireGuard picker) - capture the SPECIFIC window by ID
        // Using screencapture -l ensures we capture only the dialog, not background windows
        let bounds = &capture_window.bounds;
        let capture_bounds = bounds.clone();

        if cfg!(debug_assertions) {
            eprintln!(
                "Auth dialog detected - window capture by ID: id={}, bounds=({}, {}, {}x{})",
                capture_window.window_id, bounds.x, bounds.y, bounds.width, bounds.height
            );
        }

        let capture_result = capture_window_by_id(capture_window.window_id, &screenshot_path);
        if let Err(err) = capture_result {
            debug_log(session, &format!("auth_window_capture_failed: {err}"));
            if cfg!(debug_assertions) {
                eprintln!("Auth window capture failed ({err}), falling back to region capture");
            }

            if bounds.width == 0 || bounds.height == 0 {
                return Err(PipelineError::ScreenshotFailed(format!("{err}")));
            }

            write_auth_placeholder(&screenshot_path, bounds.width, bounds.height)
                .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

            actual_window_title = "Authentication dialog (secure)".to_string();
            resolved_window_title = actual_window_title.clone();
            debug_log(session, "auth_placeholder_written");

            let x_pct = calculate_click_percent(click.x, bounds.x, bounds.width as i32);
            let y_pct = calculate_click_percent(click.y, bounds.y, bounds.height as i32);
            (x_pct, y_pct, capture_bounds.clone())
        } else {
            debug_log(session, "auth_window_capture: ok");
            // Calculate click position relative to window bounds
            let x_pct = calculate_click_percent(click.x, bounds.x, bounds.width as i32);
            let y_pct = calculate_click_percent(click.y, bounds.y, bounds.height as i32);
            (x_pct, y_pct, capture_bounds.clone())
        }
    } else if capture_window.window_id > 0 {
        // Check if click is in menubar region or in a menu interaction.

        let overlay_kind = classify_titleless_overlay_window(
            &capture_window.window_title,
            capture_window.window_id,
            window_info.window_id,
            &capture_window.bounds,
            click_display_y,
        );

        let click_y_in_display = click.y - click_display_y;
        let ax_role = clicked_ax.as_ref().map(|ax| ax.role.as_str());
        let is_right_click = matches!(click.button, super::click_event::MouseButton::Right);
        let is_dropdown_menu = overlay_kind == helpers::TitlelessOverlayKind::DropdownMenu;
        let recent_menu_open = {
            let mut ps = pipeline_state.lock().unwrap_or_else(|e| e.into_inner());
            let is_recent = ps
                .last_menu_bar_click_ms
                .map(|ts| {
                    let dt = click.timestamp_ms - ts;
                    (0..=2_500).contains(&dt)
                })
                .unwrap_or(false);
            if !is_recent {
                ps.last_menu_bar_click_ms = None;
            }
            is_recent
        };

        // Use menu-region capture only for true menu-bar/dropdown interactions near top.
        // Transient overlays (pickers/menus away from menu bar) are handled below with
        // window/popup bounds so we don't crop to the top strip.
        let use_menu_region_capture = should_use_menu_region_capture(
            overlay_kind,
            ax_role,
            click_y_in_display,
            recent_menu_open,
        );
        let use_region_capture = use_menu_region_capture;

        if resolved_window_title.is_empty() {
            if is_sheet_dialog {
                resolved_window_title = "Dialog".to_string();
            } else if overlay_kind == helpers::TitlelessOverlayKind::Popup {
                resolved_window_title = "Popup".to_string();
            } else if use_menu_region_capture || is_dropdown_menu {
                resolved_window_title = "Menu".to_string();
            } else {
                resolved_window_title = "Window".to_string();
            }
        }

        if use_region_capture {
            resolved_window_title = "Menu".to_string();
            // Menubar/dropdown click - capture a region around the click
            let region_height = 500.min(click_display_h.max(1)); // Include dropdown content
            let region_width = 600.min(click_display_w.max(1));

            // Center horizontally on click, clamped to clicked display bounds.
            let min_region_x = click_display_x;
            let max_region_x = (click_display_x + click_display_w - region_width).max(min_region_x);
            let region_x = (click.x - region_width / 2).clamp(min_region_x, max_region_x);
            // For dropdown clicks, start capture from top of the clicked display
            // (not global y=0) so secondary-display menubars are captured correctly.
            let region_y = click_display_y;

            // Capture the region
            capture_region_best(
                session,
                region_x,
                region_y,
                region_width,
                region_height,
                &screenshot_path,
            )
            .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

            // Calculate click position as percentage within captured region
            let x_pct =
                ((click.x - region_x) as f64 / region_width as f64 * 100.0).clamp(0.0, 100.0);
            let y_pct =
                ((click.y - region_y) as f64 / region_height as f64 * 100.0).clamp(0.0, 100.0);

            let mut ax_info_for_step = ax_info.clone();
            if let (Some(ref mut info), Some(ax_label)) =
                (ax_info_for_step.as_mut(), clicked_ax.as_ref())
            {
                let capture_bounds = super::window_info::WindowBounds {
                    x: region_x,
                    y: region_y,
                    width: region_width as u32,
                    height: region_height as u32,
                };
                info.element_bounds = ax_label
                    .element_bounds
                    .as_ref()
                    .and_then(|b| bounds_percent_in_capture(b, &capture_bounds));
            }

            let step = Step {
                id: step_id,
                ts: click.timestamp_ms,
                action: match (click.button, click.click_count) {
                    (super::click_event::MouseButton::Right, _) => ActionType::RightClick,
                    (super::click_event::MouseButton::Left, 2) => ActionType::DoubleClick,
                    (super::click_event::MouseButton::Left, n) if n >= 3 => ActionType::DoubleClick,
                    _ => ActionType::Click,
                },
                x: click.x,
                y: click.y,
                click_x_percent: x_pct as f32,
                click_y_percent: y_pct as f32,
                app: actual_app_name,
                window_title: resolved_window_title,
                screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
                note: None,
                description: None,
                description_source: None,
                description_status: None,
                description_error: None,
                ax: ax_info_for_step,
                capture_status: Some(CaptureStatus::Ok),
                capture_error: None,
                crop_region: None,
            };
            session.add_step(step.clone());
            return Ok(step);
        }

        let overlay_kind = classify_titleless_overlay_window(
            &capture_window.window_title,
            capture_window.window_id,
            window_info.window_id,
            &capture_window.bounds,
            click_display_y,
        );
        // Only treat titleless overlays as "popup menus" here when they're dropdown menus.
        // Other titleless overlays are typically in-app popovers and should be captured as windows.
        let is_popup_menu = overlay_kind == helpers::TitlelessOverlayKind::DropdownMenu;
        let prefer_transient_region_capture =
            should_prefer_transient_region_capture(ax_role, overlay_kind, is_right_click);

        // For right-clicks, poll for the context menu to appear.
        // macOS renders context menus asynchronously; a single fixed delay
        // sometimes captures before the menu is visible.  We poll a few
        // times with short sleeps (total max ~250ms) so the screenshot
        // reliably includes the menu.
        let context_menu_bounds = if is_right_click && !is_popup_menu {
            if cfg!(debug_assertions) {
                eprintln!(
                    "Looking for context menu near click ({}, {}) for app '{}'",
                    click.x, click.y, &capture_window.app_name
                );
            }
            let mut found = None;
            for attempt in 0..5 {
                std::thread::sleep(std::time::Duration::from_millis(if attempt == 0 {
                    80
                } else {
                    40
                }));
                found = find_context_menu_near_click(click.x, click.y, &capture_window.app_name);
                if found.is_some() {
                    debug_log(
                        session,
                        &format!("context_menu found on attempt {}", attempt + 1),
                    );
                    // Let the menu finish its open animation before measuring final bounds.
                    // Finder menus can be slow to populate (Quick Actions, extensions …).
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    // Re-measure — the menu may have grown during its animation
                    if let Some(refreshed) =
                        find_context_menu_near_click(click.x, click.y, &capture_window.app_name)
                    {
                        found = Some(refreshed);
                    }
                    break;
                }
            }
            found
        } else {
            None
        };

        if resolved_window_title.is_empty() {
            if is_sheet_dialog {
                resolved_window_title = "Dialog".to_string();
            } else if overlay_kind == helpers::TitlelessOverlayKind::Popup {
                resolved_window_title = "Popup".to_string();
            } else if is_popup_menu || context_menu_bounds.is_some() {
                resolved_window_title = "Menu".to_string();
            } else {
                resolved_window_title = "Window".to_string();
            }
        }

        // For popup menus, popovers, or right-click context menus: use region capture that includes
        // both base window and overlay/menu when available.
        let (use_region_capture, mut actual_bounds) = if is_sheet_dialog {
            // For sheets/dialogs, capture parent+dialog context (easier for users to follow)
            // instead of a cropped dialog-only image.
            let main = &window_info.bounds;
            let dialog = &capture_window.bounds;
            let union_x = main.x.min(dialog.x);
            let union_y = main.y.min(dialog.y);
            let union_right = (main.x + main.width as i32).max(dialog.x + dialog.width as i32);
            let union_bottom = (main.y + main.height as i32).max(dialog.y + dialog.height as i32);

            let mut union_bounds = super::window_info::WindowBounds {
                x: union_x,
                y: union_y,
                width: (union_right - union_x) as u32,
                height: (union_bottom - union_y) as u32,
            };

            let left = union_bounds.x.max(click_display_x);
            let top = union_bounds.y.max(click_display_y);
            let right =
                (union_bounds.x + union_bounds.width as i32).min(click_display_x + click_display_w);
            let bottom = (union_bounds.y + union_bounds.height as i32)
                .min(click_display_y + click_display_h);
            if right > left && bottom > top {
                union_bounds.x = left;
                union_bounds.y = top;
                union_bounds.width = (right - left) as u32;
                union_bounds.height = (bottom - top) as u32;
            }

            if cfg!(debug_assertions) {
                eprintln!(
                    "Dialog/sheet detected - using window+dialog union: ({}, {}, {}x{})",
                    union_bounds.x, union_bounds.y, union_bounds.width, union_bounds.height
                );
            }
            (true, union_bounds)
        } else if is_popup_menu {
            // Calculate union of main window and menu bounds
            let main = &window_info.bounds;
            let menu = &capture_window.bounds;

            let union_x = main.x.min(menu.x);
            let union_y = main.y.min(menu.y);
            let union_right = (main.x + main.width as i32).max(menu.x + menu.width as i32);
            let union_bottom = (main.y + main.height as i32).max(menu.y + menu.height as i32);

            let union_bounds = super::window_info::WindowBounds {
                x: union_x,
                y: union_y,
                width: (union_right - union_x) as u32,
                height: (union_bottom - union_y) as u32,
            };

            if cfg!(debug_assertions) {
                eprintln!(
                    "Popup menu detected - using region capture for window+menu union: ({}, {}, {}x{})",
                    union_bounds.x, union_bounds.y,
                    union_bounds.width, union_bounds.height
                );
            }
            (true, union_bounds)
        } else if overlay_kind == helpers::TitlelessOverlayKind::Popup {
            // In-app popovers (sticker picker, emoji picker, etc.): avoid tiny "overlay-only"
            // screenshots by capturing the union with the main window for context.
            let mut main_bounds = window_info.bounds.clone();
            if let Some((clicked_pid, clicked_app)) = clicked_info.as_ref() {
                if let Some(candidate) = get_window_for_pid_at_click(
                    *clicked_pid,
                    clicked_app,
                    click.x,
                    click.y,
                    Some(capture_window.window_id),
                ) {
                    main_bounds = candidate.bounds;
                } else if let Some(candidate) = get_main_window_for_pid(*clicked_pid, clicked_app) {
                    main_bounds = candidate.bounds;
                }
            }
            let main = &main_bounds;
            let popup = &capture_window.bounds;

            let union_x = main.x.min(popup.x);
            let union_y = main.y.min(popup.y);
            let union_right = (main.x + main.width as i32).max(popup.x + popup.width as i32);
            let union_bottom = (main.y + main.height as i32).max(popup.y + popup.height as i32);

            let mut union_bounds = super::window_info::WindowBounds {
                x: union_x,
                y: union_y,
                width: (union_right - union_x) as u32,
                height: (union_bottom - union_y) as u32,
            };

            // Clamp to clicked display bounds to avoid oversized multi-display captures.
            let left = union_bounds.x.max(click_display_x);
            let top = union_bounds.y.max(click_display_y);
            let right =
                (union_bounds.x + union_bounds.width as i32).min(click_display_x + click_display_w);
            let bottom = (union_bounds.y + union_bounds.height as i32)
                .min(click_display_y + click_display_h);
            if right > left && bottom > top {
                union_bounds.x = left;
                union_bounds.y = top;
                union_bounds.width = (right - left) as u32;
                union_bounds.height = (bottom - top) as u32;
            }

            if cfg!(debug_assertions) {
                eprintln!(
                    "Popup overlay detected - using union capture for context: ({}, {}, {}x{}), capture_from_topmost={}",
                    union_bounds.x,
                    union_bounds.y,
                    union_bounds.width,
                    union_bounds.height,
                    capture_from_topmost
                );
            }
            (true, union_bounds)
        } else if let Some(ref menu_bounds) = context_menu_bounds {
            // Right-click with context menu found - include both window and menu.
            // Add generous padding around the menu to account for macOS drop shadows
            // (~20-30px) and items that may render beyond reported window bounds.
            const MENU_PAD: i32 = 50;
            let main = &capture_window.bounds;

            let union_x = main.x.min(menu_bounds.x - MENU_PAD);
            let union_y = main.y.min(menu_bounds.y - MENU_PAD);
            let union_right = (main.x + main.width as i32)
                .max(menu_bounds.x + menu_bounds.width as i32 + MENU_PAD);
            let union_bottom = (main.y + main.height as i32)
                .max(menu_bounds.y + menu_bounds.height as i32 + MENU_PAD);

            let union_bounds = super::window_info::WindowBounds {
                x: union_x,
                y: union_y,
                width: (union_right - union_x) as u32,
                height: (union_bottom - union_y) as u32,
            };

            if cfg!(debug_assertions) {
                eprintln!(
                    "Right-click with context menu - using union: ({}, {}, {}x{})",
                    union_bounds.x, union_bounds.y, union_bounds.width, union_bounds.height
                );
            }
            (true, union_bounds)
        } else if prefer_transient_region_capture {
            // Volatile UI actions (menu rows, picker rows) can disappear during click handling.
            // Prefer immediate region capture over slower window-ID capture to preserve what the
            // user actually saw at click-time.
            debug_log(
                session,
                &format!(
                    "transient_region_capture: role={:?} overlay_kind={:?}",
                    ax_role, overlay_kind
                ),
            );
            let bounds = if overlay_kind == helpers::TitlelessOverlayKind::NotOverlay {
                // Keep OCR/context focused around click for weak AX roles (e.g. AXGroup),
                // instead of capturing the whole app window.
                let region_w = 1400.min(click_display_w.max(1));
                let region_h = 900.min(click_display_h.max(1));
                let min_x = click_display_x;
                let max_x = (click_display_x + click_display_w - region_w).max(min_x);
                let min_y = click_display_y;
                let max_y = (click_display_y + click_display_h - region_h).max(min_y);
                let x = (click.x - region_w / 2).clamp(min_x, max_x);
                let y = (click.y - region_h / 2).clamp(min_y, max_y);
                super::window_info::WindowBounds {
                    x,
                    y,
                    width: region_w as u32,
                    height: region_h as u32,
                }
            } else {
                capture_window.bounds.clone()
            };
            (true, bounds)
        } else {
            (false, capture_window.bounds.clone())
        };

        // Dialog sheet fallback for parent-window captures:
        // prefer AX window bounds when available, because CGWindow can point to the parent.
        let is_button_click = clicked_ax
            .as_ref()
            .map(|ax| {
                ax.role == accessibility_sys::kAXButtonRole
                    || ax.role == accessibility_sys::kAXPopUpButtonRole
            })
            .unwrap_or(false);
        let is_dialog_marker = clicked_ax
            .as_ref()
            .map(|ax| {
                ax.window_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.window_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.window_subrole.as_deref()
                        == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.top_level_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.top_level_subrole.as_deref() == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.top_level_subrole.as_deref()
                        == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.parent_dialog_role.as_deref() == Some(accessibility_sys::kAXSheetRole)
                    || ax.parent_dialog_subrole.as_deref()
                        == Some(accessibility_sys::kAXDialogSubrole)
                    || ax.parent_dialog_subrole.as_deref()
                        == Some(accessibility_sys::kAXSystemDialogSubrole)
                    || ax.is_cancel_button
                    || ax.is_default_button
            })
            .unwrap_or(false);
        let ax_window_bounds = clicked_ax.as_ref().and_then(|ax| {
            ax.parent_dialog_bounds
                .clone()
                .or_else(|| ax.top_level_bounds.clone())
                .or_else(|| ax.window_bounds.clone())
        });

        if !is_popup_menu
            && context_menu_bounds.is_none()
            && capture_window.window_id == window_info.window_id
            && is_button_click
        {
            if let Some(ax_bounds) = ax_window_bounds {
                let click_in_ax = click.x >= ax_bounds.x
                    && click.x < ax_bounds.x + ax_bounds.width as i32
                    && click.y >= ax_bounds.y
                    && click.y < ax_bounds.y + ax_bounds.height as i32;

                let ax_left = ax_bounds.x;
                let ax_top = ax_bounds.y;
                let ax_right = ax_bounds.x + ax_bounds.width as i32;
                let ax_bottom = ax_bounds.y + ax_bounds.height as i32;
                let main_left = window_info.bounds.x;
                let main_top = window_info.bounds.y;
                let main_right = window_info.bounds.x + window_info.bounds.width as i32;
                let main_bottom = window_info.bounds.y + window_info.bounds.height as i32;
                let inter_left = ax_left.max(main_left);
                let inter_top = ax_top.max(main_top);
                let inter_right = ax_right.min(main_right);
                let inter_bottom = ax_bottom.min(main_bottom);
                let inter_w = (inter_right - inter_left).max(0) as i64;
                let inter_h = (inter_bottom - inter_top).max(0) as i64;
                let inter_area = inter_w * inter_h;
                let ax_area = (ax_bounds.width as i64) * (ax_bounds.height as i64);
                let overlap_ratio = if ax_area > 0 {
                    inter_area as f32 / ax_area as f32
                } else {
                    0.0
                };
                let larger_than_main = ax_bounds.width > window_info.bounds.width + 24
                    || ax_bounds.height > window_info.bounds.height + 24;
                let should_use_ax_bounds =
                    click_in_ax && overlap_ratio >= 0.25 && (is_dialog_marker || larger_than_main);

                if should_use_ax_bounds {
                    let (display_x, display_y, display_w, display_h) =
                        get_display_bounds_for_click(click.x, click.y);
                    let left = ax_bounds.x.max(display_x);
                    let top = ax_bounds.y.max(display_y);
                    let right = (ax_bounds.x + ax_bounds.width as i32).min(display_x + display_w);
                    let bottom = (ax_bounds.y + ax_bounds.height as i32).min(display_y + display_h);
                    if right > left && bottom > top {
                        debug_log(
                            session,
                            &format!(
                                "ax_window_bounds_override: old=({}, {}, {}x{}) new=({}, {}, {}x{}) overlap={:.2} dialog_marker={} larger={}",
                                actual_bounds.x,
                                actual_bounds.y,
                                actual_bounds.width,
                                actual_bounds.height,
                                left,
                                top,
                                right - left,
                                bottom - top,
                                overlap_ratio,
                                is_dialog_marker,
                                larger_than_main
                            ),
                        );
                        actual_bounds.x = left;
                        actual_bounds.y = top;
                        actual_bounds.width = (right - left) as u32;
                        actual_bounds.height = (bottom - top) as u32;
                        if is_dialog_marker && resolved_window_title == window_info.window_title {
                            resolved_window_title = "Dialog".to_string();
                        }
                    }
                }
            } else if is_dialog_marker {
                // Reserve fallback when AX has dialog markers but no geometry.
                let (display_x, display_y, display_w, display_h) =
                    get_display_bounds_for_click(click.x, click.y);
                let left = (actual_bounds.x - 180).max(display_x);
                let top = (actual_bounds.y - 80).max(display_y);
                let right =
                    (actual_bounds.x + actual_bounds.width as i32 + 180).min(display_x + display_w);
                let bottom = (actual_bounds.y + actual_bounds.height as i32 + 140)
                    .min(display_y + display_h);
                if right > left && bottom > top {
                    debug_log(
                        session,
                        &format!(
                            "sheet_expand_fallback: old=({}, {}, {}x{}) new=({}, {}, {}x{})",
                            actual_bounds.x,
                            actual_bounds.y,
                            actual_bounds.width,
                            actual_bounds.height,
                            left,
                            top,
                            right - left,
                            bottom - top
                        ),
                    );
                    actual_bounds.x = left;
                    actual_bounds.y = top;
                    actual_bounds.width = (right - left) as u32;
                    actual_bounds.height = (bottom - top) as u32;
                    if resolved_window_title == window_info.window_title {
                        resolved_window_title = "Dialog".to_string();
                    }
                }
            }
        }

        // For normal window captures (no popup/context menu), try window-ID capture first.
        // This avoids race conditions where the window closes before the region capture
        // (e.g., clicking a close button). Fall back to region capture on failure.
        let mut capture_ok = false;
        let mut used_fallback = false;
        let mut last_capture_err: Option<String> = None;

        if prefer_transient_region_capture && !is_right_click {
            if let Some(buffer) = pre_click_buffer {
                match buffer.capture_for_click(
                    click.x,
                    click.y,
                    click.timestamp_ms,
                    &screenshot_path,
                ) {
                    Ok(Some(pre)) => {
                        actual_bounds = pre.bounds;
                        capture_ok = true;
                        debug_log(
                            session,
                            &format!(
                                "preclick_buffer_capture ok: age_ms={} bounds=({}, {}, {}x{})",
                                pre.frame_age_ms,
                                actual_bounds.x,
                                actual_bounds.y,
                                actual_bounds.width,
                                actual_bounds.height
                            ),
                        );
                    }
                    Ok(None) => {
                        debug_log(session, "preclick_buffer_capture unavailable");
                    }
                    Err(err) => {
                        debug_log(session, &format!("preclick_buffer_capture failed: {err}"));
                    }
                }
            }
        }

        if !use_region_capture && context_menu_bounds.is_none() && capture_window.window_id > 0 {
            if cfg!(debug_assertions) {
                eprintln!(
                    "Trying window-ID capture: id={} bounds=({}, {}, {}x{})",
                    capture_window.window_id,
                    actual_bounds.x,
                    actual_bounds.y,
                    actual_bounds.width,
                    actual_bounds.height,
                );
            }
            match capture_window_cg(capture_window.window_id, &screenshot_path) {
                Ok(()) if validate_screenshot(&screenshot_path) => {
                    debug_log(
                        session,
                        &format!("window_id_capture ok: id={}", capture_window.window_id),
                    );
                    capture_ok = true;
                }
                Ok(()) => {
                    debug_log(
                        session,
                        "window_id_capture produced empty file, falling back to region",
                    );
                    last_capture_err = Some("window capture produced empty file".to_string());
                }
                Err(err) => {
                    if cfg!(debug_assertions) {
                        eprintln!("Window-ID capture failed ({err}), falling back to region");
                    }
                    debug_log(
                        session,
                        &format!("window_id_capture failed: {err}, falling back to region"),
                    );
                    last_capture_err = Some(format!("{err}"));
                }
            }
        }

        if !capture_ok {
            if cfg!(debug_assertions) {
                eprintln!(
                    "Region capture: bounds=({}, {}, {}x{}) popup={}",
                    actual_bounds.x,
                    actual_bounds.y,
                    actual_bounds.width,
                    actual_bounds.height,
                    use_region_capture
                );
            }
            match capture_region_best(
                session,
                actual_bounds.x,
                actual_bounds.y,
                actual_bounds.width as i32,
                actual_bounds.height as i32,
                &screenshot_path,
            ) {
                Ok(()) if validate_screenshot(&screenshot_path) => {
                    if last_capture_err.is_some() {
                        used_fallback = true;
                    }
                    capture_ok = true;
                }
                Ok(()) => {
                    debug_log(session, "region_capture produced empty file");
                    last_capture_err = Some(
                        last_capture_err.unwrap_or_default()
                            + "; region capture produced empty file",
                    );
                }
                Err(e) => {
                    debug_log(session, &format!("region_capture failed: {e}"));
                    let msg = format!("{e}");
                    last_capture_err = Some(
                        last_capture_err
                            .map(|prev| format!("{prev}; {msg}"))
                            .unwrap_or(msg),
                    );
                }
            }
        }

        // Record capture outcome
        if capture_ok && used_fallback {
            final_capture_status = CaptureStatus::Fallback;
            final_capture_error = last_capture_err.clone();
            session.diagnostics.captures_fallback += 1;
            if let Some(ref reason) = last_capture_err {
                session.diagnostics.failure_reasons.push(reason.clone());
            }
        } else if !capture_ok {
            final_capture_status = CaptureStatus::Failed;
            final_capture_error = last_capture_err.clone();
            session.diagnostics.captures_failed += 1;
            if let Some(ref reason) = last_capture_err {
                session.diagnostics.failure_reasons.push(reason.clone());
            }
        }

        if cfg!(debug_assertions) {
            eprintln!(
                "Click calc: click=({}, {}), capture_bounds=(x={}, y={}, w={}, h={})",
                click.x,
                click.y,
                actual_bounds.x,
                actual_bounds.y,
                actual_bounds.width,
                actual_bounds.height
            );
        }

        // Calculate click position relative to the CAPTURED window bounds
        let x_pct = calculate_click_percent(click.x, actual_bounds.x, actual_bounds.width as i32);
        let y_pct = calculate_click_percent(click.y, actual_bounds.y, actual_bounds.height as i32);

        if cfg!(debug_assertions) {
            eprintln!("Click percent: x={x_pct}%, y={y_pct}%");
        }

        (x_pct, y_pct, actual_bounds)
    } else {
        // No valid window - check if click is in menubar/dropdown region on the clicked display,
        // otherwise capture the full clicked display.
        let screen_width = click_display_w;
        let screen_height = click_display_h;
        let screen_x = click_display_x;
        let screen_y = click_display_y;
        let click_y_in_display = click.y - screen_y;

        // Menubar and dropdown region (top portion of screen)
        // This covers: menubar clicks, dropdown menus, status menu popups
        const MENUBAR_REGION_HEIGHT: i32 = 500;

        if (0..MENUBAR_REGION_HEIGHT).contains(&click_y_in_display) {
            // Use fixed region size and center on click (in global coordinates)
            // Global coordinates can be negative for displays left of primary
            let region_width = 800.min(screen_width.max(1));
            let region_height = MENUBAR_REGION_HEIGHT.min(screen_height);

            // Center horizontally on click, clamped to clicked display bounds.
            let min_region_x = screen_x;
            let max_region_x = (screen_x + screen_width - region_width).max(min_region_x);
            let region_x = (click.x - region_width / 2).clamp(min_region_x, max_region_x);
            let capture_bounds = super::window_info::WindowBounds {
                x: region_x,
                y: screen_y,
                width: region_width as u32,
                height: region_height as u32,
            };

            if cfg!(debug_assertions) {
                eprintln!(
                    "Top-region click at ({}, {}), using focused capture ({}, {}, {}x{})",
                    click.x, click.y, region_x, screen_y, region_width, region_height
                );
            }

            capture_region_best(
                session,
                region_x,
                screen_y,
                region_width,
                region_height,
                &screenshot_path,
            )
            .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

            let x_pct = if region_width > 0 {
                ((click.x - region_x) as f64 / region_width as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                50.0
            };
            let y_pct = if region_height > 0 {
                ((click.y - screen_y) as f64 / region_height as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                50.0
            };
            (x_pct, y_pct, capture_bounds)
        } else {
            // Fullscreen capture for clicks in lower screen area without window
            if cfg!(debug_assertions) {
                eprintln!("No valid window_id, using fullscreen capture");
            }
            let capture_bounds = super::window_info::WindowBounds {
                x: screen_x,
                y: screen_y,
                width: screen_width as u32,
                height: screen_height as u32,
            };
            capture_region_best(
                session,
                screen_x,
                screen_y,
                screen_width,
                screen_height,
                &screenshot_path,
            )
            .map_err(|e| PipelineError::ScreenshotFailed(format!("{e}")))?;

            let x_pct = if screen_width > 0 {
                ((click.x - screen_x) as f64 / screen_width as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                50.0
            };
            let y_pct = if screen_height > 0 {
                ((click.y - screen_y) as f64 / screen_height as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                50.0
            };
            (x_pct, y_pct, capture_bounds)
        }
    };

    if let (Some(ref mut ax_info), Some(ax_label)) = (ax_info.as_mut(), clicked_ax.as_ref()) {
        ax_info.element_bounds = ax_label
            .element_bounds
            .as_ref()
            .and_then(|b| bounds_percent_in_capture(b, &capture_bounds_for_step));
    }

    if resolved_window_title.is_empty() {
        if is_sheet_dialog {
            resolved_window_title = "Dialog".to_string();
        } else {
            resolved_window_title = "Window".to_string();
        }
    }

    if !is_auth_dialog {
        if let Some(ax_label) = clicked_ax {
            let role = ax_label.role.as_str();
            let label = ax_label.label;
            let is_menu_item = role == accessibility_sys::kAXMenuItemRole;
            let is_button = role == accessibility_sys::kAXButtonRole
                || role == accessibility_sys::kAXPopUpButtonRole;

            if is_menu_item {
                resolved_window_title = format!("Menu - {label}");
            } else if is_button {
                if resolved_window_title == "Window" || resolved_window_title == "Menu" {
                    resolved_window_title = format!("Button - {label}");
                } else if !resolved_window_title.contains(&label) {
                    resolved_window_title = format!("{resolved_window_title} - {label}");
                }
            }
        }
    }

    // 6. Determine action type based on click count and button
    use super::click_event::MouseButton;
    let action = match (click.button, click.click_count) {
        (MouseButton::Right, _) => ActionType::RightClick,
        (MouseButton::Left, 2) => ActionType::DoubleClick,
        (MouseButton::Left, n) if n >= 3 => ActionType::DoubleClick, // Triple+ as double
        _ => ActionType::Click,
    };

    let auto_crop_region = if final_capture_status != CaptureStatus::Failed
        && !is_auth_dialog
        && should_apply_focus_crop(&capture_bounds_for_step, click_display_w, click_display_h)
    {
        suggested_focus_crop_for_capture(
            &capture_bounds_for_step,
            click_x_percent,
            click_y_percent,
            ax_info
                .as_ref()
                .and_then(|info| info.element_bounds.as_ref()),
        )
    } else {
        None
    };

    // 7. Create step
    let screenshot = if final_capture_status == CaptureStatus::Failed {
        None
    } else {
        Some(screenshot_path.to_string_lossy().to_string())
    };
    let step = Step {
        id: step_id,
        ts: click.timestamp_ms,
        action,
        x: click.x,
        y: click.y,
        click_x_percent: click_x_percent as f32,
        click_y_percent: click_y_percent as f32,
        app: actual_app_name,
        window_title: resolved_window_title,
        screenshot_path: screenshot,
        note: None,
        description: None,
        description_source: None,
        description_status: None,
        description_error: None,
        ax: ax_info,
        capture_status: Some(final_capture_status),
        capture_error: final_capture_error,
        crop_region: auto_crop_region,
    };

    // 8. Add to session
    session.add_step(step.clone());

    Ok(step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::click_event::{ClickEvent, MouseButton};
    use std::path::Path;

    #[test]
    fn calculate_click_percent_at_origin() {
        // Click at window origin should be 0%
        let percent = calculate_click_percent(100, 100, 800);
        assert!((percent - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_at_end() {
        // Click at window edge should be 100%
        let percent = calculate_click_percent(900, 100, 800);
        assert!((percent - 100.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_in_middle() {
        // Click in middle should be 50%
        let percent = calculate_click_percent(500, 100, 800);
        assert!((percent - 50.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_handles_zero_size() {
        // Zero window size should return 0
        let percent = calculate_click_percent(100, 50, 0);
        assert!((percent - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_handles_negative_size() {
        // Negative window size should return 0
        let percent = calculate_click_percent(100, 50, -10);
        assert!((percent - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_clamps_below_zero() {
        // Click before window should clamp to 0%
        let percent = calculate_click_percent(50, 100, 800);
        assert!((percent - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_click_percent_clamps_above_hundred() {
        // Click after window should clamp to 100%
        let percent = calculate_click_percent(1000, 100, 800);
        assert!((percent - 100.0).abs() < 0.001);
    }

    #[test]
    fn pipeline_error_displays_correctly() {
        let err = PipelineError::WindowInfoFailed("no app".to_string());
        assert!(err.to_string().contains("window info failed"));
        assert!(err.to_string().contains("no app"));

        let err = PipelineError::ScreenshotFailed("capture failed".to_string());
        assert!(err.to_string().contains("screenshot failed"));
        assert!(err.to_string().contains("capture failed"));
    }

    #[test]
    fn app_name_match_normalizes_hidden_chars() {
        assert!(app_names_match("‎WhatsApp", "WhatsApp"));
        assert!(app_names_match("WireGuard", "wireguard"));
    }

    #[test]
    fn app_name_match_rejects_different_names() {
        assert!(!app_names_match("Finder", "Preview"));
    }

    // Note: is_click_on_own_app uses the Accessibility API and requires
    // actual UI elements to test, so we can't easily unit test it.
    // It's tested manually by running the app.

    // --- TrayRect::contains ---

    #[test]
    fn tray_rect_contains_point_inside() {
        let rect = TrayRect {
            x: 100,
            y: 0,
            width: 30,
            height: 22,
        };
        assert!(rect.contains(115, 11));
    }

    #[test]
    fn tray_rect_contains_top_left_corner() {
        let rect = TrayRect {
            x: 100,
            y: 0,
            width: 30,
            height: 22,
        };
        assert!(rect.contains(100, 0));
    }

    #[test]
    fn tray_rect_excludes_bottom_right_edge() {
        let rect = TrayRect {
            x: 100,
            y: 0,
            width: 30,
            height: 22,
        };
        // x < x + width, y < y + height (exclusive)
        assert!(!rect.contains(130, 22));
    }

    #[test]
    fn tray_rect_excludes_point_outside() {
        let rect = TrayRect {
            x: 100,
            y: 0,
            width: 30,
            height: 22,
        };
        assert!(!rect.contains(50, 10));
        assert!(!rect.contains(200, 10));
        assert!(!rect.contains(115, 30));
    }

    // --- PanelRect::contains ---

    #[test]
    fn panel_rect_contains_point_inside() {
        let rect = PanelRect {
            x: 50,
            y: 30,
            width: 340,
            height: 640,
        };
        assert!(rect.contains(200, 300));
    }

    #[test]
    fn panel_rect_excludes_point_outside() {
        let rect = PanelRect {
            x: 50,
            y: 30,
            width: 340,
            height: 640,
        };
        assert!(!rect.contains(0, 300));
        assert!(!rect.contains(400, 300));
        assert!(!rect.contains(200, 700));
    }

    // --- is_debounced ---

    #[test]
    fn first_click_is_not_debounced() {
        let mut ps = PipelineState::new();
        let (debounced, upgrade) = is_debounced(&mut ps, 1000, 100, 200, 1);
        assert!(!debounced);
        assert!(!upgrade);
    }

    #[test]
    fn same_position_within_threshold_is_debounced() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1050, 102, 201, 1);
        assert!(debounced);
        assert!(!upgrade);
    }

    #[test]
    fn same_position_after_threshold_is_not_debounced() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1200, 102, 201, 1);
        assert!(!debounced);
        assert!(!upgrade);
    }

    #[test]
    fn different_position_within_threshold_is_not_debounced() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1050, 200, 300, 1);
        assert!(!debounced);
        assert!(!upgrade);
    }

    #[test]
    fn double_click_upgrades_previous() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1100, 101, 201, 2);
        assert!(!debounced);
        assert!(upgrade);
    }

    #[test]
    fn double_click_at_different_position_does_not_upgrade() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1100, 200, 300, 2);
        assert!(!debounced);
        assert!(!upgrade);
    }

    #[test]
    fn double_click_after_timeout_does_not_upgrade() {
        let mut ps = PipelineState::new();
        is_debounced(&mut ps, 1000, 100, 200, 1);
        let (debounced, upgrade) = is_debounced(&mut ps, 1600, 101, 201, 2);
        assert!(!debounced);
        assert!(!upgrade);
    }

    // --- PipelineState::reset ---

    #[test]
    fn pipeline_state_reset_clears_all() {
        let mut ps = PipelineState::new();
        ps.last_click = Some((1000, 100, 200, 1));
        ps.last_auth_click_ms = Some(500);
        ps.last_tray_click = Some(TrayClick {
            rect: TrayRect {
                x: 0,
                y: 0,
                width: 30,
                height: 22,
            },
            timestamp_ms: 999,
        });
        ps.panel_state.visible = true;
        ps.panel_state.rect = Some(PanelRect {
            x: 50,
            y: 30,
            width: 340,
            height: 640,
        });
        ps.last_auth_prompt = Some((42, 1000));

        ps.reset();

        assert!(ps.last_click.is_none());
        assert!(ps.last_auth_click_ms.is_none());
        assert!(ps.last_tray_click.is_none());
        assert!(!ps.panel_state.visible);
        assert!(ps.panel_state.rect.is_none());
        assert!(ps.last_auth_prompt.is_none());
    }

    // --- Negative coordinates (multi-monitor) ---

    #[test]
    fn calculate_click_percent_negative_offsets() {
        // Secondary monitor left of primary: window at x=-1440, click at x=-720
        let percent = calculate_click_percent(-720, -1440, 1440);
        assert!((percent - 50.0).abs() < 0.001);
    }

    #[test]
    fn debounce_handles_negative_coords() {
        let mut ps = PipelineState::new();
        let (d, u) = is_debounced(&mut ps, 1000, -500, -200, 1);
        assert!(!d);
        assert!(!u);
        // Same position within threshold
        let (d2, _) = is_debounced(&mut ps, 1050, -498, -199, 1);
        assert!(d2);
    }

    // --- should_filter_tray_click with PipelineState ---

    #[test]
    fn filter_tray_click_within_window() {
        let mut ps = PipelineState::new();
        ps.last_tray_click = Some(TrayClick {
            rect: TrayRect {
                x: 100,
                y: 0,
                width: 30,
                height: 22,
            },
            timestamp_ms: 1000,
        });
        let click = ClickEvent {
            x: 115,
            y: 11,
            button: MouseButton::Left,
            click_count: 1,
            timestamp_ms: 1500,
        };
        assert!(should_filter_tray_click(&ps, &click));
    }

    #[test]
    fn filter_tray_click_expired() {
        let mut ps = PipelineState::new();
        ps.last_tray_click = Some(TrayClick {
            rect: TrayRect {
                x: 100,
                y: 0,
                width: 30,
                height: 22,
            },
            timestamp_ms: 1000,
        });
        let click = ClickEvent {
            x: 115,
            y: 11,
            button: MouseButton::Left,
            click_count: 1,
            timestamp_ms: 3000, // > 1s after tray click
        };
        assert!(!should_filter_tray_click(&ps, &click));
    }

    // --- should_filter_panel_click with PipelineState ---

    #[test]
    fn filter_panel_click_visible() {
        let mut ps = PipelineState::new();
        ps.panel_state.visible = true;
        ps.panel_state.rect = Some(PanelRect {
            x: 50,
            y: 30,
            width: 340,
            height: 640,
        });
        let click = ClickEvent {
            x: 200,
            y: 300,
            button: MouseButton::Left,
            click_count: 1,
            timestamp_ms: 1000,
        };
        assert!(should_filter_panel_click(&ps, &click));
    }

    #[test]
    fn filter_panel_click_hidden() {
        let mut ps = PipelineState::new();
        ps.panel_state.visible = false;
        ps.panel_state.rect = Some(PanelRect {
            x: 50,
            y: 30,
            width: 340,
            height: 640,
        });
        let click = ClickEvent {
            x: 200,
            y: 300,
            button: MouseButton::Left,
            click_count: 1,
            timestamp_ms: 1000,
        };
        assert!(!should_filter_panel_click(&ps, &click));
    }

    // --- should_emit_auth_prompt dedup ---

    #[test]
    fn auth_prompt_first_emission() {
        let mut ps = PipelineState::new();
        assert!(should_emit_auth_prompt(&mut ps, 42, 1000));
        assert_eq!(ps.last_auth_prompt, Some((42, 1000)));
    }

    #[test]
    fn auth_prompt_dedup_same_window_within_cooldown() {
        let mut ps = PipelineState::new();
        assert!(should_emit_auth_prompt(&mut ps, 42, 1000));
        // Same window, within AUTH_PROMPT_DEDUP_MS (5000ms)
        assert!(!should_emit_auth_prompt(&mut ps, 42, 3000));
    }

    #[test]
    fn auth_prompt_emits_after_cooldown() {
        let mut ps = PipelineState::new();
        assert!(should_emit_auth_prompt(&mut ps, 42, 1000));
        // Same window, after AUTH_PROMPT_DEDUP_MS
        assert!(should_emit_auth_prompt(&mut ps, 42, 7000));
    }

    #[test]
    fn auth_prompt_emits_for_different_window() {
        let mut ps = PipelineState::new();
        assert!(should_emit_auth_prompt(&mut ps, 42, 1000));
        // Different window ID, even within cooldown
        assert!(should_emit_auth_prompt(&mut ps, 99, 2000));
    }

    #[test]
    fn infer_window_control_from_subrole() {
        let bounds = WindowBounds {
            x: 100,
            y: 100,
            width: 800,
            height: 600,
        };
        let kind = infer_window_control_kind(
            120,
            116,
            &bounds,
            Some(accessibility_sys::kAXButtonRole),
            Some("AXCloseButton"),
            None,
        );
        assert_eq!(kind, Some(WindowControlKind::Close));
    }

    #[test]
    fn infer_window_control_from_geometry_fallback() {
        let bounds = WindowBounds {
            x: 200,
            y: 80,
            width: 1200,
            height: 900,
        };
        let kind = infer_window_control_kind(
            214, // near top-left traffic light
            96,
            &bounds,
            Some("AXApplication"),
            None,
            None,
        );
        assert_eq!(kind, Some(WindowControlKind::Close));
    }

    #[test]
    fn infer_window_control_rejects_non_control_area() {
        let bounds = WindowBounds {
            x: 200,
            y: 80,
            width: 1200,
            height: 900,
        };
        let kind = infer_window_control_kind(
            500,
            400,
            &bounds,
            Some(accessibility_sys::kAXButtonRole),
            None,
            None,
        );
        assert_eq!(kind, None);
    }

    // --- validate_screenshot ---

    #[test]
    fn validate_screenshot_nonexistent() {
        assert!(!validate_screenshot(Path::new(
            "/tmp/definitely_not_a_real_file_xyz.png"
        )));
    }

    #[test]
    fn validate_screenshot_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.png");
        std::fs::write(&path, b"").unwrap();
        assert!(!validate_screenshot(&path));
    }

    #[test]
    fn validate_screenshot_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("valid.png");
        std::fs::write(&path, b"PNG data here").unwrap();
        assert!(validate_screenshot(&path));
    }
}
