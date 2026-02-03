# Core Recording Functionality

## Overview

Implement the core recording pipeline: capture mouse clicks, take screenshots of the active window, and display steps in real-time in the UI.

## Goals

- **Primary use cases:** Documentation and bug reports
- **MVP scope:** Automatic click recording, window screenshots with click highlight, HTML/MD export

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Frontend (React)                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Controls  │  │  StepsList  │  │   Export Panel      │  │
│  │  Start/Stop │  │  Thumbnails │  │   HTML/MD/PDF       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│         │               ▲                    │               │
│         │    listen     │                    │               │
│         │  "step-captured"                   │               │
└─────────┼───────────────┼────────────────────┼───────────────┘
          │ invoke        │ emit               │ invoke
          ▼               │                    ▼
┌─────────────────────────────────────────────────────────────┐
│                      Backend (Rust/Tauri)                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Recorder  │  │   Capture   │  │      Export         │  │
│  │  State Mgmt │  │  Pipeline   │  │   HTML/MD Gen       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│         │               │                                    │
│         ▼               ▼                                    │
│  ┌─────────────┐  ┌─────────────┐                           │
│  │ Click       │  │ Screenshot  │                           │
│  │ Listener    │  │ Capture     │                           │
│  │ (CGEventTap)│  │ (CGWindow)  │                           │
│  └─────────────┘  └─────────────┘                           │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Tasks

### Phase 1: Click Listener

**Goal:** Detect mouse clicks globally while recording.

**Files:**
- `src-tauri/src/recorder/click_listener.rs` (new)

