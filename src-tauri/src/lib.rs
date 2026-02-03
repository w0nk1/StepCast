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
use std::fs;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use base64::Engine;

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
        .map_err(|error| format!("{error:?}"))?;

    // TODO: Hide panel and show recording indicator
    // Temporarily disabled - causes crash
    // if let Ok(panel) = app.get_webview_panel(panel::panel_label()) {
    //     panel.hide();
    // }
    // if let Err(e) = tray::set_recording_icon(&app) {
    //     eprintln!("Failed to set recording icon: {}", e);
    // }

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

    // TODO: Reset tray icon and show panel
    // Temporarily disabled - causes crash
    // if let Err(e) = tray::set_default_icon(&app) {
    //     eprintln!("Failed to reset tray icon: {}", e);
    // }
    // if let Ok(panel) = app.get_webview_panel(panel::panel_label()) {
    //     panel.show();
    // }

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
fn discard_recording(state: tauri::State<'_, RecorderAppState>) -> Result<(), String> {
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

    // Clear the session completely
    {
        let mut session_lock = state
            .session
            .lock()
            .map_err(|_| "session lock poisoned")?;
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

    Ok(())
}

#[derive(Serialize)]
struct StepWithBase64 {
    step: Step,
    image_base64: Option<String>,
}

fn load_screenshot_base64(path: &str) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(&buffer))
}

fn generate_html(title: &str, steps: &[Step]) -> String {
    let steps_html: String = steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let step_num = i + 1;
            let image_html = step.screenshot_path.as_ref()
                .and_then(|p| load_screenshot_base64(p))
                .map(|b64| format!(r#"<img src="data:image/png;base64,{}" alt="Step {}">"#, b64, step_num))
                .unwrap_or_default();
            let click_marker = if step.screenshot_path.is_some() {
                format!(r#"<div class="click-marker" style="left: {}%; top: {}%;"></div>"#,
                    step.click_x_percent, step.click_y_percent)
            } else {
                String::new()
            };
            format!(r#"
    <article class="step">
      <div class="step-header">
        <span class="step-number">Step {}</span>
        <span class="step-app">{} - "{}"</span>
      </div>
      <div class="step-image">
        {}
        {}
      </div>
    </article>"#, step_num, html_escape(&step.app), html_escape(&step.window_title), image_html, click_marker)
        })
        .collect();

    format!(r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{}</title>
<style>
* {{ box-sizing: border-box; }}
body {{ font-family: -apple-system, BlinkMacSystemFont, sans-serif; max-width: 800px; margin: 0 auto; padding: 40px 20px; color: #1d1d1f; line-height: 1.5; }}
h1 {{ margin-bottom: 8px; }}
.meta {{ color: #86868b; font-size: 14px; margin-bottom: 32px; }}
.step {{ margin-bottom: 32px; border: 1px solid #e8e8ed; border-radius: 12px; overflow: hidden; }}
.step-header {{ padding: 12px 16px; background: #f5f5f7; border-bottom: 1px solid #e8e8ed; display: flex; gap: 12px; align-items: center; }}
.step-number {{ font-weight: 600; font-size: 12px; text-transform: uppercase; color: #86868b; }}
.step-app {{ color: #1d1d1f; }}
.step-image {{ position: relative; background: #f5f5f7; }}
.step-image img {{ display: block; max-width: 100%; height: auto; }}
.click-marker {{ position: absolute; width: 16px; height: 16px; border-radius: 50%; background: #ff3b30; border: 3px solid #fff; box-shadow: 0 2px 8px rgba(0,0,0,0.3); transform: translate(-50%, -50%); }}
@media print {{ .step {{ break-inside: avoid; }} }}
</style>
</head>
<body>
<h1>{}</h1>
<p class="meta">{} steps</p>
{}
</body>
</html>"#, html_escape(title), html_escape(title), steps.len(), steps_html)
}

fn generate_markdown(title: &str, steps: &[Step]) -> String {
    let steps_md: String = steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let step_num = i + 1;
            let image_md = step.screenshot_path.as_ref()
                .and_then(|p| load_screenshot_base64(p))
                .map(|b64| format!("![Step {}](data:image/png;base64,{})\n\n", step_num, b64))
                .unwrap_or_default();
            format!("## Step {}\n\n{}**Action:** Clicked in {} - \"{}\"\n\n---\n\n",
                step_num, image_md, step.app, step.window_title)
        })
        .collect();

    format!("# {}\n\n{} steps\n\n---\n\n{}", title, steps.len(), steps_md)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[tauri::command]
fn export_html(
    state: tauri::State<'_, RecorderAppState>,
    title: String,
    output_path: String,
) -> Result<(), String> {
    let steps = {
        let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        session_lock.as_ref().map(|s| s.get_steps().to_vec()).unwrap_or_default()
    };

    let html = generate_html(&title, &steps);
    fs::write(&output_path, html).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

#[tauri::command]
fn export_markdown(
    state: tauri::State<'_, RecorderAppState>,
    title: String,
    output_path: String,
) -> Result<(), String> {
    let steps = {
        let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        session_lock.as_ref().map(|s| s.get_steps().to_vec()).unwrap_or_default()
    };

    let markdown = generate_markdown(&title, &steps);
    fs::write(&output_path, markdown).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

#[tauri::command]
fn export_html_temp(
    state: tauri::State<'_, RecorderAppState>,
    title: String,
) -> Result<String, String> {
    let steps = {
        let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        session_lock.as_ref().map(|s| s.get_steps().to_vec()).unwrap_or_default()
    };

    let html = generate_html(&title, &steps);

    // Use user's cache directory for better compatibility
    let cache_dir = dirs::cache_dir()
        .ok_or("Could not find cache directory")?;
    let stepcast_dir = cache_dir.join("stepcast");
    fs::create_dir_all(&stepcast_dir)
        .map_err(|e| format!("Failed to create cache dir: {}", e))?;

    let filename = format!("stepcast-guide-{}.html", chrono::Utc::now().timestamp_millis());
    let output_path = stepcast_dir.join(&filename);

    fs::write(&output_path, html)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(output_path.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _recorder = recorder::Recorder::new();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            export_html,
            export_markdown,
            export_html_temp,
            discard_recording,
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
