# Recorder State Quality Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace string errors with typed errors and centralize recorder state transitions.

**Architecture:** Add `RecorderAction` and `RecorderStateError` enums and a `transition` helper on `RecorderState` that validates allowed states and updates `SessionState`. Update `start/pause/resume/stop` to delegate to `transition` with explicit allowed states.

**Tech Stack:** Rust, unit tests in module `recorder::state`.

---

### Task 1: Add typed error + action enums + transition helper

**Files:**
- Modify: `src-tauri/src/recorder/state.rs`

**Step 1: Write the failing test**

No new test required; existing tests cover transitions and can keep `assert!(is_err())`.

**Step 2: Run test to verify it fails**

Run: `cargo test recorder::state::tests`
Expected: compiler error until function signatures updated.

**Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderAction {
    Start,
    Pause,
    Resume,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderStateError {
    InvalidTransition { from: SessionState, action: RecorderAction },
}

impl RecorderState {
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
}
```

Update `start/pause/resume/stop` to call `transition` and return `Result<(), RecorderStateError>`.

**Step 4: Run test to verify it passes**

Run: `cargo test recorder::state::tests`
Expected: PASS

**Step 5: Commit**

Skip (per user request: no commits).
