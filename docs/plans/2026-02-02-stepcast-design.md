# StepCast Design

## Spec (minimal)
Goal: macOS PSR-style recorder. Menu bar panel. Capture steps with screenshots. Export PDF + Markdown + HTML. Local-only. Open source later (repo private now).

Non-goals (v0):
- Cloud sync, team sharing, auto-upload
- OCR, redaction automation, AI summaries
- Cross-platform
- Silent PDF export (use Print dialog)

Constraints:
- No Apple Developer account now (unsigned builds).
- macOS permissions required: Screen Recording + Accessibility (Input Monitoring optional).
- Tauri 2 base.

Success criteria (v0):
- Record/pause/stop from menu bar panel
- Each step has screenshot + action metadata
- Steps editable
- Export HTML/Markdown/PDF works on macOS
- Works on multi-monitor

## UX
- Menu bar tray icon.
- Panel opens on click, hides on blur.
- Clear permission gating UI with deep-link to System Settings.
- Live step list with thumbnails, edit step title/notes.
- Export buttons: HTML, Markdown, PDF (Print dialog).

## Architecture
- Tauri 2 app. React + TypeScript UI. Rust core.
- Recorder service: state machine + event capture + screenshot capture.
- Exporter: HTML + Markdown template render. PDF via print.

## Data model
Guide:
- id, title, created_at, steps[]

Step:
- id, ts, action (click|shortcut|note), x, y, app, window_title, screenshot_path, note?

Session:
- state (idle|recording|paused|stopped), guide_id

## Storage
- Project dir per guide:
  - steps.json
  - screenshots/step-001.png
  - export/guide.html, guide.md, guide.pdf

## Permissions
- Screen Recording: required for screenshots.
- Accessibility: required for global clicks/shortcuts.
- Input Monitoring: only if we capture raw key input; v0 default = shortcuts only.

## Export
- HTML template (single file) referencing local images.
- Markdown with image links.
- PDF via print dialog from HTML preview.

## Distribution
- v0 unsigned builds. Later: signed + notarized, auto-updater.

## Risks / mitigations
- Permissions friction: explicit onboarding screen.
- Screenshot reliability: fallback to step without image, visible warning.
- PDF quality: use HTML print CSS, avoid complex layouts.

## References
- Screen Recording permission (Apple): https://support.apple.com/guide/mac-help/control-access-screen-system-audio-recording-mchld6aa7d23/mac
- Accessibility permission (tauri-plugin-macos-permissions): https://deepwiki.com/ayangweb/tauri-plugin-macos-permissions/4.3-accessibility-permission
- Screen Recording permission (tauri-plugin-macos-permissions): https://deepwiki.com/ayangweb/tauri-plugin-macos-permissions/4.6-screen-recording-permission
- Tauri PDF export limitation (wry issue): https://github.com/tauri-apps/wry/issues/707
