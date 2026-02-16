// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod apple_intelligence;
mod export;
mod i18n;
mod panel;
mod recorder;
mod startup_state;
mod tray;
use recorder::click_listener::ClickListener;
use recorder::pipeline;
use recorder::session::Session;
use recorder::state::{RecorderState, SessionState};
use recorder::types::{ActionType, BoundsPercent, DescriptionSource, DescriptionStatus, Step};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
#[cfg(not(debug_assertions))]
use tauri_plugin_aptabase::EventTracker;

#[cfg(target_os = "macos")]
fn permission_debug_log(message: &str) {
    use std::io::Write;

    let Some(cache) = dirs::cache_dir() else {
        return;
    };
    let dir = cache.join("com.w0nk1.stepcast");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("permissions.log");

    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f%:z");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "[{ts}] {message}");
    }
}

#[cfg(debug_assertions)]
fn session_debug_log(session_dir: &std::path::Path, message: &str) {
    use std::io::Write;

    let log_path = session_dir.join("recording.log");
    let is_new = !log_path.exists();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        if is_new {
            let _ = writeln!(file, "session_dir={}", session_dir.to_string_lossy());
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let _ = writeln!(file, "[{ts}] {message}");
    }
}

#[cfg(debug_assertions)]
fn write_session_json(session_dir: &std::path::Path, filename: &str, value: &serde_json::Value) {
    let path = session_dir.join(filename);
    if let Ok(s) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(path, s);
    }
}

#[cfg(debug_assertions)]
fn json_escape_one_line(s: &str) -> String {
    // Keep `recording.log` one-result-per-line for easy grep.
    s.replace(['\n', '\r', '\t'], " ")
}

struct RecorderAppState {
    recorder_state: Mutex<RecorderState>,
    session: Mutex<Option<Session>>,
    click_listener: Mutex<Option<ClickListener>>,
    pre_click_buffer: Mutex<Option<recorder::pre_click_buffer::PreClickFrameBuffer>>,
    processing_running: Arc<AtomicBool>,
    pipeline_state: Mutex<pipeline::PipelineState>,
    ai_descriptions_running: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
struct PermissionStatus {
    screen_recording: bool,
    accessibility: bool,
}

#[derive(Debug, Clone, Serialize)]
struct AppleIntelligenceEligibility {
    eligible: bool,
    reason: String,
    /// Best-effort details for debugging (platform/version/arch).
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

const SCREEN_RECORDING_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

#[cfg(target_os = "macos")]
fn macos_product_version() -> Option<String> {
    let out = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[tauri::command]
fn get_apple_intelligence_eligibility() -> AppleIntelligenceEligibility {
    let arch = std::env::consts::ARCH;

    #[cfg(not(target_os = "macos"))]
    {
        return AppleIntelligenceEligibility {
            eligible: false,
            reason: "Not supported on this platform.".to_string(),
            details: Some(format!("{} ({arch})", std::env::consts::OS)),
        };
    }

    #[cfg(target_os = "macos")]
    {
        let version = macos_product_version();
        let platform_details = version
            .as_ref()
            .map(|v| format!("macos {v} ({arch})"))
            .or_else(|| Some(format!("macos (unknown version) ({arch})")));

        if arch != "aarch64" {
            return AppleIntelligenceEligibility {
                eligible: false,
                reason: "Requires Apple Silicon (M1+).".to_string(),
                details: platform_details,
            };
        }

        let major = version
            .as_ref()
            .and_then(|v| v.split('.').next())
            .and_then(|m| m.parse::<u32>().ok());
        if major.is_none() {
            return AppleIntelligenceEligibility {
                eligible: false,
                reason: "Could not detect macOS version.".to_string(),
                details: platform_details,
            };
        }

        if major.unwrap_or(0) < 26 {
            return AppleIntelligenceEligibility {
                eligible: false,
                reason: "Requires macOS 26+.".to_string(),
                details: platform_details,
            };
        }

        let availability =
            match crate::apple_intelligence::availability(Some(crate::i18n::system_locale())) {
                Ok(a) => a,
                Err(err) => {
                    return AppleIntelligenceEligibility {
                        eligible: false,
                        reason: "Could not check Apple Intelligence availability.".to_string(),
                        details: Some(format!(
                            "{}; {}",
                            platform_details.unwrap_or_else(|| format!("macos (unknown) ({arch})")),
                            err
                        )),
                    };
                }
            };

        if availability.available {
            return AppleIntelligenceEligibility {
                eligible: true,
                reason: "Available.".to_string(),
                details: platform_details,
            };
        }

        let reason = match availability.reason.as_deref() {
            Some("appleIntelligenceNotEnabled") => {
                "Apple Intelligence is disabled in System Settings.".to_string()
            }
            Some("deviceNotEligible") => {
                "This device is not eligible for Apple Intelligence.".to_string()
            }
            Some("modelNotReady") => {
                "Apple Intelligence model is not ready yet (downloading/initializing).".to_string()
            }
            _ => availability
                .details
                .clone()
                .unwrap_or_else(|| "Apple Intelligence unavailable.".to_string()),
        };

        let mut details = platform_details;
        if let Some(extra) = availability.details.as_deref() {
            if let Some(ref mut d) = details {
                d.push_str("; ");
                d.push_str(extra);
            } else {
                details = Some(extra.to_string());
            }
        }

        AppleIntelligenceEligibility {
            eligible: false,
            reason,
            details,
        }
    }
}

#[tauri::command]
async fn check_permissions() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    let screen_recording = check_screen_recording();
    #[cfg(not(target_os = "macos"))]
    let screen_recording = false;
    #[cfg(target_os = "macos")]
    let accessibility = ax_is_process_trusted();
    #[cfg(not(target_os = "macos"))]
    let accessibility = false;
    if cfg!(debug_assertions) {
        eprintln!(
            "check_permissions: screen_recording={screen_recording} accessibility={accessibility}"
        );
    }
    PermissionStatus {
        screen_recording,
        accessibility,
    }
}

/// Check screen recording permission.
///
/// We use the well-known window-name heuristic: without screen recording
/// permission macOS strips `kCGWindowName` from the window-info
/// dictionaries of other processes.  If we can read a window name from any
/// foreign, non-Dock process we know the permission has been granted.
///
/// NOTE: We intentionally do NOT call `CGPreflightScreenCaptureAccess` or
/// `CGRequestScreenCaptureAccess` here.  On macOS 26 (Tahoe) these
/// CoreGraphics APIs silently create a TCC deny record without ever
/// showing a prompt, which prevents ScreenCaptureKit from prompting later.
/// The window-name heuristic is side-effect-free and reflects the live
/// TCC state — no restart required.
#[cfg(target_os = "macos")]
fn check_screen_recording() -> bool {
    check_screen_recording_via_window_names()
}

/// Enumerate on-screen windows and return `true` if any window belonging to
/// another process exposes its `kCGWindowName`.
#[cfg(target_os = "macos")]
fn check_screen_recording_via_window_names() -> bool {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;

    let our_pid = std::process::id() as i32;

    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };

