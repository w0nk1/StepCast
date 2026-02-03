#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Recording,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderAction {
    Start,
    Pause,
    Resume,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderStateError {
    InvalidTransition {
        from: SessionState,
        action: RecorderAction,
    },
}

pub struct RecorderState {
    state: SessionState,
}

impl RecorderState {
    pub fn new() -> Self {
        Self {
            state: SessionState::Idle,
        }
    }

    fn transition(
        &mut self,
        allowed: &[SessionState],
        to: SessionState,
        action: RecorderAction,
    ) -> Result<(), RecorderStateError> {
        if allowed.contains(&self.state) {
            self.state = to;
            Ok(())
        } else {
            Err(RecorderStateError::InvalidTransition {
                from: self.state,
                action,
            })
        }
    }

    pub fn start(&mut self) -> Result<(), RecorderStateError> {
        self.transition(
            &[SessionState::Idle, SessionState::Stopped],
            SessionState::Recording,
            RecorderAction::Start,
        )
    }

    pub fn pause(&mut self) -> Result<(), RecorderStateError> {
        self.transition(
            &[SessionState::Recording],
            SessionState::Paused,
            RecorderAction::Pause,
        )
    }

    pub fn resume(&mut self) -> Result<(), RecorderStateError> {
        self.transition(
            &[SessionState::Paused],
            SessionState::Recording,
            RecorderAction::Resume,
        )
    }

    pub fn stop(&mut self) -> Result<(), RecorderStateError> {
        self.transition(
            &[SessionState::Recording, SessionState::Paused],
            SessionState::Stopped,
            RecorderAction::Stop,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_pause_stop_flow() {
        let mut state = RecorderState::new();
        assert!(state.start().is_ok());
        assert!(state.pause().is_ok());
        assert!(state.resume().is_ok());
        assert!(state.stop().is_ok());
    }

    #[test]
    fn cannot_pause_when_idle() {
        let mut state = RecorderState::new();
        assert!(state.pause().is_err());
    }
}
