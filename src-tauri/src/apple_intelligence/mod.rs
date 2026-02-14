use crate::recorder::types::Step;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityResponse {
    pub available: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub steps: Vec<Step>,
    #[serde(default)]
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResultItem {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub debug: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateFailureItem {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub results: Vec<GenerateResultItem>,
    pub failures: Vec<GenerateFailureItem>,
}

#[cfg(target_os = "macos")]
static AI_HELPER_PATH: OnceLock<PathBuf> = OnceLock::new();

#[cfg(target_os = "macos")]
pub fn init(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::path::BaseDirectory;
    use tauri::Manager;

    let path = app
        .path()
        .resolve("bin/stepcast_ai_helper", BaseDirectory::Resource)
        .map_err(|e| format!("resolve ai helper resource: {e}"))?;
    if !path.exists() {
        return Err(format!("missing ai helper resource at {}", path.display()));
    }
    AI_HELPER_PATH
        .set(path)
        .map_err(|_| "ai helper path already initialized".to_string())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn helper_path() -> Result<&'static PathBuf, String> {
    if let Some(p) = AI_HELPER_PATH.get() {
        return Ok(p);
    }

    // Best-effort fallback (dev/test): derive from the current executable location.
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    // StepCast.app/Contents/MacOS/StepCast -> StepCast.app/Contents/Resources/bin/stepcast_ai_helper
    let candidate = exe
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("Resources").join("bin").join("stepcast_ai_helper"))
        .ok_or("could not derive helper path".to_string())?;

    if !candidate.exists() {
        return Err(format!(
            "ai helper not initialized and fallback path missing: {}",
            candidate.display()
        ));
    }
    let _ = AI_HELPER_PATH.set(candidate);
    Ok(AI_HELPER_PATH.get().expect("just set"))
}

#[cfg(target_os = "macos")]
fn run_helper(args: &[&str], stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let helper = helper_path()?;
    let mut cmd = Command::new(helper);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }

    let mut child = cmd.spawn().map_err(|e| format!("spawn ai helper: {e}"))?;
    if let Some(input) = stdin {
        if let Some(mut w) = child.stdin.take() {
            w.write_all(input)
                .map_err(|e| format!("write helper stdin: {e}"))?;
        }
    }

    let out = child
        .wait_with_output()
        .map_err(|e| format!("wait ai helper: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "ai helper failed ({}): {}",
            out.status,
            stderr.trim()
        ));
    }
    Ok(out.stdout)
}

#[cfg(not(target_os = "macos"))]
fn run_helper(_args: &[&str], _stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
    Err("not supported on this platform".into())
}

pub fn availability() -> Result<AvailabilityResponse, String> {
    let out = run_helper(&["availability"], None)?;
    serde_json::from_slice(&out).map_err(|e| format!("parse availability json: {e}"))
}

pub fn generate_descriptions(
    steps: Vec<Step>,
    max_chars: usize,
) -> Result<GenerateResponse, String> {
    // Keep the Swift helper API stable: snake_case JSON.
    let req = GenerateRequest {
        steps,
        max_chars: Some(max_chars),
    };
    let input = serde_json::to_vec(&req).map_err(|e| format!("encode generate json: {e}"))?;
    let out = run_helper(&["generate"], Some(&input))?;
    serde_json::from_slice(&out).map_err(|e| format!("parse generate json: {e}"))
}

pub fn is_auth_placeholder(step: &Step) -> bool {
    step.window_title == "Authentication dialog (secure)"
        || step.app.to_lowercase() == "authentication"
}

pub fn is_blank_description(desc: Option<&str>) -> bool {
    desc.unwrap_or("").trim().is_empty()
}
