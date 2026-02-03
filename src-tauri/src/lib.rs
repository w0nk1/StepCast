// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod panel;
mod recorder;
mod tray;
use recorder::state::RecorderState;
use serde::Serialize;
use std::sync::Mutex;

struct RecorderAppState {
    recorder_state: Mutex<RecorderState>,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
struct PermissionStatus {
    screen_recording: bool,
    accessibility: bool,
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
    tauri_plugin_macos_permissions::request_screen_recording_permission().await;
    tauri_plugin_macos_permissions::request_accessibility_permission().await;
    let screen_recording = tauri_plugin_macos_permissions::check_screen_recording_permission().await;
    let accessibility = tauri_plugin_macos_permissions::check_accessibility_permission().await;
    PermissionStatus {
        screen_recording,
        accessibility,
    }
}

#[tauri::command]
async fn start_recording(state: tauri::State<'_, RecorderAppState>) -> Result<(), String> {
    let permissions = check_permissions().await;
    if !permissions.screen_recording || !permissions.accessibility {
        return Err("missing screen recording or accessibility permission".to_string());
    }

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
fn stop_recording(state: tauri::State<'_, RecorderAppState>) -> Result<(), String> {
    let mut recorder_state = state
        .recorder_state
        .lock()
        .map_err(|_| "recorder state lock poisoned".to_string())?;
    recorder_state
        .stop()
        .map_err(|error| format!("{error:?}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_nspanel::init())
        .setup(|app| {
            panel::init(app.handle())?;
            tray::create(app.handle())?;
            Ok(())
        })
        .manage(RecorderAppState {
            recorder_state: Mutex::new(RecorderState::new()),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            check_permissions,
            request_permissions,
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording
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