    if window_list.is_null() {
        return false;
    }

    let count = unsafe { core_foundation::array::CFArrayGetCount(window_list as _) };

    for i in 0..count {
        let window_dict = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef
        };
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                window_dict,
            )
        };

        // Skip our own windows.
        let pid_key = CFString::new("kCGWindowOwnerPID");
        let owner_pid = dict
            .find(pid_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32()
            })
            .unwrap_or(0);

        if owner_pid == our_pid {
            continue;
        }

        // Skip the Dock — it always exposes window names.
        let owner_name_key = CFString::new("kCGWindowOwnerName");
        let owner_name = dict.find(owner_name_key).map(|v| {
            let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
            s.to_string()
        });

        if let Some(ref name) = owner_name {
            if name == "Dock" || name == "Window Server" {
                continue;
            }
        }

        // The key test: can we see the window name?
        let name_key = CFString::new("kCGWindowName");
        if dict.find(name_key).is_some() {
            if cfg!(debug_assertions) {
                eprintln!(
                    "check_screen_recording_via_window_names: found window name for pid={owner_pid} ({})",
                    owner_name.as_deref().unwrap_or("?")
                );
            }
            return true;
        }
    }

    false
}

/// Return the first on-screen window ID belonging to a foreign process.
///
/// We use this to attempt a "real" cross-process capture when probing.
#[cfg(target_os = "macos")]
fn first_foreign_window_id() -> Option<u32> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;

    let our_pid = std::process::id() as i32;

    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };

    if window_list.is_null() {
        return None;
    }

    let count = unsafe { core_foundation::array::CFArrayGetCount(window_list as _) };
    for i in 0..count {
        let window_dict = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef
        };
        let dict = unsafe {
            core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                window_dict,
            )
        };

        let pid_key = CFString::new("kCGWindowOwnerPID");
        let owner_pid = dict
            .find(pid_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32()
            })
            .unwrap_or(0);
        if owner_pid == 0 || owner_pid == our_pid {
            continue;
        }

        let owner_name_key = CFString::new("kCGWindowOwnerName");
        let owner_name = dict.find(owner_name_key).map(|v| {
            let s: CFString = unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as _) };
            s.to_string()
        });
        if matches!(owner_name.as_deref(), Some("Dock") | Some("Window Server")) {
            continue;
        }

        let id_key = CFString::new("kCGWindowNumber");
        let window_id = dict
            .find(id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
                num.to_i32()
            })
            .unwrap_or(0);
        if window_id > 0 {
            return Some(window_id as u32);
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn ax_is_process_trusted() -> bool {
    unsafe { accessibility_sys::AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
fn ax_is_process_trusted_with_prompt() -> bool {
    unsafe {
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::string::CFString;

        let key = CFString::wrap_under_get_rule(accessibility_sys::kAXTrustedCheckOptionPrompt);
        let dict = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
        accessibility_sys::AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef())
    }
}

#[tauri::command]
async fn request_screen_recording(app: tauri::AppHandle) -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = app.run_on_main_thread(move || {
            permission_debug_log(
                "request_screen_recording(main): calling CGRequestScreenCaptureAccess",
            );

            #[link(name = "CoreGraphics", kind = "framework")]
            extern "C" {
                fn CGRequestScreenCaptureAccess() -> bool;
            }
            let result = unsafe { CGRequestScreenCaptureAccess() };
            permission_debug_log(&format!(
                "request_screen_recording(main): CGRequestScreenCaptureAccess -> {result}"
            ));
            let _ = tx.send(());
        });
        let _ = rx.recv();

        // Only open System Settings if still not granted (avoid double dialog)
        if !check_screen_recording() {
            if let Err(err) =
                tauri_plugin_opener::open_url(SCREEN_RECORDING_SETTINGS_URL, None::<&str>)
            {
                eprintln!("Failed to open Screen Recording settings: {err}");
            }
        }
    }

    check_permissions().await
}

