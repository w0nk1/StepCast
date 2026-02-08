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
- Default: request_permissions opens macOS Privacy panes when permissions missing; reason: macOS does not always show prompts.
- Default: PermissionStatus implements Default (false/false) for minimal unit test; reason: keep test tiny and deterministic.
- Default: use tauri-nspanel git dependency on branch "v2.1"; reason: upstream docs recommend v2.1 for Tauri 2.
- Default: panel label "main" and size 350x720; reason: keep export visible without scrolling.
- Default: panel style mask uses nonactivating_panel; reason: menu bar panel should not steal focus.
- Default: align panel settings with OpenUsage (MainMenu+1 level, can_join_all_spaces + stationary + full_screen_auxiliary, nonactivating style mask); reason: proven menu bar panel behavior.
- Default: show panel before positioning on tray click; reason: macOS quirk requires window shown to move between monitors.
- Default: tray icon uses bundled icons/icon.png with template rendering and left-click toggles panel; reason: matches macOS menu bar conventions.
- Default: tray icon disables menu-on-left-click to ensure click events fire; reason: click should toggle panel instead of a menu.
- Default: tray toggle reacts on MouseButtonState::Up; reason: avoid double toggles from Down+Up events.
- Default: panel movable by window background; reason: allow dragging even without a title bar.
- Default: tray click uses show_and_make_key before positioning; reason: ensures visibility and avoids macOS positioning quirks.
- Default: panel can become key and uses show_and_make_key on tray click; reason: avoid immediate hide when app is inactive.
- Default: start_recording returns "missing screen recording or accessibility permission" when permissions false; reason: concise error without new error types.
- Default: tray icon uses `icons/icon.png` resource; reason: existing asset in repo, no new files.
- Default: panel/tray uses window label `main`; reason: default Tauri window label.
- Default: tray icon resolve order = Resource icons/icon.png -> Resource icon.png -> App icons/icon.png -> App icon.png; reason: cover dev/prod bundle layouts with explicit NotFound error.
- Default: export templates minimal HTML/Markdown with only title placeholder; reason: task scope requires title only and avoids extra styling.
- Default: exclude .vscode recommendations from commit unless requested; reason: editor-specific and not required for build.

2026-02-08 (smoothness review)
- Default: package manager = npm; reason: CI already uses npm ci, wider contributor compat, one lockfile.
- Default: pin git deps (tauri-nspanel da9c9a8, tauri-plugin-aptabase e896cce) to commit SHAs; reason: reproducible builds, update quarterly.
- Default: skip ESLint/Prettier; reason: TS strict mode suffices, small team, zero code quality issues found. Revisit when team >2.
- Default: skip structured logging (log/tracing crates); reason: custom debug_log() + conditional eprintln! is sufficient for current scale.
- Default: add cargo-audit + npm audit to CI; reason: catch known vulnerabilities early.
- Default: keep stale worktrees (codex/fast-capture, codex/permission-gate-settings); reason: branches have unmerged work.
