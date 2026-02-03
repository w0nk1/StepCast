# StepCast

PSR-style step recorder for macOS. Menu bar panel, local-only workflow, exportable guides.

## Status

Early MVP. UI and permissions wiring in place; capture and persistence are being integrated.

## Features (MVP)

- Menu bar panel with record/pause/resume/stop controls
- Permission checks for Screen Recording + Accessibility
- Export HTML, Markdown, and PDF (via print dialog)
- Local-only by default

## Permissions

StepCast needs macOS permissions to record steps and screenshots:

- Screen Recording
- Accessibility

If missing, the UI will prompt to grant them. After granting, restart the app if macOS requires it.

## Development

```bash
bun install
bun tauri dev
```

## Build

```bash
bun tauri build
```

Unsigned builds will trigger Gatekeeper. Use right-click -> Open, or allow in System Settings > Privacy & Security.

## Manual Verification Checklist

- Tray icon appears
- Panel toggles on click, hides on blur
- Permissions banner updates after request
- Record/Pause/Resume/Stop buttons call Tauri commands without errors
- Export HTML/Markdown downloads
- PDF opens print dialog

## License

MIT
