# Startup UX Spec

## Problem
StepCast auto-shows its panel on every launch. This is intrusive for returning users and fails silently when menu bar icons are hidden by third-party apps.

## Goals
- Remove auto-show on every launch
- First-run hint via temporary Dock icon (ActivationPolicy::Regular)
- Add tray menu items ("Open StepCast", "Quick Start") for discoverability
- Add global shortcut (Cmd+Shift+S) as fallback for menu-bar-hider apps
- Show a one-time tutorial banner in the UI on first run

## Non-goals
- macOS notifications (would require new plugin + notification permission)
- Onboarding wizard or multi-step tutorial

## Approach
1. **Startup state module** (`startup_state.rs`): persist `has_launched_before` as JSON in `dirs::config_dir()/com.w0nk1.stepcast/startup_state.json`. Uses existing `dirs` + `serde_json` deps.
2. **No auto-show**: panel never opens automatically. First run uses `ActivationPolicy::Regular` (Dock icon visible as hint). Returning users get `Accessory` (menu bar only).
3. **Global shortcut**: register `Cmd+Shift+S` via `tauri-plugin-global-shortcut` to toggle panel visibility. New dependency (first-party Tauri 2 plugin).
4. **Tray menu**: add "Open StepCast", "Quick Start", separator, "Quit StepCast". "Quick Start" emits event to show tutorial banner.
5. **Tutorial banner**: lightweight React component shown once on first run or via "Quick Start" tray menu. Dismisses via `mark_startup_seen` command, which also switches to Accessory policy.
6. **Tauri commands**: `get_startup_state` (returns `{has_launched_before}`), `mark_startup_seen` (sets `has_launched_before = true` + switches to Accessory).
7. **Dock reopen**: `RunEvent::Reopen` handler shows the panel when Dock icon is clicked.
8. **Icon fix**: regenerated all icon assets (PNGs, .icns, .ico) with full-bleed gradient (no transparent corners) so macOS applies squircle mask correctly.

## Success criteria
- Cold start: panel does NOT auto-show
- First run: Dock icon visible, clicking it opens panel with welcome banner
- "Got it" dismisses banner + hides Dock icon (switches to Accessory)
- Subsequent launches: no Dock icon, no auto-show
- Cmd+Shift+S toggles panel at any time
- Tray menu "Open StepCast" shows panel
- Tray menu "Quick Start" shows panel + tutorial banner
- All existing tests pass; new unit tests for startup state module
