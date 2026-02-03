# Core Recording Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement click detection, window screenshots, and real-time step display so users can record documentation workflows.

**Architecture:** CGEventTap listens for clicks → Window info fetched via Accessibility API → Screenshot captured via CGWindowListCreateImage → Step emitted to frontend via Tauri events → UI displays steps with thumbnails.

**Tech Stack:** Rust (core-graphics, core-foundation, cocoa), Tauri Events, React/TypeScript

---

## Task 1: Add Rust Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add required crates**

Add to `[dependencies]` section:

```toml
core-graphics = "0.24"
core-foundation = "0.10"
objc2 = "0.6"
objc2-app-kit = { version = "0.3", features = ["NSWorkspace", "NSRunningApplication"] }
base64 = "0.22"
```

**Step 2: Verify dependencies resolve**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: add core-graphics and cocoa dependencies for click capture"
```

---

## Task 2: Click Event Types

**Files:**
- Create: `src-tauri/src/recorder/click_event.rs`
- Modify: `src-tauri/src/recorder/mod.rs`

**Step 1: Create click event types**

Create `src-tauri/src/recorder/click_event.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickEvent {
    pub x: i32,
    pub y: i32,
    pub timestamp_ms: i64,
    pub button: MouseButton,
}

impl ClickEvent {
    pub fn new(x: i32, y: i32, button: MouseButton) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Self {
            x,
            y,
            timestamp_ms,
            button,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_event_creates_with_timestamp() {
        let event = ClickEvent::new(100, 200, MouseButton::Left);
        assert_eq!(event.x, 100);
        assert_eq!(event.y, 200);
        assert!(event.timestamp_ms > 0);
    }
}
```

**Step 2: Export from mod.rs**

Add to `src-tauri/src/recorder/mod.rs`:

```rust
pub mod click_event;
```

**Step 3: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml click_event`
Expected: PASS

**Step 4: Commit**

```bash
git add src-tauri/src/recorder/click_event.rs src-tauri/src/recorder/mod.rs
git commit -m "feat: add ClickEvent type for mouse click capture"
```

---

## Task 3: Window Info Types

**Files:**
- Create: `src-tauri/src/recorder/window_info.rs`
- Modify: `src-tauri/src/recorder/mod.rs`

**Step 1: Create window info types**

Create `src-tauri/src/recorder/window_info.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub window_id: u32,
    pub bounds: WindowBounds,
}

impl WindowInfo {
    #[cfg(test)]
    pub fn sample() -> Self {
        Self {
            app_name: "Finder".to_string(),
            window_title: "Downloads".to_string(),
            window_id: 12345,
            bounds: WindowBounds {
                x: 100,
                y: 100,
                width: 800,
                height: 600,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_info_serializes() {
        let info = WindowInfo::sample();
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("Finder"));
        assert!(json.contains("Downloads"));
    }
}
```

**Step 2: Export from mod.rs**

Add to `src-tauri/src/recorder/mod.rs`:

```rust
pub mod window_info;
```

**Step 3: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml window_info`
Expected: PASS

**Step 4: Commit**

```bash
git add src-tauri/src/recorder/window_info.rs src-tauri/src/recorder/mod.rs
git commit -m "feat: add WindowInfo type for active window detection"
```

---

## Task 4: Get Frontmost Window (macOS)

**Files:**
- Modify: `src-tauri/src/recorder/window_info.rs`

**Step 1: Add macOS imports and implement get_frontmost_window**

Update `src-tauri/src/recorder/window_info.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug)]
pub enum WindowError {
    NoFrontmostApp,
    NoWindows,
    WindowInfoFailed,
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowError::NoFrontmostApp => write!(f, "no frontmost application"),
            WindowError::NoWindows => write!(f, "no windows found"),
            WindowError::WindowInfoFailed => write!(f, "failed to get window info"),
        }
    }
}

impl std::error::Error for WindowError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub window_id: u32,
    pub bounds: WindowBounds,
}

