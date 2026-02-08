# Export Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract export logic into a dedicated module, improve output quality (dark mode, notes, shortcuts), add real PDF via wkhtmltopdf, and use image folders for Markdown exports.

**Architecture:** New `src-tauri/src/export/` module with shared helpers and per-format generators. Single `export_guide` Tauri command replaces 3 existing commands. Frontend simplified to one unified export flow. `slug` crate for filename sanitization.

**Tech Stack:** Rust (slug, std::process::Command for wkhtmltopdf), Tauri 2, React 19, TypeScript

---

### Task 1: Add `slug` dependency

**Files:**
- Modify: `src-tauri/Cargo.toml:38` (add before `[dev-dependencies]`)

**Step 1: Add slug to Cargo.toml**

Add this line in `[dependencies]`:
```toml
slug = "0.1"
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: add slug crate for filename sanitization"
```

---

### Task 2: Create `export/helpers.rs` — shared utilities

**Files:**
- Create: `src-tauri/src/export/helpers.rs`

**Step 1: Write tests for helpers**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::types::{ActionType, Step};

    fn sample_step() -> Step {
        Step {
            id: "s1".into(),
            ts: 0,
            action: ActionType::Click,
            x: 10, y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".into(),
            window_title: "Downloads".into(),
            screenshot_path: None,
            note: None,
        }
    }

    #[test]
    fn action_description_click() {
        let s = sample_step();
        assert_eq!(action_description(&s), "Clicked in Finder — \"Downloads\"");
    }

    #[test]
    fn action_description_double_click() {
        let mut s = sample_step();
        s.action = ActionType::DoubleClick;
        assert_eq!(action_description(&s), "Double-clicked in Finder — \"Downloads\"");
    }

    #[test]
    fn action_description_right_click() {
        let mut s = sample_step();
        s.action = ActionType::RightClick;
        assert_eq!(action_description(&s), "Right-clicked in Finder — \"Downloads\"");
    }

    #[test]
    fn action_description_shortcut() {
        let mut s = sample_step();
        s.action = ActionType::Shortcut;
        assert_eq!(action_description(&s), "Used keyboard shortcut in Finder — \"Downloads\"");
    }

    #[test]
    fn action_description_note() {
        let mut s = sample_step();
        s.action = ActionType::Note;
        s.note = Some("Remember to save".into());
        assert_eq!(action_description(&s), "Note");
    }

    #[test]
    fn action_description_auth_placeholder() {
        let mut s = sample_step();
        s.window_title = "Authentication dialog (secure)".into();
        assert_eq!(action_description(&s), "Authentication required (secure dialog)");
    }

    #[test]
    fn action_description_auth_by_app() {
        let mut s = sample_step();
        s.app = "Authentication".into();
        assert_eq!(action_description(&s), "Authentication required (secure dialog)");
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn html_escape_quotes() {
        assert_eq!(html_escape(r#"say "hello""#), "say &quot;hello&quot;");
    }

    #[test]
    fn slugify_title_basic() {
        assert_eq!(slugify_title("My Guide Title"), "my-guide-title");
    }

    #[test]
    fn slugify_title_umlauts() {
        let result = slugify_title("Ärger mit Ölförderung");
        // slug/deunicode transliterates ä→a, ö→o (phonetic, not German)
        assert!(!result.contains('ä'));
        assert!(!result.contains('ö'));
        assert!(result.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn slugify_title_special_chars() {
        assert_eq!(slugify_title("Hello World! (2026)"), "hello-world-2026");
    }

    #[test]
    fn is_auth_placeholder_checks() {
        let mut s = sample_step();
        assert!(!is_auth_placeholder(&s));

        s.window_title = "Authentication dialog (secure)".into();
        assert!(is_auth_placeholder(&s));

        s.window_title = "Normal".into();
        s.app = "Authentication".into();
        assert!(is_auth_placeholder(&s));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test export::helpers::tests -- --nocapture`
Expected: compilation error (module doesn't exist yet)

**Step 3: Implement helpers**

```rust
use crate::recorder::types::{ActionType, Step};
use base64::Engine;
use std::fs;
use std::io::Read as _;

/// Check if a step represents an authentication placeholder
pub fn is_auth_placeholder(step: &Step) -> bool {
    step.window_title == "Authentication dialog (secure)"
        || step.app.to_lowercase() == "authentication"
}

/// Human-readable description of what happened in a step
pub fn action_description(step: &Step) -> String {
    if is_auth_placeholder(step) {
        return "Authentication required (secure dialog)".to_string();
    }

    match step.action {
        ActionType::Note => "Note".to_string(),
        _ => {
            let verb = match step.action {
                ActionType::DoubleClick => "Double-clicked in",
                ActionType::RightClick => "Right-clicked in",
                ActionType::Shortcut => "Used keyboard shortcut in",
                _ => "Clicked in",
            };
            format!("{} {} — \"{}\"", verb, step.app, step.window_title)
        }
    }
}

/// Load a screenshot file and return its base64-encoded content
pub fn load_screenshot_base64(path: &str) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(&buffer))
}

/// Escape HTML special characters
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Convert a title to a filesystem-safe slug
pub fn slugify_title(title: &str) -> String {
    slug::slugify(title)
}
```

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test export::helpers::tests -- --nocapture`
Expected: all tests pass

**Step 5: Commit**

```bash
git add src-tauri/src/export/helpers.rs
git commit -m "feat(export): add shared helpers — action descriptions, html escape, slugify"
```

---

### Task 3: Create `export/html.rs` — HTML generator with dark mode + notes

**Files:**
- Create: `src-tauri/src/export/html.rs`

**Step 1: Write test for HTML generation**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::types::{ActionType, Step};

    fn sample_step() -> Step {
        Step {
            id: "s1".into(),
            ts: 0,
            action: ActionType::Click,
            x: 10, y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".into(),
            window_title: "Downloads".into(),
            screenshot_path: None,
            note: None,
        }
    }

    #[test]
    fn generate_contains_title() {
        let html = generate("Test Guide", &[sample_step()]);
        assert!(html.contains("<title>Test Guide</title>"));
        assert!(html.contains("<h1>Test Guide</h1>"));
    }

    #[test]
    fn generate_contains_step_count() {
        let html = generate("G", &[sample_step(), sample_step()]);
        assert!(html.contains("2 steps"));
    }

    #[test]
    fn generate_contains_dark_mode() {
        let html = generate("G", &[sample_step()]);
        assert!(html.contains("prefers-color-scheme: dark"));
    }

    #[test]
    fn generate_contains_step_article() {
        let html = generate("G", &[sample_step()]);
        assert!(html.contains("Step 1"));
        assert!(html.contains("Clicked in Finder"));
    }

    #[test]
    fn generate_includes_note_when_present() {
        let mut s = sample_step();
        s.note = Some("Don't forget this!".into());
        let html = generate("G", &[s]);
        assert!(html.contains("step-note"));
        assert!(html.contains("Don&#x27;t forget this!"));
    }

    #[test]
    fn generate_no_note_div_when_absent() {
        let html = generate("G", &[sample_step()]);
        assert!(!html.contains("step-note"));
    }

    #[test]
    fn generate_shortcut_action_text() {
        let mut s = sample_step();
        s.action = ActionType::Shortcut;
        let html = generate("G", &[s]);
        assert!(html.contains("Used keyboard shortcut in"));
    }

    #[test]
    fn generate_click_marker_classes() {
        let mut dc = sample_step();
        dc.action = ActionType::DoubleClick;
        let html = generate("G", &[dc]);
        assert!(html.contains("double-click"));

        let mut rc = sample_step();
        rc.action = ActionType::RightClick;
        let html = generate("G", &[rc]);
        assert!(html.contains("right-click"));
    }

    #[test]
    fn html_escape_in_title() {
        let html = generate("<script>alert(1)</script>", &[]);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test export::html::tests -- --nocapture`
Expected: compilation error

**Step 3: Implement HTML generator**

The `generate` function produces a self-contained HTML document. Key improvements over current code:
- `@media (prefers-color-scheme: dark)` CSS block
- `step.note` rendered as `<p class="step-note">` when present
- `ActionType::Shortcut` gets proper text
- `ActionType::Note` gets "Note" label
- Quote escaping in `html_escape` (adds `"` → `&quot;`)
- Note text escaped with a helper that also handles `'` → `&#x27;`

```rust
use crate::recorder::types::{ActionType, Step};
use super::helpers::{action_description, html_escape, is_auth_placeholder, load_screenshot_base64};

/// Generate a self-contained HTML document from steps.
pub fn generate(title: &str, steps: &[Step]) -> String {
    let steps_html: String = steps
        .iter()
        .enumerate()
        .map(|(i, step)| render_step(i + 1, step))
        .collect();

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title_esc}</title>
<style>
{css}
</style>
</head>
<body>
<h1>{title_esc}</h1>
<p class="meta">{count} step{plural}</p>
{steps_html}
</body>
</html>"#,
        title_esc = html_escape(title),
        css = CSS,
        count = steps.len(),
        plural = if steps.len() == 1 { "" } else { "s" },
        steps_html = steps_html,
    )
}

fn render_step(num: usize, step: &Step) -> String {
    let desc = html_escape(&action_description(step));

    let image_html = step.screenshot_path.as_ref()
        .and_then(|p| load_screenshot_base64(p))
        .map(|b64| format!(r#"<img src="data:image/png;base64,{b64}" alt="Step {num}">"#))
        .unwrap_or_default();

    let marker_class = match step.action {
        ActionType::DoubleClick => "click-marker double-click",
        ActionType::RightClick => "click-marker right-click",
        _ => "click-marker",
    };

    let click_marker = if step.screenshot_path.is_some() && !is_auth_placeholder(step) {
        format!(
            r#"<div class="{}" style="left: {}%; top: {}%;"></div>"#,
            marker_class, step.click_x_percent, step.click_y_percent
        )
    } else {
        String::new()
    };

    let note_html = step.note.as_ref()
        .map(|n| format!(r#"<p class="step-note">{}</p>"#, escape_text(n)))
        .unwrap_or_default();

    format!(
        r#"
    <article class="step">
      <div class="step-header">
        <span class="step-number">Step {num}</span>
        <span class="step-app">{desc}</span>
      </div>
      <div class="step-image">
        <div class="image-wrapper">
          {image_html}
          {click_marker}
        </div>
      </div>
      {note_html}
    </article>"#
    )
}

/// Escape text content (includes single quotes for note text)
fn escape_text(s: &str) -> String {
    html_escape(s).replace('\'', "&#x27;")
}

const CSS: &str = r#"* { box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; max-width: 800px; margin: 0 auto; padding: 40px 20px; color: #1d1d1f; background: #fff; line-height: 1.5; }
h1 { margin-bottom: 8px; }
.meta { color: #86868b; font-size: 14px; margin-bottom: 32px; }
.step { margin-bottom: 32px; border: 1px solid #e8e8ed; border-radius: 12px; overflow: hidden; }
.step-header { padding: 12px 16px; background: #f5f5f7; border-bottom: 1px solid #e8e8ed; display: flex; gap: 12px; align-items: center; }
.step-number { font-weight: 600; font-size: 12px; text-transform: uppercase; color: #86868b; white-space: nowrap; }
.step-app { color: #1d1d1f; }
.step-image { background: #f5f5f7; display: flex; align-items: center; justify-content: center; padding: 16px; }
.step-image img { display: block; max-width: 100%; height: auto; }
.image-wrapper { position: relative; display: inline-block; max-width: 100%; }
.step-note { margin: 0; padding: 10px 16px; font-size: 14px; color: #1d1d1f; background: #fef9e7; border-top: 1px solid #e8e8ed; }
.click-marker { position: absolute; width: 24px; height: 24px; border-radius: 50%; background: transparent; border: 2.5px solid #ff3b30; box-shadow: 0 0 0 1.5px rgba(255,255,255,0.9), 0 2px 6px rgba(0,0,0,0.25); transform: translate(-50%, -50%); pointer-events: none; }
.click-marker.double-click { width: 18px; height: 18px; border-width: 2px; }
.click-marker.double-click::after { content: ''; position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); width: 30px; height: 30px; border-radius: 50%; border: 2px solid #ff3b30; box-shadow: 0 0 0 1.5px rgba(255,255,255,0.9); pointer-events: none; }
.click-marker.right-click { border-style: dashed; }
@media print { .step { break-inside: avoid; } }
@media (prefers-color-scheme: dark) {
  body { background: #1d1d1f; color: #f5f5f7; }
  .step { border-color: #38383d; }
  .step-header { background: #2c2c2e; border-color: #38383d; }
  .step-app { color: #f5f5f7; }
  .step-image { background: #2c2c2e; }
  .step-note { background: #3a3520; color: #f5f5f7; border-color: #38383d; }
}"#;
```

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test export::html::tests -- --nocapture`
Expected: all pass

**Step 5: Commit**

```bash
git add src-tauri/src/export/html.rs
git commit -m "feat(export): HTML generator with dark mode, notes, and shortcut support"
```

---

### Task 4: Create `export/markdown.rs` — Markdown with image folder

**Files:**
- Create: `src-tauri/src/export/markdown.rs`

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::types::{ActionType, Step};
    use std::path::Path;

    fn sample_step() -> Step {
        Step {
            id: "s1".into(),
            ts: 0,
            action: ActionType::Click,
            x: 10, y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".into(),
            window_title: "Downloads".into(),
            screenshot_path: None,
            note: None,
        }
    }

    #[test]
    fn generate_contains_title() {
        let md = generate_content("Test Guide", &[sample_step()], "test-guide-images");
        assert!(md.starts_with("# Test Guide\n"));
    }

    #[test]
    fn generate_contains_step_count() {
        let md = generate_content("G", &[sample_step(), sample_step()], "g-images");
        assert!(md.contains("2 steps"));
    }

    #[test]
    fn generate_contains_action_desc() {
        let md = generate_content("G", &[sample_step()], "g-images");
        assert!(md.contains("Clicked in Finder"));
    }

    #[test]
    fn generate_shortcut_text() {
        let mut s = sample_step();
        s.action = ActionType::Shortcut;
        let md = generate_content("G", &[s], "g-images");
        assert!(md.contains("Used keyboard shortcut in"));
    }

    #[test]
    fn generate_includes_note() {
        let mut s = sample_step();
        s.note = Some("Important!".into());
        let md = generate_content("G", &[s], "g-images");
        assert!(md.contains("> Important!"));
    }

    #[test]
    fn generate_image_references_folder() {
        let mut s = sample_step();
        s.screenshot_path = Some("/tmp/fake.png".into());
        // Won't find the file, so no image
        let md = generate_content("G", &[s], "my-guide-images");
        // When screenshot can't be loaded, image line is omitted
        assert!(!md.contains("!["));
    }

    #[test]
    fn images_dir_name_from_output_path() {
        let p = Path::new("/Users/me/docs/My Guide.md");
        assert_eq!(images_dir_name(p), "My Guide-images");
    }

    #[test]
    fn images_dir_name_no_extension() {
        let p = Path::new("/Users/me/docs/readme");
        assert_eq!(images_dir_name(p), "readme-images");
    }
}
```

**Step 2: Run to verify failure**

Run: `cd src-tauri && cargo test export::markdown::tests -- --nocapture`
Expected: compilation error

**Step 3: Implement markdown generator**

```rust
use crate::recorder::types::Step;
use super::helpers::{action_description, load_screenshot_base64};
use std::fs;
use std::path::Path;

/// Derive the images directory name from the output .md file path.
/// "/path/to/My Guide.md" → "My Guide-images"
pub fn images_dir_name(output_path: &Path) -> String {
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");
    format!("{stem}-images")
}

/// Generate markdown content. `images_dir` is the relative folder name for images.
pub fn generate_content(title: &str, steps: &[Step], images_dir: &str) -> String {
    let mut md = format!(
        "# {title}\n\n{count} step{plural}\n\n---\n\n",
        count = steps.len(),
        plural = if steps.len() == 1 { "" } else { "s" },
    );

    for (i, step) in steps.iter().enumerate() {
        let num = i + 1;
        let desc = action_description(step);

        md.push_str(&format!("## Step {num}\n\n"));

        // Image reference (relative path into images dir)
        if step.screenshot_path.is_some() {
            md.push_str(&format!("![Step {num}](./{images_dir}/step-{num}.png)\n\n"));
        }

        md.push_str(&format!("**Action:** {desc}\n\n"));

        if let Some(note) = &step.note {
            md.push_str(&format!("> {note}\n\n"));
        }

        md.push_str("---\n\n");
    }

    md
}

/// Write the markdown file and copy screenshots into an adjacent images folder.
pub fn write(title: &str, steps: &[Step], output_path: &str) -> Result<(), String> {
    let path = Path::new(output_path);
    let parent = path.parent().ok_or("Invalid output path")?;
    let dir_name = images_dir_name(path);
    let images_dir = parent.join(&dir_name);

    // Create images directory
    if steps.iter().any(|s| s.screenshot_path.is_some()) {
        fs::create_dir_all(&images_dir)
            .map_err(|e| format!("Failed to create images directory: {e}"))?;
    }

    // Copy screenshots
    for (i, step) in steps.iter().enumerate() {
        if let Some(src) = &step.screenshot_path {
            let dest = images_dir.join(format!("step-{}.png", i + 1));
            fs::copy(src, &dest)
                .map_err(|e| format!("Failed to copy screenshot {}: {e}", i + 1))?;
        }
    }

    let content = generate_content(title, steps, &dir_name);
    fs::write(output_path, content)
        .map_err(|e| format!("Failed to write markdown file: {e}"))?;

    Ok(())
}
```

**Step 4: Run tests**

Run: `cd src-tauri && cargo test export::markdown::tests -- --nocapture`
Expected: all pass

**Step 5: Commit**

```bash
git add src-tauri/src/export/markdown.rs
git commit -m "feat(export): Markdown generator with image folder and notes"
```

---

### Task 5: Create `export/pdf.rs` — PDF via wkhtmltopdf

**Files:**
- Create: `src-tauri/src/export/pdf.rs`

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_wkhtmltopdf_returns_some_or_none() {
        // Just verify it doesn't panic — result depends on system
        let _ = find_wkhtmltopdf();
    }

    #[test]
    fn generate_delegates_to_html() {
        use crate::recorder::types::{ActionType, Step};
        let step = Step {
            id: "s1".into(), ts: 0, action: ActionType::Click,
            x: 10, y: 20, click_x_percent: 50.0, click_y_percent: 50.0,
            app: "Finder".into(), window_title: "Downloads".into(),
            screenshot_path: None, note: None,
        };
        // generate calls html::generate internally — verify it produces valid HTML
        let result = generate("Test", &[step]);
        assert!(result.contains("<!doctype html>"));
    }
}
```

**Step 2: Implement PDF export**

```rust
use crate::recorder::types::Step;
use std::fs;
use std::process::Command;

/// Generate HTML content (delegates to html module)
pub fn generate(title: &str, steps: &[Step]) -> String {
    super::html::generate(title, steps)
}

/// Check common paths for wkhtmltopdf
pub fn find_wkhtmltopdf() -> Option<String> {
    let candidates = [
        "/usr/local/bin/wkhtmltopdf",
        "/opt/homebrew/bin/wkhtmltopdf",
        "/usr/bin/wkhtmltopdf",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    // Try PATH
    Command::new("which")
        .arg("wkhtmltopdf")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Export steps as PDF. Generates temp HTML, converts via wkhtmltopdf, writes to output_path.
pub fn write(title: &str, steps: &[Step], output_path: &str) -> Result<(), String> {
    let wk = find_wkhtmltopdf().ok_or(
        "wkhtmltopdf not found. Install it with: brew install wkhtmltopdf"
    )?;

    let html = generate(title, steps);

    // Write temp HTML
    let cache_dir = dirs::cache_dir().ok_or("Could not find cache directory")?;
    let stepcast_dir = cache_dir.join("stepcast");
    fs::create_dir_all(&stepcast_dir)
        .map_err(|e| format!("Failed to create cache dir: {e}"))?;

    let temp_html = stepcast_dir.join(format!(
        "stepcast-pdf-{}.html",
        chrono::Utc::now().timestamp_millis()
    ));

    fs::write(&temp_html, &html)
        .map_err(|e| format!("Failed to write temp HTML: {e}"))?;

    // Convert to PDF
    let output = Command::new(&wk)
        .args([
            "--enable-local-file-access",
            "--quiet",
            &temp_html.to_string_lossy(),
            output_path,
        ])
        .output()
        .map_err(|e| format!("Failed to run wkhtmltopdf: {e}"))?;

    // Clean up temp file
    let _ = fs::remove_file(&temp_html);

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("wkhtmltopdf failed: {stderr}"))
    }
}
```

**Step 3: Run tests**

Run: `cd src-tauri && cargo test export::pdf::tests -- --nocapture`
Expected: pass

**Step 4: Commit**

```bash
git add src-tauri/src/export/pdf.rs
git commit -m "feat(export): PDF generation via wkhtmltopdf"
```

---

### Task 6: Create `export/mod.rs` — module root + unified export command

**Files:**
- Create: `src-tauri/src/export/mod.rs`

**Step 1: Write test for the unified export function**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_str_valid() {
        assert!(matches!(ExportFormat::from_str("html"), Ok(ExportFormat::Html)));
        assert!(matches!(ExportFormat::from_str("md"), Ok(ExportFormat::Markdown)));
        assert!(matches!(ExportFormat::from_str("pdf"), Ok(ExportFormat::Pdf)));
    }

    #[test]
    fn format_from_str_invalid() {
        assert!(ExportFormat::from_str("docx").is_err());
    }
}
```

**Step 2: Implement mod.rs**

```rust
pub mod helpers;
pub mod html;
pub mod markdown;
pub mod pdf;

use crate::recorder::types::Step;

#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Html,
    Markdown,
    Pdf,
}

impl ExportFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "html" => Ok(Self::Html),
            "md" => Ok(Self::Markdown),
            "pdf" => Ok(Self::Pdf),
            other => Err(format!("Unknown export format: {other}")),
        }
    }
}

/// Unified export: writes the given steps to output_path in the requested format.
pub fn export(title: &str, steps: &[Step], format: ExportFormat, output_path: &str) -> Result<(), String> {
    match format {
        ExportFormat::Html => {
            let content = html::generate(title, steps);
            std::fs::write(output_path, content)
                .map_err(|e| format!("Failed to write HTML: {e}"))
        }
        ExportFormat::Markdown => {
            markdown::write(title, steps, output_path)
        }
        ExportFormat::Pdf => {
            pdf::write(title, steps, output_path)
        }
    }
}
```

**Step 3: Run tests**

Run: `cd src-tauri && cargo test export::tests -- --nocapture`
Expected: pass

**Step 4: Commit**

```bash
git add src-tauri/src/export/mod.rs
git commit -m "feat(export): unified export module with format dispatch"
```

---

### Task 7: Wire up in `lib.rs` — replace 3 commands with 1

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: Add `mod export;` and remove old code**

In `lib.rs`:
- Add `mod export;` at the top (after `mod tray;` on line 4)
- Remove `use base64::Engine;` (line 16) — moved to helpers
- Remove `use std::io::Read;` (line 12) — moved to helpers
- Remove `load_screenshot_base64` fn (lines 421-426)
- Remove `generate_html` fn (lines 428-521)
- Remove `generate_markdown` fn (lines 523-556)
- Remove `html_escape` fn (lines 558-562)
- Remove `export_html` command (lines 564-578)
- Remove `export_markdown` command (lines 580-594)
- Remove `export_html_temp` command (lines 596-622)

**Step 2: Add the new unified command**

```rust
#[tauri::command]
fn export_guide(
    state: tauri::State<'_, RecorderAppState>,
    title: String,
    format: String,
    output_path: String,
) -> Result<(), String> {
    let fmt = export::ExportFormat::from_str(&format)?;
    let steps = {
        let session_lock = state.session.lock().map_err(|_| "session lock poisoned")?;
        session_lock.as_ref().map(|s| s.get_steps().to_vec()).unwrap_or_default()
    };
    export::export(&title, &steps, fmt, &output_path)
}
```

**Step 3: Update invoke_handler registration**

In `invoke_handler` (line 651-664), replace:
```rust
export_html,
export_markdown,
export_html_temp,
```
with:
```rust
export_guide,
```

**Step 4: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: compiles

**Step 5: Run all tests**

Run: `cd src-tauri && cargo test`
Expected: all pass (existing lib tests + new export tests)

**Step 6: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "refactor(export): replace 3 export commands with unified export_guide"
```

---

### Task 8: Update frontend — unified export handler

**Files:**
- Modify: `src/components/RecorderPanel.tsx:165-193`

**Step 1: Replace handleExport callback**

Replace the current `handleExport` (lines 165-193) with:

```typescript
const handleExport = useCallback(async (title: string, format: "html" | "md" | "pdf") => {
    setError(null);
    setExporting(true);
    try {
      const ext = { html: "html", md: "md", pdf: "pdf" }[format];
      const name = { html: "HTML", md: "Markdown", pdf: "PDF" }[format];
      const path = await save({
        defaultPath: `${title}.${ext}`,
        filters: [{ name, extensions: [ext] }],
      });
      if (!path) return;
      await invoke("export_guide", { title, format, outputPath: path });
      setShowExportSheet(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setExporting(false);
    }
  }, []);
```

**Step 2: Remove unused import**

If `openPath` from `@tauri-apps/plugin-opener` is no longer used anywhere else in the file, remove the import.

**Step 3: Verify TypeScript compiles**

Run: `cd /Users/markus/Daten/Development/Privat/psr_tool_mac && npm run build`
Expected: compiles

**Step 4: Commit**

```bash
git add src/components/RecorderPanel.tsx
git commit -m "refactor(export): unified frontend export handler, remove openPath"
```

---

### Task 9: Integration test — full export round-trip

**Files:**
- Modify: `src-tauri/src/export/helpers.rs` (extend tests)
- Modify: `src-tauri/src/export/markdown.rs` (add write integration test)

**Step 1: Add markdown write integration test**

Add to `export/markdown.rs` tests:

```rust
#[test]
fn write_creates_md_and_images_dir() {
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join("test-guide.md");

    let step = sample_step(); // no screenshot_path, so no images dir needed

    write("Test Guide", &[step], md_path.to_str().unwrap()).unwrap();

    assert!(md_path.exists());
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(content.contains("# Test Guide"));
    assert!(content.contains("Step 1"));

    // No images dir since no screenshots
    let images_dir = tmp.path().join("test-guide-images");
    assert!(!images_dir.exists());
}
```

**Step 2: Run all tests**

Run: `cd src-tauri && cargo test`
Expected: all pass

**Step 3: Run clippy**

Run: `cd src-tauri && cargo clippy -- -D warnings`
Expected: no warnings

**Step 4: Commit**

```bash
git add src-tauri/src/export/
git commit -m "test(export): add integration tests for markdown write"
```

---

### Task 10: Final verification and cleanup

**Step 1: Full build check (frontend + backend)**

Run: `cd /Users/markus/Daten/Development/Privat/psr_tool_mac && npm run build`
Run: `cd src-tauri && cargo test`
Run: `cd src-tauri && cargo clippy -- -D warnings`

**Step 2: Verify lib.rs is cleaner**

Check that `lib.rs` no longer contains HTML/CSS strings, base64 loading, or per-format export functions. The file should be ~200 lines shorter.

**Step 3: Final commit (if any cleanup needed)**

```bash
git add -A
git commit -m "chore: final export refactor cleanup"
```
