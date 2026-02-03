2026-02-02
- Default: initial open-source distribution unsigned (Gatekeeper bypass) until Apple Developer account available; reason: no dev account now, keep momentum.
- Default: stack = Tauri 2 + Vite + React + TypeScript + bun; reason: common ecosystem, fast iteration, easy to hire/help.
- Default: MVP screenshot capture via macOS `screencapture` CLI; reason: minimal native code, fastest to ship.
- Default: worktree branch name `feat/stepcast-mvp`; reason: conventional branch naming, matches plan scope.
- Default: scaffold in repo root using create-tauri-app project name from current directory; reason: plan requires root, CLI derives names from target dir.
- Default: license = MIT; reason: permissive OSS license, easy adoption.
- Default: record Task 2 spec by extending existing spec file, not adding a new doc; reason: user request to avoid extra docs while honoring spec requirement.

2026-02-03
- Default: allow negative x/y in capture region validation; reason: multi-display coordinates can be negative.
- Default: storage write_steps returns Result<(), String> with serde_json::to_string_pretty; reason: simple error surface + readable steps.json.
- Default: use tauri-plugin-macos-permissions version "2"; reason: aligns with Tauri 2 plugin major versions.
- Default: request_permissions issues request_* then check_* to return booleans; reason: request_* returns unit in plugin API.
- Default: PermissionStatus implements Default (false/false) for minimal unit test; reason: keep test tiny and deterministic.
- Default: use tauri-nspanel git dependency on branch "v2.1"; reason: upstream docs recommend v2.1 for Tauri 2.
- Default: panel label "panel" and size 360x420; reason: stable handle for tray toggling and compact starter size.
- Default: panel style mask uses nonactivating_panel + utility_window with no_activate + hides_on_deactivate; reason: menu bar panel should not steal focus and should hide on blur.
- Default: tray icon uses bundled icons/icon.png with template rendering and left-click toggles panel; reason: matches macOS menu bar conventions.
- Default: start_recording returns "missing screen recording or accessibility permission" when permissions false; reason: concise error without new error types.
- Default: tray icon uses `icons/icon.png` resource; reason: existing asset in repo, no new files.
- Default: panel/tray uses window label `main`; reason: default Tauri window label.
- Default: tray icon resolve order = Resource icons/icon.png -> Resource icon.png -> App icons/icon.png -> App icon.png; reason: cover dev/prod bundle layouts with explicit NotFound error.
- Default: export templates minimal HTML/Markdown with only title placeholder; reason: task scope requires title only and avoids extra styling.
- Default: exclude .vscode recommendations from commit unless requested; reason: editor-specific and not required for build.
