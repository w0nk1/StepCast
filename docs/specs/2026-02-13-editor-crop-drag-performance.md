# Spec: Editor Crop Drag Performance & UX Hardening

Date: 2026-02-13
Owner: codex
Status: Implemented

## Goal
- Make drag-based crop repositioning feel immediate and stable.
- Eliminate native image drag artifacts (ghost image + plus cursor).
- Keep render cost low for larger recordings.

## Problems Observed
- Browser native image drag could trigger while user drags crop preview.
- Crop updates on every pointer event can cause unnecessary re-render churn.
- Full-size screenshots in long lists increase initial rendering and decode cost.

## Decisions
- Disable native image drag (`draggable=false`, `onDragStart preventDefault`, `-webkit-user-drag: none`).
- Keep pointer events on crop frame, not image (`pointer-events: none` on cropped image).
- Batch pointer-move crop preview with `requestAnimationFrame`.
- Persist crop only on pointer release and only when movement is meaningful.
- Use `loading="lazy"` and `decoding="async"` for preview and thumbnail images.

## Validation
- Unit tests for editor crop interactions remain green.
- Full frontend test suite remains green.
- Production build passes.