#[cfg(target_os = "macos")]
pub fn get_frontmost_window() -> Result<WindowInfo, WindowError> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::*;
    use objc2_app_kit::{NSApplicationActivationPolicy, NSWorkspace};

    // Get frontmost app
    let workspace = unsafe { NSWorkspace::sharedWorkspace() };
    let frontmost = unsafe { workspace.frontmostApplication() }
        .ok_or(WindowError::NoFrontmostApp)?;

    let app_name = unsafe { frontmost.localizedName() }
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let pid = unsafe { frontmost.processIdentifier() };

    // Get windows for this app
    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };

    if window_list.is_null() {
        return Err(WindowError::NoWindows);
    }

    let windows: Vec<CFDictionaryRef> = unsafe {
        let count = core_foundation::array::CFArrayGetCount(window_list as _);
        (0..count)
            .map(|i| core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef)
            .collect()
    };

    for window_dict in windows {
        let dict = unsafe { core_foundation::dictionary::CFDictionary::<CFString, CFType>::wrap_under_get_rule(window_dict) };

        // Check if window belongs to frontmost app
        let owner_pid_key = CFString::new("kCGWindowOwnerPID");
        if let Some(owner_pid) = dict.find(&owner_pid_key) {
            let owner_pid: CFNumber = unsafe { CFNumber::wrap_under_get_rule(*owner_pid as _) };
            if let Some(owner_pid_val) = owner_pid.to_i32() {
                if owner_pid_val != pid {
                    continue;
                }
            }
        }

        // Get window ID
        let window_id_key = CFString::new("kCGWindowNumber");
        let window_id = dict.find(&window_id_key)
            .and_then(|v| {
                let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(*v as _) };
                num.to_i32().map(|n| n as u32)
            })
            .unwrap_or(0);

        // Get window title
        let title_key = CFString::new("kCGWindowName");
        let window_title = dict.find(&title_key)
            .map(|v| {
                let s: CFString = unsafe { CFString::wrap_under_get_rule(*v as _) };
                s.to_string()
            })
            .unwrap_or_default();

        // Get window bounds
        let bounds_key = CFString::new("kCGWindowBounds");
        let bounds = dict.find(&bounds_key)
            .map(|v| {
                let bounds_dict: core_foundation::dictionary::CFDictionary<CFString, CFNumber> =
                    unsafe { core_foundation::dictionary::CFDictionary::wrap_under_get_rule(*v as _) };

                let x = bounds_dict.find(&CFString::new("X"))
                    .and_then(|n| unsafe { CFNumber::wrap_under_get_rule(*n as _) }.to_i32())
                    .unwrap_or(0);
                let y = bounds_dict.find(&CFString::new("Y"))
                    .and_then(|n| unsafe { CFNumber::wrap_under_get_rule(*n as _) }.to_i32())
                    .unwrap_or(0);
                let width = bounds_dict.find(&CFString::new("Width"))
                    .and_then(|n| unsafe { CFNumber::wrap_under_get_rule(*n as _) }.to_i32())
                    .unwrap_or(0) as u32;
                let height = bounds_dict.find(&CFString::new("Height"))
                    .and_then(|n| unsafe { CFNumber::wrap_under_get_rule(*n as _) }.to_i32())
                    .unwrap_or(0) as u32;

                WindowBounds { x, y, width, height }
            })
            .unwrap_or(WindowBounds { x: 0, y: 0, width: 800, height: 600 });

        // Skip windows with no title (menu bar, etc)
        if window_title.is_empty() {
            continue;
        }

        return Ok(WindowInfo {
            app_name,
            window_title,
            window_id,
            bounds,
        });
    }

    // Fallback: return app info without specific window
    Ok(WindowInfo {
        app_name,
        window_title: String::new(),
        window_id: 0,
        bounds: WindowBounds { x: 0, y: 0, width: 800, height: 600 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_info_serializes() {
        let info = WindowInfo {
            app_name: "Finder".to_string(),
            window_title: "Downloads".to_string(),
            window_id: 12345,
            bounds: WindowBounds {
                x: 100,
                y: 100,
                width: 800,
                height: 600,
            },
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("Finder"));
        assert!(json.contains("Downloads"));
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles (we can't easily test window detection in CI)

**Step 3: Commit**

```bash
git add src-tauri/src/recorder/window_info.rs
git commit -m "feat: implement get_frontmost_window for macOS"
```

---

## Task 5: Screenshot Capture by Window ID

**Files:**
- Modify: `src-tauri/src/recorder/macos_screencapture.rs`

**Step 1: Implement window screenshot capture**

Replace `src-tauri/src/recorder/macos_screencapture.rs`:

```rust
use super::capture::CaptureError;
use core_graphics::display::*;
use core_graphics::image::CGImage;
use std::path::Path;

pub fn capture_window(window_id: u32, output_path: &Path) -> Result<(), CaptureError> {
    // Capture the specific window
    let image = unsafe {
        CGImage::from_ptr(CGWindowListCreateImage(
            CGRectNull,
            kCGWindowListOptionIncludingWindow,
            window_id,
            kCGWindowImageBoundsIgnoreFraming | kCGWindowImageNominalResolution,
        ))
    };

    // Convert CGImage to PNG and save
    save_cgimage_as_png(&image, output_path)?;

    Ok(())
}

pub fn capture_screen_region(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    output_path: &Path,
) -> Result<(), CaptureError> {
    let rect = CGRect::new(
        &CGPoint::new(x as f64, y as f64),
        &CGSize::new(width as f64, height as f64),
    );

    let image = unsafe {
        CGImage::from_ptr(CGWindowListCreateImage(
            rect,
            kCGWindowListOptionOnScreenOnly,
            kCGNullWindowID,
            kCGWindowImageDefault,
        ))
    };

    save_cgimage_as_png(&image, output_path)?;

    Ok(())
}

fn save_cgimage_as_png(image: &CGImage, output_path: &Path) -> Result<(), CaptureError> {
    use core_foundation::data::CFData;
    use core_foundation::url::CFURL;
    use core_graphics::image::CGImageDestination;

    let url = CFURL::from_path(output_path, false)
        .ok_or_else(|| CaptureError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid output path",
        )))?;

    let dest = CGImageDestination::new(&url, "public.png", 1)
        .ok_or_else(|| CaptureError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failed to create image destination",
        )))?;

    dest.add_image(image, None);

    if !dest.finalize() {
        return Err(CaptureError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failed to write image",
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn capture_screen_region_creates_file() {
        let dir = tempdir().expect("tempdir");
        let output = dir.path().join("screenshot.png");

        // Capture a small region of the screen
        let result = capture_screen_region(0, 0, 100, 100, &output);

        // This may fail in headless CI, so we just check it doesn't panic
        if result.is_ok() {
            assert!(output.exists());
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles

**Step 3: Commit**

```bash
git add src-tauri/src/recorder/macos_screencapture.rs
git commit -m "feat: implement window screenshot capture via CGWindowListCreateImage"
```

---

## Task 6: Session Management

**Files:**
- Create: `src-tauri/src/recorder/session.rs`
- Modify: `src-tauri/src/recorder/mod.rs`
- Modify: `src-tauri/src/recorder/types.rs`

**Step 1: Update Step type to include relative click position**

Update `src-tauri/src/recorder/types.rs` to add `click_x_percent` and `click_y_percent`:

```rust
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
    pub click_x_percent: f32,
    pub click_y_percent: f32,
    pub app: String,
    pub window_title: String,
    pub screenshot_path: Option<String>,
    pub note: Option<String>,
}

#[cfg(test)]
impl Step {
    pub fn sample() -> Self {
        Self {
            id: "step-1".to_string(),
            ts: 0,
            action: ActionType::Click,
            x: 10,
            y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".to_string(),
            window_title: "Downloads".to_string(),
            screenshot_path: Some("screenshots/step-001.png".to_string()),
            note: None,
        }
    }
}

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

**Step 2: Create session management**

Create `src-tauri/src/recorder/session.rs`:

```rust
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
```

**Step 3: Add uuid dependency**

Add to `src-tauri/Cargo.toml`:

```toml
uuid = { version = "1.0", features = ["v4"] }
dirs = "5.0"
```

**Step 4: Export from mod.rs**

Add to `src-tauri/src/recorder/mod.rs`:

```rust
pub mod session;
```

**Step 5: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml session`
Expected: PASS

**Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/recorder/session.rs src-tauri/src/recorder/mod.rs src-tauri/src/recorder/types.rs
git commit -m "feat: add Session management for recording sessions"
```

---

## Task 7: Click Listener with CGEventTap

**Files:**
- Create: `src-tauri/src/recorder/click_listener.rs`
- Modify: `src-tauri/src/recorder/mod.rs`

**Step 1: Implement click listener**

Create `src-tauri/src/recorder/click_listener.rs`:

```rust
use super::click_event::{ClickEvent, MouseButton};
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

pub struct ClickListener {
    running: Arc<AtomicBool>,
    receiver: Receiver<ClickEvent>,
    _handle: thread::JoinHandle<()>,
}

impl ClickListener {
    pub fn start() -> Result<Self, String> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let (sender, receiver) = channel();

        let handle = thread::spawn(move || {
            run_event_tap(running_clone, sender);
        });

        // Give the tap time to start
        thread::sleep(std::time::Duration::from_millis(100));

        Ok(Self {
            running,
            receiver,
            _handle: handle,
        })
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        CFRunLoop::get_current().stop();
    }

    pub fn try_recv(&self) -> Option<ClickEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn recv(&self) -> Option<ClickEvent> {
        self.receiver.recv().ok()
    }
}

fn run_event_tap(running: Arc<AtomicBool>, sender: Sender<ClickEvent>) {
    let sender = Arc::new(sender);
    let sender_clone = sender.clone();

    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::LeftMouseUp, CGEventType::RightMouseUp],
        move |_proxy, event_type, event| {
            let button = match event_type {
                CGEventType::LeftMouseUp => MouseButton::Left,
                CGEventType::RightMouseUp => MouseButton::Right,
                _ => return Some(event.clone()),
            };

            let location = event.location();
            let click_event = ClickEvent::new(
                location.x as i32,
                location.y as i32,
                button,
            );

            sender_clone.send(click_event).ok();

            Some(event.clone())
        },
    );

    match tap {
        Ok(tap) => {
            let loop_source = tap.mach_port()
                .create_runloop_source(0)
                .expect("create runloop source");

            let run_loop = CFRunLoop::get_current();
            run_loop.add_source(&loop_source, unsafe { kCFRunLoopCommonModes });

            tap.enable();

            while running.load(Ordering::SeqCst) {
                CFRunLoop::run_in_mode(
                    unsafe { kCFRunLoopCommonModes },
                    std::time::Duration::from_millis(100),
                    true,
                );
            }
        }
        Err(()) => {
            eprintln!("Failed to create event tap. Check Accessibility permissions.");
        }
    }
}

#[cfg(test)]
mod tests {
    // Click listener requires accessibility permissions and a GUI environment,
    // so we can't easily test it in CI. Manual testing required.
}
```

**Step 2: Export from mod.rs**

Add to `src-tauri/src/recorder/mod.rs`:

```rust
pub mod click_listener;
```

**Step 3: Verify it compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles

**Step 4: Commit**

```bash
git add src-tauri/src/recorder/click_listener.rs src-tauri/src/recorder/mod.rs
git commit -m "feat: implement CGEventTap click listener for global mouse events"
```

---

## Task 8: Capture Pipeline

**Files:**
- Create: `src-tauri/src/recorder/pipeline.rs`
- Modify: `src-tauri/src/recorder/mod.rs`

**Step 1: Create the capture pipeline**

Create `src-tauri/src/recorder/pipeline.rs`:

```rust
use super::click_event::ClickEvent;
use super::click_listener::ClickListener;
use super::macos_screencapture::capture_window;
use super::session::Session;
use super::types::{ActionType, Step};
use super::window_info::get_frontmost_window;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

pub struct CapturePipeline {
    running: Arc<AtomicBool>,
    session: Arc<Mutex<Session>>,
    _handle: Option<thread::JoinHandle<()>>,
}

impl CapturePipeline {
    pub fn new(session: Session) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            session: Arc::new(Mutex::new(session)),
            _handle: None,
        }
    }

    pub fn start(&mut self, app_handle: AppHandle) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Pipeline already running".to_string());
        }

        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let session = self.session.clone();

        let handle = thread::spawn(move || {
            run_pipeline(running, session, app_handle);
        });

        self._handle = Some(handle);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn get_session(&self) -> Arc<Mutex<Session>> {
        self.session.clone()
    }
}

fn run_pipeline(
    running: Arc<AtomicBool>,
    session: Arc<Mutex<Session>>,
    app_handle: AppHandle,
) {
    let listener = match ClickListener::start() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to start click listener: {}", e);
            return;
        }
    };

    while running.load(Ordering::SeqCst) {
        if let Some(click) = listener.try_recv() {
            if let Err(e) = process_click(&click, &session, &app_handle) {
                eprintln!("Failed to process click: {}", e);
            }
        }
        thread::sleep(std::time::Duration::from_millis(10));
    }

    listener.stop();
}

fn process_click(
    click: &ClickEvent,
    session: &Arc<Mutex<Session>>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    // Get window info
    let window_info = get_frontmost_window()
        .map_err(|e| format!("Failed to get window info: {}", e))?;

    // Lock session and prepare step
    let (step_id, screenshot_path, step) = {
        let mut session = session.lock().map_err(|e| e.to_string())?;
        let step_id = session.next_step_id();
        let screenshot_path = session.screenshot_path(&step_id);

        // Calculate click position relative to window
        let click_x_percent = if window_info.bounds.width > 0 {
            ((click.x - window_info.bounds.x) as f32 / window_info.bounds.width as f32 * 100.0)
                .clamp(0.0, 100.0)
        } else {
            50.0
        };
        let click_y_percent = if window_info.bounds.height > 0 {
            ((click.y - window_info.bounds.y) as f32 / window_info.bounds.height as f32 * 100.0)
                .clamp(0.0, 100.0)
        } else {
            50.0
        };

        let step = Step {
            id: step_id.clone(),
            ts: click.timestamp_ms,
            action: ActionType::Click,
            x: click.x,
            y: click.y,
            click_x_percent,
            click_y_percent,
            app: window_info.app_name.clone(),
            window_title: window_info.window_title.clone(),
            screenshot_path: Some(screenshot_path.to_string_lossy().to_string()),
            note: None,
        };

        (step_id, screenshot_path, step)
    };

    // Capture screenshot (outside of lock)
    if window_info.window_id > 0 {
        capture_window(window_info.window_id, &screenshot_path)
            .map_err(|e| format!("Screenshot failed: {}", e))?;
    }

    // Add step to session and emit event
    {
        let mut session = session.lock().map_err(|e| e.to_string())?;
        session.add_step(step.clone());
    }

    // Emit to frontend
    app_handle
        .emit("step-captured", &step)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}
```

**Step 2: Export from mod.rs**

Add to `src-tauri/src/recorder/mod.rs`:

```rust
pub mod pipeline;
```

**Step 3: Verify it compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles

**Step 4: Commit**

```bash
git add src-tauri/src/recorder/pipeline.rs src-tauri/src/recorder/mod.rs
git commit -m "feat: implement capture pipeline connecting clicks to screenshots"
```

---

## Task 9: Integrate Pipeline with Tauri Commands

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: Update lib.rs to use the pipeline**

Update `src-tauri/src/lib.rs` to integrate the pipeline:

```rust
mod panel;
mod recorder;
mod tray;

use recorder::pipeline::CapturePipeline;
use recorder::session::Session;
use recorder::state::RecorderState;
use recorder::types::Step;
use serde::Serialize;
use std::sync::Mutex;
use tauri::Manager;

struct AppState {
    recorder_state: Mutex<RecorderState>,
    pipeline: Mutex<Option<CapturePipeline>>,
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

#[tauri::command]
async fn start_recording(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let permissions = check_permissions().await;
    if !permissions.screen_recording || !permissions.accessibility {
        return Err("missing screen recording or accessibility permission".to_string());
    }

    // Update state machine
    {
        let mut recorder_state = state
            .recorder_state
            .lock()
            .map_err(|_| "recorder state lock poisoned".to_string())?;
        recorder_state
            .start()
            .map_err(|error| format!("{error:?}"))?;
    }

    // Create and start pipeline
    let session = Session::new().map_err(|e| format!("Failed to create session: {}", e))?;
    let mut pipeline = CapturePipeline::new(session);
    pipeline.start(app_handle)?;

    // Store pipeline
    {
        let mut pipeline_guard = state
            .pipeline
            .lock()
            .map_err(|_| "pipeline lock poisoned".to_string())?;
        *pipeline_guard = Some(pipeline);
    }

    Ok(())
}

#[tauri::command]
fn pause_recording(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut recorder_state = state
        .recorder_state
        .lock()
        .map_err(|_| "recorder state lock poisoned".to_string())?;
    recorder_state
        .pause()
        .map_err(|error| format!("{error:?}"))?;

    // Stop pipeline but keep session
    if let Ok(mut pipeline_guard) = state.pipeline.lock() {
        if let Some(ref mut pipeline) = *pipeline_guard {
            pipeline.stop();
        }
    }

    Ok(())
}

#[tauri::command]
async fn resume_recording(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
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
        .map_err(|error| format!("{error:?}"))?;

    // Restart pipeline
    if let Ok(mut pipeline_guard) = state.pipeline.lock() {
        if let Some(ref mut pipeline) = *pipeline_guard {
            pipeline.start(app_handle)?;
        }
    }

    Ok(())
}

#[tauri::command]
fn stop_recording(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut recorder_state = state
        .recorder_state
        .lock()
        .map_err(|_| "recorder state lock poisoned".to_string())?;
    recorder_state
        .stop()
        .map_err(|error| format!("{error:?}"))?;

    // Stop pipeline
    if let Ok(mut pipeline_guard) = state.pipeline.lock() {
        if let Some(ref mut pipeline) = *pipeline_guard {
            pipeline.stop();
        }
    }

    Ok(())
}

#[tauri::command]
fn get_steps(state: tauri::State<'_, AppState>) -> Result<Vec<Step>, String> {
    let pipeline_guard = state
        .pipeline
        .lock()
        .map_err(|_| "pipeline lock poisoned".to_string())?;

    if let Some(ref pipeline) = *pipeline_guard {
        let session = pipeline
            .get_session()
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        Ok(session.get_steps().to_vec())
    } else {
        Ok(Vec::new())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
        .manage(AppState {
            recorder_state: Mutex::new(RecorderState::new()),
            pipeline: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            check_permissions,
            request_permissions,
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording,
            get_steps
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 2: Verify it compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: integrate capture pipeline with Tauri commands"
```

---

## Task 10: Frontend Step Types

**Files:**
- Create: `src/types/step.ts`

**Step 1: Create TypeScript types matching Rust types**

Create `src/types/step.ts`:

```typescript
export type ActionType = "Click" | "Shortcut" | "Note";

export interface Step {
  id: string;
  ts: number;
  action: ActionType;
  x: number;
  y: number;
  click_x_percent: number;
  click_y_percent: number;
  app: string;
  window_title: string;
  screenshot_path: string | null;
  note: string | null;
}
```

**Step 2: Commit**

```bash
mkdir -p src/types
git add src/types/step.ts
git commit -m "feat: add TypeScript Step type"
```

---

## Task 11: StepItem Component

**Files:**
- Create: `src/components/StepItem.tsx`

**Step 1: Create the StepItem component**

Create `src/components/StepItem.tsx`:

```tsx
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Step } from "../types/step";

interface StepItemProps {
  step: Step;
  index: number;
}

export default function StepItem({ step, index }: StepItemProps) {
  const thumbnailSrc = step.screenshot_path
    ? convertFileSrc(step.screenshot_path)
    : null;

  return (
    <div className="step-item">
      <div className="step-thumb">
        {thumbnailSrc ? (
          <>
            <img src={thumbnailSrc} alt={`Step ${index + 1}`} />
            <div
              className="click-indicator"
              style={{
                left: `${step.click_x_percent}%`,
                top: `${step.click_y_percent}%`,
              }}
            />
          </>
        ) : (
          <div className="step-thumb-placeholder" />
        )}
      </div>
      <div className="step-content">
        <span className="step-number">Step {index + 1}</span>
        <span className="step-desc">
          {step.app}
          {step.window_title && ` - "${step.window_title}"`}
        </span>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add src/components/StepItem.tsx
git commit -m "feat: add StepItem component with thumbnail and click indicator"
```

---

## Task 12: Update RecorderPanel to Display Steps

**Files:**
- Modify: `src/components/RecorderPanel.tsx`

**Step 1: Update RecorderPanel to listen for step events**

Update `src/components/RecorderPanel.tsx`:

```tsx
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { renderHtml, renderMarkdown } from "../export/render";
import type { Step } from "../types/step";
import StepItem from "./StepItem";

type PermissionStatus = {
  screen_recording: boolean;
  accessibility: boolean;
};

type RecorderStatus = "idle" | "recording" | "paused" | "stopped";

const STATUS_LABELS: Record<RecorderStatus, string> = {
  idle: "Ready",
  recording: "Recording",
  paused: "Paused",
  stopped: "Stopped",
};

const STATUS_TONES: Record<RecorderStatus, "quiet" | "active" | "warn"> = {
  idle: "quiet",
  recording: "active",
  paused: "warn",
  stopped: "quiet",
};

const COMMANDS = {
  start: "start_recording",
  pause: "pause_recording",
  resume: "resume_recording",
  stop: "stop_recording",
} as const;

type RecorderCommand = keyof typeof COMMANDS;

function downloadText(filename: string, contents: string, mime: string) {
  const blob = new Blob([contents], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

export default function RecorderPanel() {
  const [permissions, setPermissions] = useState<PermissionStatus | null>(null);
  const [status, setStatus] = useState<RecorderStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [title, setTitle] = useState("New StepCast Guide");
  const [steps, setSteps] = useState<Step[]>([]);

  const permissionsReady = Boolean(
    permissions && permissions.screen_recording && permissions.accessibility,
  );

  const refreshPermissions = useCallback(async () => {
    try {
      const next = await invoke<PermissionStatus>("check_permissions");
      setPermissions(next);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    refreshPermissions();
  }, [refreshPermissions]);

  // Listen for step-captured events
  useEffect(() => {
    const unlisten = listen<Step>("step-captured", (event) => {
      setSteps((prev) => [...prev, event.payload]);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const missingPermissions = useMemo(() => {
    if (!permissions) {
      return [] as string[];
    }
    const missing = [] as string[];
    if (!permissions.screen_recording) missing.push("Screen Recording");
    if (!permissions.accessibility) missing.push("Accessibility");
    return missing;
  }, [permissions]);

  const handleCommand = useCallback(
    async (command: RecorderCommand, nextStatus?: RecorderStatus) => {
      setError(null);
      try {
        await invoke(COMMANDS[command]);
        if (nextStatus) {
          setStatus(nextStatus);
        }
        // Clear steps when starting a new recording
        if (command === "start") {
          setSteps([]);
        }
      } catch (err) {
        const message = String(err);
        if (message.includes("missing screen recording")) {
          setError("Grant Screen Recording and Accessibility permissions to record.");
        } else {
          setError(message);
        }
      }
    },
    [],
  );

  const handleRequestPermissions = useCallback(async () => {
    setError(null);
    try {
      const next = await invoke<PermissionStatus>("request_permissions");
      setPermissions(next);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const handleExportHtml = useCallback(() => {
    const html = renderHtml(title, steps);
    downloadText("stepcast-guide.html", html, "text/html");
  }, [title, steps]);

  const handleExportMarkdown = useCallback(() => {
    const markdown = renderMarkdown(title, steps);
    downloadText("stepcast-guide.md", markdown, "text/markdown");
  }, [title, steps]);

  const handleExportPdf = useCallback(() => {
    const html = renderHtml(title, steps);
    const preview = window.open("", "_blank", "width=960,height=720");
    if (!preview) {
      setError("Popup blocked. Allow new windows to print.");
      return;
    }
    preview.document.open();
    preview.document.write(html);
    preview.document.close();
    preview.focus();
    setTimeout(() => {
      preview.print();
    }, 120);
  }, [title, steps]);

  return (
    <main className="panel">
      {/* Minimal Header */}
      <header className="panel-header">
        <h1 className="panel-title">StepCast</h1>
        <div className="status-chip" data-tone={STATUS_TONES[status]}>
          {STATUS_LABELS[status]}
        </div>
      </header>

      {/* Permissions - only show if missing */}
      {missingPermissions.length > 0 && (
        <section className="panel-card">
          <div className="permissions">
            <div className="permission-banner warn">
              Missing: {missingPermissions.join(", ")}
            </div>
            <div className="permission-row">
              <span>Screen Recording</span>
              <span className={permissions?.screen_recording ? "pill ok" : "pill warn"}>
                {permissions?.screen_recording ? "OK" : "Missing"}
              </span>
            </div>
            <div className="permission-row">
              <span>Accessibility</span>
              <span className={permissions?.accessibility ? "pill ok" : "pill warn"}>
                {permissions?.accessibility ? "OK" : "Missing"}
              </span>
            </div>
            <button className="button ghost" onClick={handleRequestPermissions}>
              Grant Permissions
            </button>
          </div>
        </section>
      )}

      {/* Controls & Steps */}
      <section className="panel-card" style={{ flex: 1, minHeight: 0 }}>
        {/* Context-dependent buttons */}
        <div className="controls">
          {(status === "idle" || status === "stopped") && (
            <button
              className="button primary"
              onClick={() => handleCommand("start", "recording")}
              disabled={!permissionsReady}
            >
              Start Recording
            </button>
          )}

          {status === "recording" && (
            <>
              <button
                className="button"
                onClick={() => handleCommand("pause", "paused")}
              >
                Pause
              </button>
              <button
                className="button danger"
                onClick={() => handleCommand("stop", "stopped")}
              >
                Stop
              </button>
            </>
          )}

          {status === "paused" && (
            <>
              <button
                className="button primary"
                onClick={() => handleCommand("resume", "recording")}
              >
                Resume
              </button>
              <button
                className="button danger"
                onClick={() => handleCommand("stop", "stopped")}
              >
                Stop
              </button>
            </>
          )}
        </div>

        {/* Steps List */}
        <div className="steps">
          <div className="steps-header">
            <h2>Steps</h2>
            <span className="muted">{steps.length} captured</span>
          </div>
          {steps.length === 0 ? (
            <div className="steps-empty">
              Click anywhere to capture steps.
            </div>
          ) : (
            <div className="steps-list">
              {steps.map((step, index) => (
                <StepItem key={step.id} step={step} index={index} />
              ))}
            </div>
          )}
        </div>
      </section>

      {/* Export with title input */}
      <section className="panel-card export-card">
        <h2>Export</h2>
        <input
          className="title-input"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Guide title..."
        />
        <div className="export-actions">
          <button className="button" onClick={handleExportHtml} disabled={steps.length === 0}>
            HTML
          </button>
          <button className="button" onClick={handleExportMarkdown} disabled={steps.length === 0}>
            MD
          </button>
          <button className="button primary" onClick={handleExportPdf} disabled={steps.length === 0}>
            PDF
          </button>
        </div>
      </section>

      {error && <div className="error-banner">{error}</div>}
    </main>
  );
}
```

**Step 2: Commit**

```bash
git add src/components/RecorderPanel.tsx
git commit -m "feat: integrate step event listening and display in RecorderPanel"
```

---

## Task 13: Add Click Indicator CSS

**Files:**
- Modify: `src/App.css`

**Step 1: Add click indicator styles**

Add to `src/App.css`:

```css
/* Click Indicator */
.step-thumb {
  position: relative;
}

.click-indicator {
  position: absolute;
  width: 12px;
  height: 12px;
  background: rgba(255, 59, 48, 0.9);
  border: 2px solid white;
  border-radius: 50%;
  transform: translate(-50%, -50%);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
  pointer-events: none;
}

.step-thumb-placeholder {
  width: 100%;
  height: 100%;
  background: var(--bg-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
}
```

**Step 2: Commit**

```bash
git add src/App.css
git commit -m "feat: add click indicator styles for step thumbnails"
```

---

## Task 14: Update Export Templates

**Files:**
- Modify: `src/export/render.ts`

**Step 1: Update render functions to accept steps**

Update `src/export/render.ts`:

```typescript
import type { Step } from "../types/step";
import { convertFileSrc } from "@tauri-apps/api/core";

export function renderHtml(title: string, steps: Step[] = []): string {
  const stepsHtml = steps
    .map(
      (step, index) => `
    <article class="step">
      <div class="step-header">
        <span class="step-number">Step ${index + 1}</span>
        <span class="step-app">${escapeHtml(step.app)}${step.window_title ? ` - "${escapeHtml(step.window_title)}"` : ""}</span>
      </div>
      <div class="step-image">
        ${step.screenshot_path ? `<img src="${convertFileSrc(step.screenshot_path)}" alt="Step ${index + 1}">` : ""}
        <div class="click-marker" style="left: ${step.click_x_percent}%; top: ${step.click_y_percent}%"></div>
      </div>
    </article>
  `
    )
    .join("\n");

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${escapeHtml(title)}</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
      background: #f5f5f7;
      color: #1d1d1f;
      padding: 40px;
      max-width: 900px;
      margin: 0 auto;
    }
    h1 { font-size: 28px; margin-bottom: 8px; }
    .meta { color: #86868b; font-size: 14px; margin-bottom: 32px; }
    .step {
      background: white;
      border-radius: 12px;
      padding: 20px;
      margin-bottom: 20px;
      box-shadow: 0 2px 8px rgba(0,0,0,0.08);
    }
    .step-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 16px;
    }
    .step-number {
      background: #007aff;
      color: white;
      padding: 4px 12px;
      border-radius: 999px;
      font-size: 13px;
      font-weight: 600;
    }
    .step-app { color: #86868b; font-size: 14px; }
    .step-image {
      position: relative;
      border-radius: 8px;
      overflow: hidden;
      background: #e8e8ed;
    }
    .step-image img {
      width: 100%;
      display: block;
    }
    .click-marker {
      position: absolute;
      width: 20px;
      height: 20px;
      background: rgba(255, 59, 48, 0.9);
      border: 3px solid white;
      border-radius: 50%;
      transform: translate(-50%, -50%);
      box-shadow: 0 2px 8px rgba(0,0,0,0.3);
    }
    @media print {
      body { padding: 20px; }
      .step { break-inside: avoid; }
    }
  </style>
</head>
<body>
  <h1>${escapeHtml(title)}</h1>
  <p class="meta">Generated with StepCast • ${steps.length} steps</p>
  ${stepsHtml}
</body>
</html>`;
}

export function renderMarkdown(title: string, steps: Step[] = []): string {
  const stepsMd = steps
    .map(
      (step, index) => `
## Step ${index + 1}

**${escapeHtml(step.app)}**${step.window_title ? ` - "${escapeHtml(step.window_title)}"` : ""}

${step.screenshot_path ? `![Step ${index + 1}](${step.screenshot_path})` : ""}
`
    )
    .join("\n---\n");

  return `# ${title}

Generated with StepCast • ${steps.length} steps

---
${stepsMd}`;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
```

**Step 2: Commit**

```bash
git add src/export/render.ts
git commit -m "feat: update export templates to include actual steps with screenshots"
```

---

## Task 15: Final Integration Test

**Step 1: Build and verify**

Run:
```bash
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: Compiles without errors

**Step 2: Run the app**

Run:
```bash
bun tauri dev
```

**Step 3: Manual test**

1. Click "Start Recording"
2. Click in various applications
3. Verify steps appear in the list with thumbnails
4. Click "Stop"
5. Export to HTML and verify output

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete core recording functionality"
```

---

## Success Criteria Checklist

- [ ] User can start/stop recording
- [ ] Clicks are captured with window screenshots
- [ ] Steps appear in UI in real-time with thumbnails
- [ ] Click position is visible on screenshots (red dot)
- [ ] HTML export generates working standalone file
- [ ] Markdown export generates valid markdown
