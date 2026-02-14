# Pre-Click ScreenCaptureKit Buffer

Date: 2026-02-12

## Problem

Volatile UI (browser/Telegram menus, popovers) closes before current capture path finishes.  
Result: screenshot often misses the clicked menu state; AI label can still be correct from AX, but image context is wrong.

## Goal

Use a short ScreenCaptureKit frame ringbuffer and prefer a pre-click frame for volatile interactions (`AXMenu`, `AXMenuItem`, transient popup/group roles).

## Scope

- Add macOS-only `pre_click_buffer` recorder module.
- Start buffer when recording starts; stop on stop/discard.
- In pipeline transient capture branch, attempt pre-click frame first.
- Keep existing CG/window/region capture as fallback.

## Approach

1. ScreenCaptureKit stream for active display, low-latency settings (BGRA, limited queue depth, capped fps).
2. Maintain small in-memory ringbuffer with recent frames + capture timestamps.
3. On volatile click:
   - Select latest frame at or before click timestamp (fallback: latest frame).
   - Write frame to current `screenshot_path`.
   - Use display bounds as capture bounds for click percentages.
4. If unavailable/failed, continue existing capture logic unchanged.

## Non-Goals

- No OCR/AI prompt changes in this patch.
- No replacement of existing capture pipeline; only an additional robust fast-path.

## Verification

- Unit tests for frame selection policy (before-click preference + fallback).
- `cargo test` targeted recorder tests.
- `cargo check` for full crate compile.
