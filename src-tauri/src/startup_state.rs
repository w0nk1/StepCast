use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StartupState {
    pub has_launched_before: bool,
    #[serde(default)]
    pub last_seen_version: Option<String>,
}

fn state_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("com.w0nk1.stepcast").join("startup_state.json"))
}

pub fn load() -> StartupState {
    let Some(path) = state_path() else {
        return StartupState::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => StartupState::default(),
    }
}

pub fn save(state: &StartupState) -> Result<(), String> {
    let path = state_path().ok_or("config dir not found")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_not_launched() {
        let state = StartupState::default();
        assert!(!state.has_launched_before);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("startup_state.json");

        let state = StartupState {
            has_launched_before: true,
            last_seen_version: Some("0.2.0".to_string()),
        };
        let json = serde_json::to_string_pretty(&state).expect("serialize");
        std::fs::write(&path, &json).expect("write");

        let loaded: StartupState =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read"))
                .expect("deserialize");
        assert!(loaded.has_launched_before);
        assert_eq!(loaded.last_seen_version.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn old_json_without_version_loads_with_none() {
        let json = r#"{"has_launched_before": true}"#;
        let state: StartupState = serde_json::from_str(json).expect("deserialize");
        assert!(state.has_launched_before);
        assert!(state.last_seen_version.is_none());
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("nonexistent.json");
        let result: StartupState = match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => StartupState::default(),
        };
        assert!(!result.has_launched_before);
    }

    #[test]
    fn corrupt_json_returns_default() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("startup_state.json");
        std::fs::write(&path, "not valid json").expect("write corrupt file");

        let result: StartupState = match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => StartupState::default(),
        };
        assert!(!result.has_launched_before);
    }
}
