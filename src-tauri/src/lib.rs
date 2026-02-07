// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod panel;
mod recorder;
mod tray;
mod export;
use recorder::click_listener::ClickListener;
use recorder::pipeline;
use recorder::session::Session;
use recorder::state::{RecorderState, SessionState};
use recorder::types::Step;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri_nspanel::ManagerExt;
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
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{ts}] {message}");
    }
}


struct RecorderAppState {
    recorder_state: Mutex<RecorderState>,
    session: Mutex<Option<Session>>,
    click_listener: Mutex<Option<ClickListener>>,
    processing_running: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
struct PermissionStatus {
    screen_recording: bool,
    accessibility: bool,
}

const SCREEN_RECORDING_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
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
        eprintln!("check_permissions: screen_recording={screen_recording} accessibility={accessibility}");
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
            core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i)
                as CFDictionaryRef
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
                let num: CFNumber =
                    unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
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
            core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i)
                as CFDictionaryRef
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
                let num: CFNumber =
                    unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
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
                let num: CFNumber =
                    unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as _) };
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
            permission_debug_log("request_screen_recording(main): calling CGRequestScreenCaptureAccess");

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

        if let Err(err) = tauri_plugin_opener::open_url(SCREEN_RECORDING_SETTINGS_URL, None::<&str>) {
            eprintln!("Failed to open Screen Recording settings: {err}");
        }
    }

    check_permissions().await
}

#[tauri::command]
async fn request_accessibility(app: tauri::AppHandle) -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = app.run_on_main_thread(move || {
            permission_debug_log("request_accessibility(main): calling AXIsProcessTrustedWithOptions");
            let result = ax_is_process_trusted_with_prompt();
            permission_debug_log(&format!(
                "request_accessibility(main): AXIsProcessTrustedWithOptions -> {result}"
            ));
            let _ = tx.send(());
        });
        let _ = rx.recv();

        if let Err(err) = tauri_plugin_opener::open_url(ACCESSIBILITY_SETTINGS_URL, None::<&str>) {
            eprintln!("Failed to open Accessibility settings: {err}");
        }
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
                    let (prompt_step, suppress_click) = pipeline::handle_auth_prompt(&click, session);
                    auth_step = prompt_step;

                    if !suppress_click {
                        if let Ok(step) = pipeline::process_click(&click, session) {
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
        permission_debug_log(&format!("probe_screen_capture: foreign window_id={window_id}"));

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

    // Clean up previous session if any
    {
        let session_lock = state
            .session
            .lock()
            .map_err(|_| "session lock poisoned")?;
        if let Some(old_session) = session_lock.as_ref() {
            old_session.cleanup();
        }
    }

    // Create new session
    let session =
        Session::new().map_err(|e| format!("Failed to create session: {e}"))?;

    // Start click listener
    let click_listener = ClickListener::start()
        .map_err(|e| format!("Failed to start click listener: {e}"))?;

    // Store session and click listener in state
    {
        let mut session_lock = state
            .session
            .lock()
            .map_err(|_| "session lock poisoned")?;
        *session_lock = Some(session);
    }
    {
        let mut listener_lock = state
            .click_listener
            .lock()
            .map_err(|_| "click listener lock poisoned")?;
        *listener_lock = Some(click_listener);
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
        recorder::pipeline::set_panel_visible(false);

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
    recorder_state
        .pause()
        .map_err(|error| format!("{error:?}"))
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

    // Get steps from session
    let steps = {
        let session_lock = state
            .session
            .lock()
            .map_err(|_| "session lock poisoned")?;
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
            if let Ok(bounds) = panel::panel_bounds(&app_clone) {
                if cfg!(debug_assertions) {
                    eprintln!(
                        "Panel bounds after auto-show: x={} y={} w={} h={}",
                        bounds.x, bounds.y, bounds.width, bounds.height
                    );
                }
                recorder::pipeline::record_panel_bounds(bounds);
            }
            recorder::pipeline::set_panel_visible(true);
        }

        if let Err(e) = tray::set_default_icon(&app_clone) {
            eprintln!("Failed to reset tray icon: {e}");
        }
    });

    Ok(steps)
}

#[tauri::command]
fn get_steps(state: tauri::State<'_, RecorderAppState>) -> Result<Vec<Step>, String> {
    let session_lock = state
        .session
        .lock()
        .map_err(|_| "session lock poisoned")?;
    Ok(session_lock
        .as_ref()
        .map(|s| s.get_steps().to_vec())
        .unwrap_or_default())
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

    // Clean up session temp dir and clear session
    {
        let mut session_lock = state
            .session
            .lock()
            .map_err(|_| "session lock poisoned")?;
        if let Some(session) = session_lock.as_ref() {
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

    // Show panel and reset icon on main thread after discard
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_clone.get_webview_window(panel::panel_label()) {
            let _ = window.show();
            if let Err(err) = tray::position_panel_at_current_tray_icon(&app_clone) {
                eprintln!("Failed to position panel: {err}");
            }
            if let Ok(bounds) = panel::panel_bounds(&app_clone) {
                if cfg!(debug_assertions) {
                    eprintln!(
                        "Panel bounds after discard: x={} y={} w={} h={}",
                        bounds.x, bounds.y, bounds.width, bounds.height
                    );
                }
                recorder::pipeline::record_panel_bounds(bounds);
            }
            recorder::pipeline::set_panel_visible(true);
        }

        if let Err(e) = tray::set_default_icon(&app_clone) {
            eprintln!("Failed to reset tray icon: {e}");
        }
    });

    Ok(())
}

#[tauri::command]
async fn export_guide(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecorderAppState>,
    title: String,
    format: String,
    output_path: String,
) -> Result<(), String> {
    let fmt = export::ExportFormat::from_str(&format)?;
    let steps = {
        let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        session_lock.as_ref().map(|s| s.get_steps().to_vec()).unwrap_or_default()
    };
    export::export(&title, &steps, fmt, &output_path, &app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _recorder = recorder::Recorder::new();

    // Clean up leftover session directories from previous runs
    Session::cleanup_all_sessions();

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
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            panel::init(app.handle())?;
            tray::create(app.handle())?;

            // Auto-show panel on launch so new users see it immediately
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                // Brief delay to let the tray icon settle and report its rect
                std::thread::sleep(std::time::Duration::from_millis(300));
                let app_inner = app_handle.clone();
                let _ = app_handle.run_on_main_thread(move || {
                    if let Ok(p) = app_inner.get_webview_panel(panel::panel_label()) {
                        if let Err(err) = tray::position_panel_at_current_tray_icon(&app_inner) {
                            eprintln!("Failed to position panel on launch: {err}");
                        }
                        p.show_and_make_key();
                        if let Ok(bounds) = panel::panel_bounds(&app_inner) {
                            recorder::pipeline::record_panel_bounds(bounds);
                        }
                        recorder::pipeline::set_panel_visible(true);
                    }
                });
            });

            #[cfg(not(debug_assertions))]
            let _ = app.track_event("app_started", None);

            Ok(())
        })
        .manage(RecorderAppState {
            recorder_state: Mutex::new(RecorderState::new()),
            session: Mutex::new(None),
            click_listener: Mutex::new(None),
            processing_running: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            check_permissions,
            request_screen_recording,
            request_accessibility,
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording,
            get_steps,
            export_guide,
            discard_recording,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