#[tauri::command]
async fn request_accessibility(app: tauri::AppHandle) -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        // AXIsProcessTrustedWithOptions(prompt: true) shows its own native
        // dialog with an "Open System Preferences" button — no need to also
        // open System Settings ourselves.
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = app.run_on_main_thread(move || {
            permission_debug_log(
                "request_accessibility(main): calling AXIsProcessTrustedWithOptions",
            );
            let result = ax_is_process_trusted_with_prompt();
            permission_debug_log(&format!(
                "request_accessibility(main): AXIsProcessTrustedWithOptions -> {result}"
            ));
            let _ = tx.send(());
        });
        let _ = rx.recv();
    }

    check_permissions().await
}

/// Background loop that processes clicks and emits step-captured events.
fn process_clicks_loop(app: tauri::AppHandle, processing_running: Arc<AtomicBool>) {
    loop {
        // Check if we should stop
        if !processing_running.load(Ordering::SeqCst) {
            break;
        }

        // Get the app state
        let state = app.state::<RecorderAppState>();

        // Check recorder state - don't process if paused or stopped
        let should_process = {
            let recorder = state.recorder_state.lock().ok();
            recorder
                .map(|r| r.current_state() == SessionState::Recording)
                .unwrap_or(false)
        };

        if !should_process {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }

        // Get click from listener
        let click = {
            let listener_lock = state.click_listener.lock().ok();
            listener_lock
                .as_ref()
                .and_then(|opt| opt.as_ref())
                .and_then(|listener| listener.recv_timeout(std::time::Duration::from_millis(50)))
        };

        if let Some(click) = click {
            let mut recorded_step: Option<Step> = None;
            let mut auth_step: Option<Step> = None;

            {
                let mut session_lock = state.session.lock().ok();
                if let Some(ref mut session) = session_lock.as_mut().and_then(|s| s.as_mut()) {
                    let (prompt_step, suppress_click) =
                        pipeline::handle_auth_prompt(&click, session, &state.pipeline_state);
                    auth_step = prompt_step;

                    if !suppress_click {
                        let pre_click_buffer = state
                            .pre_click_buffer
                            .lock()
                            .ok()
                            .and_then(|g| g.as_ref().cloned());
                        if let Ok(step) = pipeline::process_click(
                            &click,
                            session,
                            &state.pipeline_state,
                            pre_click_buffer.as_ref(),
                        ) {
                            recorded_step = Some(step);
                        }
                    }
                }
            }

            if let Some(step) = recorded_step {
                let _ = app.emit("step-captured", &step);
            }
            if let Some(step) = auth_step {
                let _ = app.emit("step-captured", &step);
            }
        }
    }
}

/// Perform a tiny screen capture to trigger the macOS 26 runtime confirmation
/// dialog ("StepCast möchte … direkt auf deinen Bildschirm und Ton zugreifen").
/// On Tahoe, the System Settings entry alone is not enough — the first real
/// capture triggers an additional one-time prompt.  By doing it here, the
/// dialog appears when the user clicks "Start Recording" instead of silently
/// during their first workflow click.
#[cfg(target_os = "macos")]
fn probe_screen_capture() {
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        create_image, kCGNullWindowID, kCGWindowImageBestResolution,
        kCGWindowImageBoundsIgnoreFraming, kCGWindowListExcludeDesktopElements,
        kCGWindowListOptionOnScreenOnly,
    };

    // kCGWindowListOptionIncludingWindow = 1 << 3 = 8
    const K_CG_WINDOW_LIST_OPTION_INCLUDING_WINDOW: u32 = 1 << 3;

    // Prefer capturing a foreign window; this is the most reliable way to
    // trigger a Screen Recording (kTCCServiceScreenCapture) record for the app.
    if let Some(window_id) = first_foreign_window_id() {
        permission_debug_log(&format!(
            "probe_screen_capture: foreign window_id={window_id}"
        ));

        // CGRectNull tells CGWindowListCreateImage to use the window's own bounds.
        let null_rect = CGRect::new(
            &CGPoint::new(f64::INFINITY, f64::INFINITY),
            &CGSize::new(0.0, 0.0),
        );

        let img = create_image(
            null_rect,
            K_CG_WINDOW_LIST_OPTION_INCLUDING_WINDOW,
            window_id,
            kCGWindowImageBestResolution | kCGWindowImageBoundsIgnoreFraming,
        );
        permission_debug_log(&format!(
            "probe_screen_capture: CGWindowListCreateImage(including_window) -> {}",
            if img.is_some() { "some" } else { "none" }
        ));
        return;
    }

    // Fallback: 1x1 point probe at origin.
    permission_debug_log("probe_screen_capture: no foreign window found; using 1x1 fallback");
    let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
    let img = create_image(
        rect,
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID,
        kCGWindowImageBestResolution,
    );
    permission_debug_log(&format!(
        "probe_screen_capture: CGWindowListCreateImage(fallback) -> {}",
        if img.is_some() { "some" } else { "none" }
    ));
}