**Approach:**
- Use `CGEventTap` (Core Graphics) for global mouse event monitoring
- Requires Accessibility permission (already requested)
- Run in background thread, send events via channel to main thread
- Passive tap only (observe, don't modify events)

**API:**
```rust
pub struct ClickListener {
    running: Arc<AtomicBool>,
}

impl ClickListener {
    pub fn start(callback: impl Fn(ClickEvent) + Send + 'static) -> Self;
    pub fn stop(&self);
}

pub struct ClickEvent {
    pub x: i32,
    pub y: i32,
    pub timestamp: i64,
    pub button: MouseButton,
}
```

### Phase 2: Window Detection

**Goal:** Get active window info when click occurs.

**Files:**
- `src-tauri/src/recorder/window_info.rs` (new)

**Approach:**
- `NSWorkspace.shared.frontmostApplication` for app name/bundle ID
- `CGWindowListCopyWindowInfo` for window list
- Accessibility API (`AXUIElement`) for window title

**API:**
```rust
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub window_id: u32,
    pub bounds: Rect,  // x, y, width, height
}

pub fn get_frontmost_window() -> Result<WindowInfo, WindowError>;
```

### Phase 3: Screenshot Capture

**Goal:** Capture screenshot of active window with click position.

**Files:**
- `src-tauri/src/recorder/macos_screencapture.rs` (update existing)

**Approach:**
- `CGWindowListCreateImage` with specific window ID
- Save as PNG to temp directory
- Store click position relative to window bounds (for overlay later)

**API:**
```rust
pub fn capture_window(window_id: u32, output_path: &Path) -> Result<(), CaptureError>;
```

**Temp Directory:**
- `~/Library/Caches/com.markus.tauri-app/sessions/{session-id}/`
- One folder per recording session
- Clean up old sessions on app start (> 7 days)

### Phase 4: Capture Pipeline

**Goal:** Connect click → window info → screenshot → step creation.

**Files:**
- `src-tauri/src/recorder/pipeline.rs` (new)
- `src-tauri/src/recorder/session.rs` (new)

**Flow:**
1. Click detected → `ClickEvent`
2. Get window info → `WindowInfo`
3. Capture screenshot → `screenshot_path`
4. Create `Step` with all data
5. Emit `step-captured` event to frontend
6. Store step in session state

**Session Management:**
```rust
pub struct Session {
    pub id: String,
    pub started_at: i64,
    pub steps: Vec<Step>,
    pub temp_dir: PathBuf,
}

impl Session {
    pub fn new() -> Self;
    pub fn add_step(&mut self, step: Step);
    pub fn get_steps(&self) -> &[Step];
}
```

### Phase 5: Frontend Integration

**Goal:** Display captured steps in real-time.

**Files:**
- `src/components/RecorderPanel.tsx` (update)
- `src/components/StepItem.tsx` (new)

**Changes:**
1. Listen to `step-captured` events
2. Maintain steps array in state
3. Render `StepItem` for each step
4. Show thumbnail with click indicator overlay

**StepItem Component:**
```tsx
type StepItemProps = {
  step: Step;
  index: number;
};

function StepItem({ step, index }: StepItemProps) {
  const thumbnailSrc = convertFileSrc(step.screenshot_path);
  const clickX = /* calculate relative position */;
  const clickY = /* calculate relative position */;

  return (
    <div className="step-item">
      <div className="step-thumb">
        <img src={thumbnailSrc} alt="" />
        <div
          className="click-indicator"
          style={{ left: `${clickX}%`, top: `${clickY}%` }}
        />
      </div>
      <div className="step-content">
        <span className="step-number">Step {index + 1}</span>
        <span className="step-desc">
          Clicked in {step.app} - "{step.window_title}"
        </span>
      </div>
    </div>
  );
}
```

### Phase 6: HTML Export

**Goal:** Generate standalone HTML file with embedded images.

**Files:**
- `src-tauri/src/export/html.rs` (new)
- Move export logic from frontend to backend (for file system access)

**Approach:**
- Read all screenshots, convert to Base64
- Generate HTML with inline styles
- Click markers as positioned overlays
- Single file output, no external dependencies

**Template Structure:**
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{{title}}</title>
  <style>
    /* Embedded styles */
  </style>
</head>
<body>
  <header>
    <h1>{{title}}</h1>
    <p>Recorded on {{date}} • {{step_count}} steps</p>
  </header>
  <main>
    {{#each steps}}
    <article class="step">
      <div class="step-header">
        <span class="step-number">{{@index + 1}}</span>
        <span class="step-app">{{app}} - "{{window_title}}"</span>
      </div>
      <div class="step-image">
        <img src="data:image/png;base64,{{screenshot_base64}}" alt="Step {{@index + 1}}">
        <div class="click-marker" style="left: {{click_x}}%; top: {{click_y}}%"></div>
      </div>
    </article>
    {{/each}}
  </main>
</body>
</html>
```

### Phase 7: Markdown Export

**Goal:** Generate Markdown for wikis/docs.

**Files:**
- `src-tauri/src/export/markdown.rs` (new)

**Output:**
```markdown
# {{title}}

Recorded on {{date}} • {{step_count}} steps

---

## Step 1

![Step 1](data:image/png;base64,...)

**Action:** Clicked in {{app}} - "{{window_title}}"

---

## Step 2
...
```

Note: Base64 images in Markdown work in most renderers (GitHub, Notion) but files are large. Future option: upload images to CDN and use URLs.

## Data Flow

```
User clicks "Start Recording"
         │
         ▼
    ┌─────────────┐
    │ start_recording() │
    │ - Create Session   │
    │ - Start ClickListener │
    └─────────────┘
         │
         ▼
    CGEventTap running in background
         │
    User clicks somewhere
         │
         ▼
    ┌─────────────┐
    │ ClickEvent  │
    │ x, y, ts    │
    └─────────────┘
         │
         ▼
    ┌─────────────┐
    │ get_frontmost_window() │
    └─────────────┘
         │
         ▼
    ┌─────────────┐
    │ capture_window() │
    │ → PNG in temp dir │
    └─────────────┘
         │
         ▼
    ┌─────────────┐
    │ Create Step │
    │ Add to Session │
    └─────────────┘
         │
         ▼
    ┌─────────────┐
    │ emit("step-captured", step) │
    └─────────────┘
         │
         ▼
    Frontend receives event
    UI updates with new step
```

## File Structure (New/Modified)

```
src-tauri/src/
├── recorder/
│   ├── mod.rs              (update)
│   ├── click_listener.rs   (new)
│   ├── window_info.rs      (new)
│   ├── pipeline.rs         (new)
│   ├── session.rs          (new)
│   ├── capture.rs          (existing)
│   ├── macos_screencapture.rs (update)
│   ├── storage.rs          (existing)
│   └── types.rs            (update)
├── export/
│   ├── mod.rs              (new)
│   ├── html.rs             (new)
│   └── markdown.rs         (new)
└── lib.rs                  (update commands)

src/
├── components/
│   ├── RecorderPanel.tsx   (update)
│   └── StepItem.tsx        (new)
└── types/
    └── step.ts             (new - TypeScript types)
```

## Dependencies

**Rust (Cargo.toml):**
```toml
[dependencies]
core-graphics = "0.23"      # CGEventTap, CGWindow
core-foundation = "0.9"     # CF types
cocoa = "0.25"              # NSWorkspace
base64 = "0.21"             # Image encoding for export
handlebars = "4.0"          # HTML templating (optional)
```

## Out of Scope (Future)

- Intelligent click filtering (Issue #1)
- Manual hotkey for note steps (Issue #2)
- AI-generated step descriptions
- Image upload to CDN for smaller exports
- Tray icon status indicator (red when recording)
- Edit/delete steps in UI
- Reorder steps via drag & drop

## Implementation Order

1. **Click Listener** – foundation for everything
2. **Window Detection** – needed for meaningful screenshots
3. **Screenshot Capture** – visual output
4. **Capture Pipeline** – connect the pieces
5. **Frontend Integration** – show results to user
6. **HTML Export** – first usable output
7. **Markdown Export** – second format

## Success Criteria

MVP is complete when:
- [ ] User can start/stop recording
- [ ] Clicks are captured with window screenshots
- [ ] Steps appear in UI in real-time with thumbnails
- [ ] Click position is visible on screenshots
- [ ] HTML export generates working standalone file
- [ ] Markdown export generates valid markdown
