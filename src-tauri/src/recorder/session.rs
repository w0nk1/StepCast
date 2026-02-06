use super::types::Step;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Session {
    pub steps: Vec<Step>,
    pub temp_dir: PathBuf,
}

impl Session {
    pub fn new() -> std::io::Result<Self> {
        let id = Uuid::new_v4().to_string();

        // Create temp directory for this session
        let temp_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("com.stepcast.app")
            .join("sessions")
            .join(&id);

        std::fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            steps: Vec::new(),
            temp_dir,
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
        let sessions_dir = cache.join("com.stepcast.app").join("sessions");
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

    pub fn next_step_id(&self) -> String {
        format!("step-{:03}", self.steps.len() + 1)
    }

    pub fn screenshot_path(&self, step_id: &str) -> PathBuf {
        self.temp_dir.join(format!("{step_id}.png"))
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
}