#[tauri::command]
async fn start_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
) -> Result<(), String> {
    let permissions = check_permissions().await;
    if !permissions.screen_recording || !permissions.accessibility {
        return Err("missing screen recording or accessibility permission".to_string());
    }

    // Trigger the macOS 26 runtime capture confirmation (one-time dialog).
    #[cfg(target_os = "macos")]
    probe_screen_capture();

    // Reset pipeline state for the new session
    {
        let mut ps = state
            .pipeline_state
            .lock()
            .map_err(|_| "pipeline state lock poisoned")?;
        ps.reset();
    }

    // Clean up previous session if any
    {
        let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        if let Some(old_session) = session_lock.as_ref() {
            // In dev, keep old session dirs so we can audit screenshots/logs/AI output.
            if !cfg!(debug_assertions) {
                old_session.cleanup();
            }
        }
    }

    // Create new session
    let session = Session::new().map_err(|e| format!("Failed to create session: {e}"))?;

    // Start click listener
    let click_listener =
        ClickListener::start().map_err(|e| format!("Failed to start click listener: {e}"))?;

    // Store session and click listener in state
    {
        let mut session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        *session_lock = Some(session);
    }
    {
        let mut listener_lock = state
            .click_listener
            .lock()
            .map_err(|_| "click listener lock poisoned")?;
        *listener_lock = Some(click_listener);
    }
    {
        let mut pre_click_lock = state
            .pre_click_buffer
            .lock()
            .map_err(|_| "pre-click buffer lock poisoned")?;
        *pre_click_lock = recorder::pre_click_buffer::PreClickFrameBuffer::start().ok();
    }

    // Set processing flag to running
    state.processing_running.store(true, Ordering::SeqCst);

    // Start background task to process clicks
    let processing_running = Arc::clone(&state.processing_running);
    let app_handle = app.clone();
    std::thread::spawn(move || {
        process_clicks_loop(app_handle, processing_running);
    });

    // Update recorder state
    let mut recorder_state = state
        .recorder_state
        .lock()
        .map_err(|_| "recorder state lock poisoned".to_string())?;
    recorder_state
        .start()
        .map_err(|error| format!("{error:?}"))?;

    // Hide panel and set recording icon on main thread (required for macOS UI operations)
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        if cfg!(debug_assertions) {
            eprintln!("Hiding window for recording (main thread)...");
        }

        // Hide the window
        if let Some(window) = app_clone.get_webview_window(panel::panel_label()) {
            let _ = window.hide();
        }
        {
            let ps_state = app_clone.state::<RecorderAppState>();
            recorder::pipeline::set_panel_visible(&ps_state.pipeline_state, false);
        }

        // Set recording icon
        if let Err(e) = tray::set_recording_icon(&app_clone) {
            eprintln!("Failed to set recording icon: {e}");
        }

        if cfg!(debug_assertions) {
            eprintln!("Recording UI updated successfully");
        }
    });

    Ok(())
}

#[tauri::command]
fn pause_recording(state: tauri::State<'_, RecorderAppState>) -> Result<(), String> {
    let mut recorder_state = state
        .recorder_state
        .lock()
        .map_err(|_| "recorder state lock poisoned".to_string())?;
    recorder_state.pause().map_err(|error| format!("{error:?}"))
}

#[tauri::command]
async fn resume_recording(state: tauri::State<'_, RecorderAppState>) -> Result<(), String> {
    let permissions = check_permissions().await;
    if !permissions.screen_recording || !permissions.accessibility {
        return Err("missing screen recording or accessibility permission".to_string());
    }

    let mut recorder_state = state
        .recorder_state
        .lock()
        .map_err(|_| "recorder state lock poisoned".to_string())?;
    recorder_state
        .resume()
        .map_err(|error| format!("{error:?}"))
}

