use super::types::Step;
use serde::Serialize;
use std::path::PathBuf;
use uuid::Uuid;

/// Lightweight diagnostics collected during a recording session.
/// Written to `diagnostics.json` in the session cache on stop/discard.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionDiagnostics {
    /// Total clicks received (before filtering).
    pub clicks_received: u32,
    /// Clicks dropped by debounce / own-app / tray / panel filters.
    pub clicks_filtered: u32,
    /// Capture attempts that used a fallback path.
    pub captures_fallback: u32,
    /// Capture attempts that failed entirely (step recorded without screenshot).
    pub captures_failed: u32,
    /// Per-failure reasons, in order of occurrence.
    pub failure_reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub steps: Vec<Step>,
    pub temp_dir: PathBuf,
    pub diagnostics: SessionDiagnostics,
}

impl Session {
    pub fn new() -> std::io::Result<Self> {
        let id = Uuid::new_v4().to_string();

        // Create temp directory for this session
        let temp_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("com.w0nk1.stepcast")
            .join("sessions")
            .join(&id);

        std::fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            steps: Vec::new(),
            temp_dir,
            diagnostics: SessionDiagnostics::default(),
        })
    }

    /// Remove this session's temp directory and all screenshots.
    pub fn cleanup(&self) {
        if self.temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.temp_dir);
        }
    }

    /// Remove all session directories and temp exports from the cache.
    pub fn cleanup_all_sessions() {
        let cache = match dirs::cache_dir() {
            Some(d) => d,
            None => return,
        };

        // Session screenshot directories
        let sessions_dir = cache.join("com.w0nk1.stepcast").join("sessions");
        if sessions_dir.is_dir() {
            let _ = std::fs::remove_dir_all(&sessions_dir);
        }

        // Temp HTML files from PDF export
        let exports_dir = cache.join("stepcast");
        if exports_dir.is_dir() {
            let _ = std::fs::remove_dir_all(&exports_dir);
        }
    }

    pub fn add_step(&mut self, step: Step) {
        self.steps.push(step);
    }

    pub fn get_steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn last_step_mut(&mut self) -> Option<&mut Step> {
        self.steps.last_mut()
    }

    /// Update a step's note by ID. Returns the updated step or None if not found.
    pub fn update_step_note(&mut self, step_id: &str, note: Option<String>) -> Option<&Step> {
        let step = self.steps.iter_mut().find(|s| s.id == step_id)?;
        step.note = note;
        Some(step)
    }

    /// Remove a step by ID. Returns true if found and removed.
    pub fn delete_step(&mut self, step_id: &str) -> bool {
        let before = self.steps.len();
        self.steps.retain(|s| s.id != step_id);
        self.steps.len() < before
    }

    /// Reorder steps to match the given ID sequence.
    /// IDs not in the list are dropped; unknown IDs are ignored.
    pub fn reorder_steps(&mut self, step_ids: &[String]) {
        let mut reordered = Vec::with_capacity(step_ids.len());
        for id in step_ids {
            if let Some(pos) = self.steps.iter().position(|s| s.id == *id) {
                reordered.push(self.steps.swap_remove(pos));
            }
        }
        self.steps = reordered;
    }

    pub fn next_step_id(&self) -> String {
        format!("step-{:03}", self.steps.len() + 1)
    }

    pub fn screenshot_path(&self, step_id: &str) -> PathBuf {
        self.temp_dir.join(format!("{step_id}.png"))
    }

    /// Write diagnostics.json to the session cache directory.
    pub fn write_diagnostics(&self) {
        let path = self.temp_dir.join("diagnostics.json");
        match serde_json::to_string_pretty(&self.diagnostics) {
            Ok(json) => {
                let _ = std::fs::write(path, json);
            }
            Err(e) => {
                if cfg!(debug_assertions) {
                    eprintln!("Failed to serialize diagnostics: {e}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_creates_temp_dir() {
        let session = Session::new().expect("create session");
        assert!(session.temp_dir.exists());
        // Cleanup
        std::fs::remove_dir_all(&session.temp_dir).ok();
    }

    #[test]
    fn session_generates_step_ids() {
        let mut session = Session::new().expect("create session");
        assert_eq!(session.next_step_id(), "step-001");

        session.add_step(Step::sample());
        assert_eq!(session.next_step_id(), "step-002");

        // Cleanup
        std::fs::remove_dir_all(&session.temp_dir).ok();
    }

    #[test]
    fn update_step_note_sets_note() {
        let mut session = Session::new().expect("create session");
        session.add_step(Step::sample());

        let updated = session.update_step_note("step-1", Some("Hello".into()));
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().note, Some("Hello".into()));

        // Clear note
        let updated = session.update_step_note("step-1", None);
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().note, None);

        // Nonexistent step
        assert!(session.update_step_note("nonexistent", Some("x".into())).is_none());

        std::fs::remove_dir_all(&session.temp_dir).ok();
    }

    #[test]
    fn write_diagnostics_creates_json() {
        let mut session = Session::new().expect("create session");
        session.diagnostics.clicks_received = 10;
        session.diagnostics.clicks_filtered = 3;
        session.diagnostics.captures_fallback = 1;
        session.diagnostics.captures_failed = 0;
        session.diagnostics.failure_reasons.push("window capture produced empty file".into());

        session.write_diagnostics();

        let path = session.temp_dir.join("diagnostics.json");
        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).expect("read diagnostics.json");
        let parsed: serde_json::Value = serde_json::from_str(&contents).expect("parse json");
        assert_eq!(parsed["clicks_received"], 10);
        assert_eq!(parsed["clicks_filtered"], 3);
        assert_eq!(parsed["captures_fallback"], 1);
        assert_eq!(parsed["captures_failed"], 0);
        assert_eq!(parsed["failure_reasons"][0], "window capture produced empty file");

        // Cleanup
        std::fs::remove_dir_all(&session.temp_dir).ok();
    }
}
