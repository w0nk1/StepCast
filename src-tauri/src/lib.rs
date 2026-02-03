// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod panel;
mod recorder;
mod tray;
use recorder::click_listener::ClickListener;
use recorder::pipeline;
use recorder::session::Session;
use recorder::state::{RecorderState, SessionState};
use recorder::types::Step;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

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

fn missing_permission_urls(status: PermissionStatus) -> Vec<&'static str> {
    let mut urls = Vec::new();
    if !status.screen_recording {
        urls.push(SCREEN_RECORDING_SETTINGS_URL);
    }
    if !status.accessibility {
        urls.push(ACCESSIBILITY_SETTINGS_URL);
    }
    urls
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn check_permissions() -> PermissionStatus {
    let screen_recording = tauri_plugin_macos_permissions::check_screen_recording_permission().await;
    let accessibility = tauri_plugin_macos_permissions::check_accessibility_permission().await;
    PermissionStatus {
        screen_recording,
        accessibility,
    }
}

#[tauri::command]
async fn request_permissions() -> PermissionStatus {
    let current = check_permissions().await;
    if !current.screen_recording {
        tauri_plugin_macos_permissions::request_screen_recording_permission().await;
    }
    if !current.accessibility {
        tauri_plugin_macos_permissions::request_accessibility_permission().await;
    }

    for url in missing_permission_urls(current) {
        if let Err(err) = tauri_plugin_opener::open_url(url, None::<&str>) {
            eprintln!("Failed to open system settings: {err}");
        }
    }

    let screen_recording = tauri_plugin_macos_permissions::check_screen_recording_permission().await;
    let accessibility = tauri_plugin_macos_permissions::check_accessibility_permission().await;
    PermissionStatus {
        screen_recording,
        accessibility,
    }
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
                .and_then(|listener| listener.try_recv())
        };

        if let Some(click) = click {
            // Process click through pipeline
            let step = {
                let mut session_lock = state.session.lock().ok();
                if let Some(ref mut session) = session_lock.as_mut().and_then(|s| s.as_mut()) {
                    pipeline::process_click(&click, session).ok()
                } else {
                    None
                }
            };

            // Emit event to frontend
            if let Some(step) = step {
                let _ = app.emit("step-captured", &step);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
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

    // Create new session
    let session =
        Session::new().map_err(|e| format!("Failed to create session: {}", e))?;

    // Start click listener
    let click_listener = ClickListener::start()
        .map_err(|e| format!("Failed to start click listener: {}", e))?;

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
        .map_err(|error| format!("{error:?}"))
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
fn stop_recording(state: tauri::State<'_, RecorderAppState>) -> Result<Vec<Step>, String> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _recorder = recorder::Recorder::new();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_nspanel::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            panel::init(app.handle())?;
            tray::create(app.handle())?;
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
            request_permissions,
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording,
            get_steps,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        missing_permission_urls, PermissionStatus, ACCESSIBILITY_SETTINGS_URL,
        SCREEN_RECORDING_SETTINGS_URL,
    };

    #[test]
    fn permission_status_defaults_false() {
        let status = PermissionStatus::default();
        assert!(!status.screen_recording);
        assert!(!status.accessibility);
    }

    #[test]
    fn missing_permission_urls_returns_expected_order() {
        let none = missing_permission_urls(PermissionStatus::default());
        assert_eq!(none, vec![SCREEN_RECORDING_SETTINGS_URL, ACCESSIBILITY_SETTINGS_URL]);

        let only_screen = missing_permission_urls(PermissionStatus {
            screen_recording: false,
            accessibility: true,
        });
        assert_eq!(only_screen, vec![SCREEN_RECORDING_SETTINGS_URL]);

        let only_accessibility = missing_permission_urls(PermissionStatus {
            screen_recording: true,
            accessibility: false,
        });
        assert_eq!(only_accessibility, vec![ACCESSIBILITY_SETTINGS_URL]);

        let all_granted = missing_permission_urls(PermissionStatus {
            screen_recording: true,
            accessibility: true,
        });
        assert!(all_granted.is_empty());
    }

}
