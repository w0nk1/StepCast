# StepCast MVP Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a macOS menu bar PSR-style recorder with screenshots and PDF/Markdown/HTML export.

**Architecture:** Tauri 2 app with Rust core (recorder + capture + storage) and React UI (menu bar panel). Export via HTML/Markdown templates; PDF via print dialog.

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Vite, bun, tauri-nspanel, tauri-plugin-macos-permissions, tauri-plugin-log.

---

### Task 1: Scaffold app + verify test harness

**Files:**
- Create (scaffold): `package.json`, `vite.config.ts`, `src/main.tsx`, `src/App.tsx`, `src-tauri/Cargo.toml`, `src-tauri/src/main.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/recorder/mod.rs`
- Test: `src-tauri/src/recorder/mod.rs`

**Step 1: Scaffold Tauri 2 app**

Run: `bunx create-tauri-app@latest .`
- Select: Tauri v2, React + TS, package manager bun.

**Step 2: Write failing test to verify Rust harness**

```rust
// src-tauri/src/recorder/mod.rs
#[cfg(test)]
mod tests {
    #[test]
    fn harness_fails_first() {
        assert_eq!(2 + 2, 5);
    }
}
```

**Step 3: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: FAIL (assertion failed)

**Step 4: Make test pass**

```rust
assert_eq!(2 + 2, 4);
```

**Step 5: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

**Step 6: Commit**

```bash
git add src-tauri/src/recorder/mod.rs
git commit -m "chore: scaffold stepcast"
```

---

### Task 2: Data model + serialization

**Files:**
- Create: `src-tauri/src/recorder/types.rs`
- Modify: `src-tauri/src/recorder/mod.rs`
- Test: `src-tauri/src/recorder/types.rs`

**Step 1: Write failing test**

```rust
// src-tauri/src/recorder/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_roundtrip_json() {
        let step = Step::sample();
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder::types::tests::step_roundtrip_json`
Expected: FAIL (Step not defined)

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/recorder/types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionType {
    Click,
    Shortcut,
    Note,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub ts: i64,
    pub action: ActionType,
    pub x: i32,
    pub y: i32,
    pub app: String,
    pub window_title: String,
    pub screenshot_path: Option<String>,
    pub note: Option<String>,
}

