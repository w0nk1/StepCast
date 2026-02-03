use super::types::Step;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub started_at: i64,
    pub steps: Vec<Step>,
    pub temp_dir: PathBuf,
}

impl Session {
    pub fn new() -> std::io::Result<Self> {
        let id = Uuid::new_v4().to_string();
        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        // Create temp directory for this session
        let temp_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("com.markus.stepcast")
            .join("sessions")
            .join(&id);

        std::fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            id,
            started_at,
            steps: Vec::new(),
            temp_dir,
        })
    }

    pub fn add_step(&mut self, step: Step) {
        self.steps.push(step);
    }

    pub fn get_steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub fn next_step_id(&self) -> String {
        format!("step-{:03}", self.steps.len() + 1)
    }

    pub fn screenshot_path(&self, step_id: &str) -> PathBuf {
        self.temp_dir.join(format!("{}.png", step_id))
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
