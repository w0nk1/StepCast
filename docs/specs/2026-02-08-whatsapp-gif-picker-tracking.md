# WhatsApp GIF Picker Capture Fix

## Problem

When clicking a GIF in WhatsApp's GIF picker overlay, the captured screenshot shows the main chat window instead of the GIF picker.

## Reproduction

1. Open WhatsApp, start a chat
2. Open the GIF picker (emoji button > GIF tab)
3. Click on a GIF
4. The recorded step shows the main chat window screenshot, not the picker overlay

## Root Cause

The capture-window reconciliation at `pipeline.rs:1053-1081` replaces a correctly-chosen topmost overlay window when `clicked_app` (from AX API `get_friendly_app_name`) doesn't exactly match `capture_window.app_name` (from CGWindow `get_process_name_by_pid`). These naming sources can differ (localized vs process name). The mismatch triggers `get_main_window_for_pid()` which returns the **largest** window (main chat), overwriting the picker.

Even when names match, the reconciliation block is designed for cross-app clicks (e.g., menu bar app clicks) and should not replace a capture window that was already correctly resolved from the topmost overlay at the click point.

## Fix

Track whether `capture_window` came from `topmost_at_click`. If so, skip the `get_main_window_for_pid` replacement in the reconciliation block. Still use `clicked_app` for label text.

## Acceptance Criteria

- GIF picker screenshot shows the picker overlay, not the main chat
- Step title references WhatsApp (correct app label)
- Existing overlay/popup capture (context menus, dialogs) still works
- `cargo test` passes