#[tauri::command]
fn stop_recording(
    _app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
) -> Result<Vec<Step>, String> {
    // Stop the processing loop
    state.processing_running.store(false, Ordering::SeqCst);

    // Stop click listener
    {
        let mut listener_lock = state
            .click_listener
            .lock()
            .map_err(|_| "click listener lock poisoned")?;
        if let Some(listener) = listener_lock.take() {
            listener.stop();
        }
    }
    {
        let mut pre_click_lock = state
            .pre_click_buffer
            .lock()
            .map_err(|_| "pre-click buffer lock poisoned")?;
        if let Some(buffer) = pre_click_lock.take() {
            buffer.stop();
        }
    }
    {
        let mut pre_click_lock = state
            .pre_click_buffer
            .lock()
            .map_err(|_| "pre-click buffer lock poisoned")?;
        if let Some(buffer) = pre_click_lock.take() {
            buffer.stop();
        }
    }

    // Write diagnostics and get steps from session
    let steps = {
        let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        if let Some(s) = session_lock.as_ref() {
            s.write_diagnostics();
        }
        session_lock
            .as_ref()
            .map(|s| s.get_steps().to_vec())
            .unwrap_or_default()
    };

    // Update recorder state
    let mut recorder_state = state
        .recorder_state
        .lock()
        .map_err(|_| "recorder state lock poisoned".to_string())?;
    recorder_state
        .stop()
        .map_err(|error| format!("{error:?}"))?;

    // Show panel and reset icon on main thread
    let app_clone = _app.clone();
    let _ = _app.run_on_main_thread(move || {
        if cfg!(debug_assertions) {
            eprintln!("Showing window after recording stopped (main thread)...");
        }

        if let Some(window) = app_clone.get_webview_window(panel::panel_label()) {
            let _ = window.show();
            if let Err(err) = tray::position_panel_at_current_tray_icon(&app_clone) {
                eprintln!("Failed to position panel: {err}");
            }
            let ps_state = app_clone.state::<RecorderAppState>();
            if let Ok(bounds) = panel::panel_bounds(&app_clone) {
                if cfg!(debug_assertions) {
                    eprintln!(
                        "Panel bounds after auto-show: x={} y={} w={} h={}",
                        bounds.x, bounds.y, bounds.width, bounds.height
                    );
                }
                recorder::pipeline::record_panel_bounds(&ps_state.pipeline_state, bounds);
            }
            recorder::pipeline::set_panel_visible(&ps_state.pipeline_state, true);
        }

        if let Err(e) = tray::set_default_icon(&app_clone) {
            eprintln!("Failed to reset tray icon: {e}");
        }
    });

    Ok(steps)
}

#[tauri::command]
fn get_steps(state: tauri::State<'_, RecorderAppState>) -> Result<Vec<Step>, String> {
    let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
    let steps = session_lock
        .as_ref()
        .map(|s| s.get_steps().to_vec())
        .unwrap_or_default();
    Ok(steps)
}

#[tauri::command]
fn discard_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
) -> Result<(), String> {
    // Stop the processing loop first
    state.processing_running.store(false, Ordering::SeqCst);

    // Small delay to let processing loop exit
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Stop and remove click listener
    {
        let mut listener_lock = state
            .click_listener
            .lock()
            .map_err(|_| "click listener lock poisoned")?;
        if let Some(listener) = listener_lock.take() {
            listener.stop();
        }
    }

    // Write diagnostics, then clean up session temp dir and clear session
    {
        let mut session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        if let Some(session) = session_lock.as_ref() {
            session.write_diagnostics();
            session.cleanup();
        }
        *session_lock = None;
    }

    // Reset recorder state to idle
    {
        let mut recorder_state = state
            .recorder_state
            .lock()
            .map_err(|_| "recorder state lock poisoned")?;
        // Force reset to idle state
        *recorder_state = RecorderState::new();
    }

    // Reset pipeline state
    {
        let mut ps = state
            .pipeline_state
            .lock()
            .map_err(|_| "pipeline state lock poisoned")?;
        ps.reset();
    }

    // Notify editor window (if open) that steps were discarded
    let _ = app.emit("steps-discarded", ());

    // Show panel and reset icon on main thread after discard
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_clone.get_webview_window(panel::panel_label()) {
            let _ = window.show();
            if let Err(err) = tray::position_panel_at_current_tray_icon(&app_clone) {
                eprintln!("Failed to position panel: {err}");
            }
            let ps_state = app_clone.state::<RecorderAppState>();
            if let Ok(bounds) = panel::panel_bounds(&app_clone) {
                if cfg!(debug_assertions) {
                    eprintln!(
                        "Panel bounds after discard: x={} y={} w={} h={}",
                        bounds.x, bounds.y, bounds.width, bounds.height
                    );
                }
                recorder::pipeline::record_panel_bounds(&ps_state.pipeline_state, bounds);
            }
            recorder::pipeline::set_panel_visible(&ps_state.pipeline_state, true);
        }

        if let Err(e) = tray::set_default_icon(&app_clone) {
            eprintln!("Failed to reset tray icon: {e}");
        }
    });

    Ok(())
}

#[tauri::command]
fn update_step_note(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
    step_id: String,
    note: Option<String>,
) -> Result<(), String> {
    let mut session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
    let session = session_lock.as_mut().ok_or("no active session")?;
    let updated = session
        .update_step_note(&step_id, note)
        .ok_or("step not found")?
        .clone();
    let _ = app.emit("step-updated", &updated);
    Ok(())
}

#[tauri::command]
fn update_step_description(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
    step_id: String,
    description: Option<String>,
) -> Result<(), String> {
    let mut session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
    let session = session_lock.as_mut().ok_or("no active session")?;
    let updated = session
        .set_step_description_manual(&step_id, description)
        .ok_or("step not found")?
        .clone();
    let _ = app.emit("step-updated", &updated);
    Ok(())
}

