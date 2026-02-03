# Task 7 Code Quality Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve tray icon path resolution, prevent tray panel jump, and make monitor handling non-panicking with sensible fallbacks.

**Architecture:** Add a tray icon path resolver that tries multiple candidate locations and surfaces a clear error when missing. Reorder tray click flow to compute position before showing. Add a monitor resolution helper that tries current, primary, then first available monitor; return explicit error if none. Use helper in panel positioning to avoid panic paths.

**Tech Stack:** Rust, Tauri 2, tauri-nspanel.

---

### Task 1: Tray icon path resolution fallback + explicit failure

**Files:**
- Modify: `src-tauri/src/tray.rs`

**Step 1: (Optional) Write the failing test**
No reliable unit test for Tauri `AppHandle::path()` or bundled resources. Skip.

**Step 2: Implement minimal change**

```rust
use std::path::PathBuf;

fn resolve_tray_icon_path(app_handle: &AppHandle) -> tauri::Result<PathBuf> {
    let candidates = [
        (BaseDirectory::Resource, "icons/icon.png"),
        (BaseDirectory::Resource, "icon.png"),
        (BaseDirectory::App, "icons/icon.png"),
        (BaseDirectory::App, "icon.png"),
    ];

    for (base, rel) in candidates {
        if let Ok(path) = app_handle.path().resolve(rel, base) {
            if path.is_file() {
                return Ok(path);
            }
        }
    }

    Err(tauri::Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "tray icon missing; tried Resource/App icon paths",
    )))
}
```

Use `resolve_tray_icon_path(app_handle)?` in `create` and keep error propagation explicit.

**Step 3: Manual check**
Run the app. Tray icon should appear. If not, error surfaced via returned `tauri::Result`.

---

### Task 2: Position panel before show

**Files:**
- Modify: `src-tauri/src/tray.rs`

**Step 1: Implement reorder**

```rust
if let Err(err) = position_panel_at_tray_icon(app_handle, rect.position, rect.size) {
    eprintln!("Failed to position panel: {}", err);
}

panel.show();
```

Place this before any `panel.show()` call to avoid a visible jump.

---

### Task 3: Monitor fallback handling (no panic)

**Files:**
- Modify: `src-tauri/src/panel.rs`

**Step 1: (Optional) Write the failing test**
No deterministic monitor API test in unit tests. Skip.

**Step 2: Implement helper + use it**

```rust
fn resolve_monitor(window: &tauri::Window) -> Result<tauri::Monitor, String> {
    if let Ok(Some(monitor)) = window.current_monitor() {
        return Ok(monitor);
    }

    if let Ok(Some(monitor)) = window.primary_monitor() {
        return Ok(monitor);
    }

    if let Ok(monitors) = window.available_monitors() {
        if let Some(monitor) = monitors.into_iter().next() {
            return Ok(monitor);
        }
    }

    Err("no monitor available".to_string())
}
```

Use in `position_panel_at_tray_icon`:

```rust
let window = panel
    .to_window()
    .ok_or_else(|| "panel window missing".to_string())?;

let _monitor = resolve_monitor(&window)?;
```

This ensures we never panic on missing monitor data and have explicit errors with fallbacks.

**Step 3: Manual check**
Open/close the panel; no panic when monitors are disconnected.