impl Step {
    #[cfg(test)]
    pub fn sample() -> Self {
        Self {
            id: "step-1".to_string(),
            ts: 0,
            action: ActionType::Click,
            x: 10,
            y: 20,
            app: "Finder".to_string(),
            window_title: "Downloads".to_string(),
            screenshot_path: Some("screenshots/step-001.png".to_string()),
            note: None,
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder::types::tests::step_roundtrip_json`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/recorder/types.rs src-tauri/src/recorder/mod.rs
git commit -m "feat: add recorder data model"
```

---

### Task 3: Recorder state machine

**Files:**
- Create: `src-tauri/src/recorder/state.rs`
- Modify: `src-tauri/src/recorder/mod.rs`
- Test: `src-tauri/src/recorder/state.rs`

**Step 1: Write failing tests**

```rust
// src-tauri/src/recorder/state.rs
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder::state::tests`
Expected: FAIL (RecorderState missing)

**Step 3: Implement minimal state machine**

```rust
// src-tauri/src/recorder/state.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionState {
    Idle,
    Recording,
    Paused,
    Stopped,
}

pub struct RecorderState {
    state: SessionState,
}

impl RecorderState {
    pub fn new() -> Self {
        Self { state: SessionState::Idle }
    }

    pub fn start(&mut self) -> Result<(), String> {
        match self.state {
            SessionState::Idle | SessionState::Stopped => {
                self.state = SessionState::Recording;
                Ok(())
            }
            _ => Err("invalid state".to_string()),
        }
    }

    pub fn pause(&mut self) -> Result<(), String> {
        match self.state {
            SessionState::Recording => {
                self.state = SessionState::Paused;
                Ok(())
            }
            _ => Err("invalid state".to_string()),
        }
    }

    pub fn resume(&mut self) -> Result<(), String> {
        match self.state {
            SessionState::Paused => {
                self.state = SessionState::Recording;
                Ok(())
            }
            _ => Err("invalid state".to_string()),
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        match self.state {
            SessionState::Recording | SessionState::Paused => {
                self.state = SessionState::Stopped;
                Ok(())
            }
            _ => Err("invalid state".to_string()),
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder::state::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/recorder/state.rs src-tauri/src/recorder/mod.rs
git commit -m "feat: add recorder state machine"
```

---

### Task 4: Screenshot capture backend (macOS)

**Files:**
- Create: `src-tauri/src/recorder/capture.rs`
- Create: `src-tauri/src/recorder/macos_screencapture.rs`
- Modify: `src-tauri/src/recorder/mod.rs`
- Test: `src-tauri/src/recorder/macos_screencapture.rs`

**Step 1: Write failing test for command args**

```rust
// src-tauri/src/recorder/macos_screencapture.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_region_args() {
        let args = build_args(10, 20, 300, 200, "/tmp/a.png");
        assert!(args.contains(&"-R".to_string()));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder::macos_screencapture::tests::builds_region_args`
Expected: FAIL (build_args missing)

**Step 3: Implement capture backend**

```rust
// src-tauri/src/recorder/capture.rs
pub trait CaptureBackend {
    fn capture_region(&self, x: i32, y: i32, w: i32, h: i32, output: &str) -> Result<(), String>;
}

// src-tauri/src/recorder/macos_screencapture.rs
use std::process::Command;
use super::capture::CaptureBackend;

pub struct MacOsScreencapture;

pub fn build_args(x: i32, y: i32, w: i32, h: i32, output: &str) -> Vec<String> {
    vec![
        "-x".to_string(),
        "-R".to_string(),
        format!("{},{},{},{}", x, y, w, h),
        output.to_string(),
    ]
}

impl CaptureBackend for MacOsScreencapture {
    fn capture_region(&self, x: i32, y: i32, w: i32, h: i32, output: &str) -> Result<(), String> {
        let status = Command::new("screencapture")
            .args(build_args(x, y, w, h, output))
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() { Ok(()) } else { Err("screencapture failed".to_string()) }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder::macos_screencapture::tests::builds_region_args`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/recorder/capture.rs src-tauri/src/recorder/macos_screencapture.rs src-tauri/src/recorder/mod.rs
git commit -m "feat: add macos screencapture backend"
```

---

### Task 5: Storage layer (steps + files)

**Files:**
- Create: `src-tauri/src/recorder/storage.rs`
- Modify: `src-tauri/src/recorder/mod.rs`
- Test: `src-tauri/src/recorder/storage.rs`

**Step 1: Write failing test**

```rust
// src-tauri/src/recorder/storage.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_steps_json() {
        let dir = tempfile::tempdir().unwrap();
        let steps = vec![crate::recorder::types::Step::sample()];
        write_steps(dir.path(), &steps).unwrap();
        let content = std::fs::read_to_string(dir.path().join("steps.json")).unwrap();
        assert!(content.contains("step-1"));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder::storage::tests::writes_steps_json`
Expected: FAIL (write_steps missing)

**Step 3: Implement storage helpers**

```rust
// src-tauri/src/recorder/storage.rs
use std::path::Path;
use crate::recorder::types::Step;

pub fn write_steps(dir: &Path, steps: &[Step]) -> Result<(), String> {
    let path = dir.join("steps.json");
    let json = serde_json::to_string_pretty(steps).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder::storage::tests::writes_steps_json`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/recorder/storage.rs src-tauri/src/recorder/mod.rs
git commit -m "feat: add recorder storage"
```

---

### Task 6: Tauri commands + permissions plugin

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

**Step 1: Write failing test**

```rust
// src-tauri/src/lib.rs
// Add a unit test to ensure permission check returns false when denied (mocked)
// This will fail until check function exists.
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml lib::tests::permission_check_default`
Expected: FAIL (test missing)

**Step 3: Implement commands + plugin**

- Add dependency: `tauri-plugin-macos-permissions`.
- Register plugin in `run()`.
- Add commands: `check_permissions`, `request_permissions`, `start_recording`, `pause_recording`, `resume_recording`, `stop_recording`.
- Update `capabilities/default.json` to allow those commands.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml lib::tests::permission_check_default`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat: add tauri commands and permissions"
```

---

### Task 7: Menu bar panel + tray

**Files:**
- Create: `src-tauri/src/panel.rs`
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`

**Step 1: Write failing test**

```rust
// src-tauri/src/panel.rs
// Add a minimal test for panel init stub (expecting Err before implementation)
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml panel::tests::init_panel`
Expected: FAIL

**Step 3: Implement panel + tray**

- Use `tauri-nspanel` to create non-activating panel.
- Tray icon toggles panel; panel hides on blur.
- Position panel using tray icon rect (OpenUsage pattern).
- Set `macOSPrivateApi: true` and `decorations: false`, `transparent: true`, `visible: false` in `tauri.conf.json`.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml panel::tests::init_panel`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/panel.rs src-tauri/src/tray.rs src-tauri/src/lib.rs src-tauri/tauri.conf.json
git commit -m "feat: add menu bar panel"
```

---

### Task 8: Exporter (HTML + Markdown) + tests

**Files:**
- Create: `src/export/render.ts`
- Create: `src/export/templates/guide.html`
- Create: `src/export/templates/guide.md`
- Test: `src/export/render.test.ts`

**Step 1: Write failing test**

```ts
// src/export/render.test.ts
import { renderHtml } from "./render";

test("renders title", () => {
  const html = renderHtml({ title: "Demo", steps: [] });
  expect(html).toContain("Demo");
});
```

**Step 2: Run test to verify it fails**

Run: `bun test src/export/render.test.ts`
Expected: FAIL (renderHtml missing)

**Step 3: Implement render functions**

```ts
// src/export/render.ts
import htmlTemplate from "./templates/guide.html?raw";
import mdTemplate from "./templates/guide.md?raw";

export function renderHtml(data: { title: string; steps: any[] }) {
  return htmlTemplate.replace("{{title}}", data.title);
}

export function renderMarkdown(data: { title: string; steps: any[] }) {
  return mdTemplate.replace("{{title}}", data.title);
}
```

**Step 4: Run test to verify it passes**

Run: `bun test src/export/render.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/export/render.ts src/export/templates/guide.html src/export/templates/guide.md src/export/render.test.ts
git commit -m "feat: add export templates"
```

---

### Task 9: UI wiring + PDF export via print

**Files:**
- Modify: `src/App.tsx`
- Create: `src/components/RecorderPanel.tsx`
- Modify: `src/main.tsx`

**Step 1: Write failing UI test (optional)**

```ts
// src/components/RecorderPanel.test.tsx
// Minimal render test expecting "Record" button
```

**Step 2: Run test to verify it fails**

Run: `bun test src/components/RecorderPanel.test.tsx`
Expected: FAIL

**Step 3: Implement UI + invoke commands**

- Permission status banner (Screen Recording + Accessibility).
- Record / Pause / Stop buttons.
- Steps list with edit for note text.
- Export buttons: HTML, Markdown, PDF.
- PDF: open a preview window with rendered HTML and call `window.print()`.

**Step 4: Run tests to verify they pass**

Run: `bun test src/components/RecorderPanel.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add src/App.tsx src/main.tsx src/components/RecorderPanel.tsx src/components/RecorderPanel.test.tsx
git commit -m "feat: add recorder panel ui"
```

---

### Task 10: Docs + manual verification

**Files:**
- Create: `README.md`
- Create: `LICENSE`

**Step 1: Write docs**

- Explain permissions and first-run flow.
- Explain unsigned build + Gatekeeper bypass.

**Step 2: Manual verification**

Run: `bun tauri dev`
Checklist:
- Tray icon shows
- Panel shows/hides on click/blur
- Permission flow works
- Steps are recorded with screenshots
- HTML/Markdown/PDF export works

**Step 3: Commit**

```bash
git add README.md LICENSE
git commit -m "docs: add readme and license"
```