fn normalize_crop_region_input(crop_region: Option<BoundsPercent>) -> Option<BoundsPercent> {
    let input = crop_region?;
    let values = [
        input.x_percent,
        input.y_percent,
        input.width_percent,
        input.height_percent,
    ];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }

    let x = input.x_percent.clamp(0.0, 100.0);
    let y = input.y_percent.clamp(0.0, 100.0);
    let mut w = input.width_percent.clamp(0.0, 100.0);
    let mut h = input.height_percent.clamp(0.0, 100.0);
    if x + w > 100.0 {
        w = (100.0 - x).max(0.0);
    }
    if y + h > 100.0 {
        h = (100.0 - y).max(0.0);
    }

    const MIN_SIZE_PERCENT: f32 = 2.0;
    if w < MIN_SIZE_PERCENT || h < MIN_SIZE_PERCENT {
        return None;
    }

    Some(BoundsPercent {
        x_percent: x,
        y_percent: y,
        width_percent: w,
        height_percent: h,
    })
}

#[tauri::command]
fn update_step_crop(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
    step_id: String,
    crop_region: Option<BoundsPercent>,
) -> Result<(), String> {
    let mut session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
    let session = session_lock.as_mut().ok_or("no active session")?;
    let updated = session
        .update_step_crop(&step_id, normalize_crop_region_input(crop_region))
        .ok_or("step not found")?
        .clone();
    let _ = app.emit("step-updated", &updated);
    Ok(())
}

#[tauri::command]
fn generate_step_descriptions(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
    mode: Option<String>,
    step_ids: Option<Vec<String>>,
    app_language: Option<String>,
) -> Result<(), String> {
    // Serialize description generation to avoid racing step updates.
    if state.ai_descriptions_running.swap(true, Ordering::SeqCst) {
        return Err("AI description generation already running.".into());
    }

    struct ResetOnDrop(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for ResetOnDrop {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let running_guard = ResetOnDrop(state.ai_descriptions_running.clone());

    #[derive(Debug, Clone, Copy)]
    enum Mode {
        MissingOnly,
        All,
        Ids,
    }

    let parsed_mode = match (mode.as_deref(), step_ids.as_ref()) {
        (_, Some(ids)) if !ids.is_empty() => Mode::Ids,
        (Some("all"), _) => Mode::All,
        _ => Mode::MissingOnly,
    };

    // Slightly longer than a one-liner, still "no novels" — enables useful context like "from the Dock".
    let max_chars = 110usize;
    let locale = i18n::resolve_locale(i18n::parse_app_language(app_language.as_deref()));
    let mut ids_to_generate: Vec<String> = Vec::new();
    let (steps_to_generate, session_dir): (Vec<Step>, std::path::PathBuf) = {
        let mut session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        let Some(session) = session_lock.as_mut() else {
            return Err("no active session".into());
        };

        let session_dir = session.temp_dir.clone();
        let mut steps: Vec<Step> = Vec::new();
        let id_set: std::collections::HashSet<String> =
            step_ids.clone().unwrap_or_default().into_iter().collect();

        for step in session.steps.iter_mut() {
            if crate::apple_intelligence::is_auth_placeholder(step) {
                continue;
            }
            if step.action == ActionType::Note {
                continue;
            }

            let should_generate = match parsed_mode {
                Mode::Ids => id_set.contains(&step.id),
                Mode::All => !matches!(step.description_source, Some(DescriptionSource::Manual)),
                Mode::MissingOnly => {
                    crate::apple_intelligence::is_blank_description(step.description.as_deref())
                        && !matches!(step.description_source, Some(DescriptionSource::Manual))
                }
            };

            if !should_generate {
                continue;
            }

            step.description_status = Some(DescriptionStatus::Generating);
            step.description_error = None;

            let updated = step.clone();
            ids_to_generate.push(step.id.clone());
            steps.push(updated.clone());
            let _ = app.emit("step-updated", &updated);
        }

        (steps, session_dir)
    };

    if steps_to_generate.is_empty() {
        return Ok(());
    }

    #[cfg(debug_assertions)]
    let trace_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    #[cfg(debug_assertions)]
    {
        session_debug_log(
            &session_dir,
            &format!(
                "ai_generate_start trace={} mode={:?} count={} max_chars={}",
                trace_ts,
                mode.as_deref().unwrap_or("missing_only"),
                steps_to_generate.len(),
                max_chars
            ),
        );
        let req_json = serde_json::json!({
            "trace": trace_ts,
            "mode": mode.as_deref().unwrap_or("missing_only"),
            "max_chars": max_chars,
            "step_ids": ids_to_generate,
            "steps": steps_to_generate,
        });
        write_session_json(
            &session_dir,
            &format!("ai-trace-{trace_ts}-request.json"),
            &req_json,
        );
    }

    let running = state.ai_descriptions_running.clone();
    let app_handle = app.clone();
    let session_dir_for_logs = session_dir.clone();

    tauri::async_runtime::spawn(async move {
        let generate_steps = steps_to_generate;

        let resp = tauri::async_runtime::spawn_blocking(move || {
            crate::apple_intelligence::generate_descriptions(generate_steps, max_chars, locale)
        })
        .await;

        let apply_error_to_all = |app_handle: &tauri::AppHandle, ids: &[String], err: String| {
            let state = app_handle.state::<RecorderAppState>();
            let mut session_lock = match state.session.lock() {
                Ok(l) => l,
                Err(e) => e.into_inner(),
            };
            let Some(session) = session_lock.as_mut() else {
                return;
            };
            for id in ids {
                if let Some(step) = session.mark_step_description_failed(id, err.clone()) {
                    let _ = app_handle.emit("step-updated", step);
                }
            }
        };

        match resp {
            Ok(Ok(gen)) => {
                #[cfg(debug_assertions)]
                {
                    let resp_json = serde_json::json!({
                        "trace": trace_ts,
                        "results": gen.results,
                        "failures": gen.failures,
                    });
                    write_session_json(
                        &session_dir_for_logs,
                        &format!("ai-trace-{trace_ts}-response.json"),
                        &resp_json,
                    );
                }

                let state = app_handle.state::<RecorderAppState>();
                let mut session_lock = match state.session.lock() {
                    Ok(l) => l,
                    Err(e) => e.into_inner(),
                };
                let Some(session) = session_lock.as_mut() else {
                    running.store(false, Ordering::SeqCst);
                    return;
                };

                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

                for r in gen.results {
                    seen.insert(r.id.clone());
                    #[cfg(debug_assertions)]
                    {
                        session_debug_log(
                            &session_dir_for_logs,
                            &format!(
                                "ai_desc trace={} id={} text={}",
                                trace_ts,
                                r.id,
                                json_escape_one_line(&r.text)
                            ),
                        );
                        if let Some(debug) = &r.debug {
                            if let Ok(debug_json) = serde_json::to_string(debug) {
                                session_debug_log(
                                    &session_dir_for_logs,
                                    &format!(
                                        "ai_desc_debug trace={} id={} data={}",
                                        trace_ts,
                                        r.id,
                                        json_escape_one_line(&debug_json)
                                    ),
                                );
                            }
                        }
                    }
                    if let Some(step) = session.apply_step_description_ai(&r.id, r.text) {
                        let _ = app_handle.emit("step-updated", step);
                    }
                }
                for f in gen.failures {
                    seen.insert(f.id.clone());
                    if f.id == "*" {
                        continue;
                    }
                    #[cfg(debug_assertions)]
                    {
                        session_debug_log(
                            &session_dir_for_logs,
                            &format!(
                                "ai_desc_failed trace={} id={} error={}",
                                trace_ts,
                                f.id,
                                json_escape_one_line(&f.error)
                            ),
                        );
                    }
                    if let Some(step) = session.mark_step_description_failed(&f.id, f.error) {
                        let _ = app_handle.emit("step-updated", step);
                    }
                }

                // Any step that was marked generating but has no result should be failed.
                for id in &ids_to_generate {
                    if seen.contains(id) {
                        continue;
                    }
                    #[cfg(debug_assertions)]
                    {
                        session_debug_log(
                            &session_dir_for_logs,
                            &format!(
                                "ai_desc_failed trace={} id={} error={}",
                                trace_ts, id, "No model output."
                            ),
                        );
                    }
                    if let Some(step) =
                        session.mark_step_description_failed(id, "No model output.".into())
                    {
                        let _ = app_handle.emit("step-updated", step);
                    }
                }

                #[cfg(debug_assertions)]
                session_debug_log(
                    &session_dir_for_logs,
                    &format!("ai_generate_done trace={trace_ts}"),
                );
            }
            Ok(Err(err)) => {
                #[cfg(debug_assertions)]
                session_debug_log(
                    &session_dir_for_logs,
                    &format!(
                        "ai_generate_failed trace={} error={}",
                        trace_ts,
                        json_escape_one_line(&err)
                    ),
                );
                apply_error_to_all(&app_handle, &ids_to_generate, err)
            }
            Err(err) => {
                #[cfg(debug_assertions)]
                session_debug_log(
                    &session_dir_for_logs,
                    &format!(
                        "ai_generate_failed trace={} error={}",
                        trace_ts,
                        json_escape_one_line(&err.to_string())
                    ),
                );
                apply_error_to_all(
                    &app_handle,
                    &ids_to_generate,
                    format!("AI generation task failed: {err}"),
                )
            }
        }

        running.store(false, Ordering::SeqCst);
    });

    // Background task owns resetting the running flag.
    std::mem::forget(running_guard);
    Ok(())
}

#[tauri::command]
fn delete_step(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
    step_id: String,
) -> Result<(), String> {
    let mut session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
    let session = session_lock.as_mut().ok_or("no active session")?;
    if !session.delete_step(&step_id) {
        return Err("step not found".into());
    }
    let _ = app.emit("step-deleted", &step_id);
    Ok(())
}

#[tauri::command]
fn reorder_steps(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
    step_ids: Vec<String>,
) -> Result<Vec<Step>, String> {
    let mut session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
    let session = session_lock.as_mut().ok_or("no active session")?;
    session.reorder_steps(&step_ids);
    let steps = session.get_steps().to_vec();
    let _ = app.emit("steps-reordered", &steps);
    Ok(steps)
}

#[tauri::command]
fn open_editor_window(app: tauri::AppHandle) -> Result<(), String> {
    // Hide the tray panel so it doesn't overlap the editor
    if let Some(panel_window) = app.get_webview_window(panel::panel_label()) {
        let _ = panel_window.hide();
    }

    // If editor already exists, focus it
    if let Some(window) = app.get_webview_window("step-editor") {
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, "step-editor", WebviewUrl::App("/editor.html".into()))
        .title("Step Editor")
        .inner_size(900.0, 700.0)
        .resizable(true)
        .decorations(true)
        .build()
        .map_err(|e| format!("Failed to create editor window: {e}"))?;

    Ok(())
}

#[tauri::command]
async fn export_guide(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
    title: String,
    format: String,
    output_path: String,
    app_language: Option<String>,
) -> Result<(), String> {
    let fmt = export::ExportFormat::from_str(&format)?;
    let locale = i18n::resolve_locale(i18n::parse_app_language(app_language.as_deref()));
    let steps = {
        let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        session_lock
            .as_ref()
            .map(|s| s.get_steps().to_vec())
            .unwrap_or_default()
    };
    export::export(&title, &steps, fmt, &output_path, &app, locale)
}

#[tauri::command]
fn get_startup_state() -> startup_state::StartupState {
    startup_state::load()
}

#[tauri::command]
fn mark_startup_seen(app: tauri::AppHandle) -> Result<(), String> {
    let mut state = startup_state::load();
    state.has_launched_before = true;
    state.last_seen_version = Some(env!("CARGO_PKG_VERSION").to_string());
    startup_state::save(&state)?;

    // Switch from Regular (Dock visible) to Accessory (menu bar only)
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    Ok(())
}

#[tauri::command]
fn dismiss_whats_new() -> Result<(), String> {
    let mut state = startup_state::load();
    state.last_seen_version = Some(env!("CARGO_PKG_VERSION").to_string());
    startup_state::save(&state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _recorder = recorder::Recorder::new();

    // Clean up leftover session directories from previous runs
    // In dev, keep session dirs so we can audit recorder + AI behavior.
    if !cfg!(debug_assertions) {
        Session::cleanup_all_sessions();
    }

    // Tokio runtime required by tauri-plugin-aptabase
    let _rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let _guard = _rt.enter();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_aptabase::Builder::new("A-EU-6084625392").build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let startup = startup_state::load();

            #[cfg(target_os = "macos")]
            {
                if startup.has_launched_before {
                    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                } else {
                    // First run: keep Regular so Dock icon is visible as a hint
                    app.set_activation_policy(tauri::ActivationPolicy::Regular);
                }
            }

            panel::init(app.handle())?;
            tray::create(app.handle())?;

            // Resolve Apple Intelligence helper path early. This is required for release builds
            // because we execute the Swift helper directly from the app bundle resources
            // (codesigned/notarized), not from a cache-extracted copy.
            #[cfg(target_os = "macos")]
            {
                if let Err(err) = crate::apple_intelligence::init(app.handle()) {
                    if cfg!(debug_assertions) {
                        eprintln!("Apple Intelligence helper init failed: {err}");
                    }
                }
            }

            // Register global shortcut Cmd+Shift+S to toggle panel
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                let shortcut = Shortcut::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyS);
                if let Err(err) =
                    app.global_shortcut()
                        .on_shortcut(shortcut, |app, _shortcut, event| {
                            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                                tray::toggle_panel(app);
                            }
                        })
                {
                    eprintln!("Failed to register global shortcut: {err}");
                }
            }

            #[cfg(not(debug_assertions))]
            let _ = app.track_event("app_started", None);

            Ok(())
        })
        .manage(RecorderAppState {
            recorder_state: Mutex::new(RecorderState::new()),
            session: Mutex::new(None),
            click_listener: Mutex::new(None),
            pre_click_buffer: Mutex::new(None),
            processing_running: Arc::new(AtomicBool::new(false)),
            pipeline_state: Mutex::new(pipeline::PipelineState::new()),
            ai_descriptions_running: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            check_permissions,
            get_apple_intelligence_eligibility,
            request_screen_recording,
            request_accessibility,
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording,
            get_steps,
            update_step_note,
            update_step_description,
            update_step_crop,
            delete_step,
            reorder_steps,
            open_editor_window,
            export_guide,
            discard_recording,
            generate_step_descriptions,
            get_startup_state,
            mark_startup_seen,
            dismiss_whats_new,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                // Dock icon clicked — show the panel
                tray::show_panel(app_handle);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::PermissionStatus;

    #[test]
    fn permission_status_defaults_false() {
        let status = PermissionStatus::default();
        assert!(!status.screen_recording);
        assert!(!status.accessibility);
    }
}
