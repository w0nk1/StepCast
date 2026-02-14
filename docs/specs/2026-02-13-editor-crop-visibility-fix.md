# Spec: Editor Crop Apply Keeps Screenshot Visible

Date: 2026-02-13
Owner: codex
Status: Implemented

## Problem
- After applying a crop in the step editor, the screenshot can disappear.
- User impact: recorded instructions become unusable because the key visual context is gone.

## Root Cause
- Cropped mode uses absolute image positioning.
- Wrapper/frame sizing could collapse (0-size layout path) after crop apply.
- Crop rendering also depended on loaded natural image size; that introduced a fragile state boundary.

## Requirements
- Applying a crop must never hide the screenshot.
- Crop view must render immediately, even if natural image size is not yet known.
- No destructive image rewrite; UI-only crop state stays in `crop_region`.

## Implementation
- `EditorStepCard`:
  - Enable cropped rendering whenever a valid non-full crop exists (independent of natural-size availability).
  - Add stable frame style with fallback `aspect-ratio` (`16/9`) + `width: 100%`.
  - Keep natural-size update on regular image load and on crop-modal image load.
- `editor.css`:
  - Make editor image wrapper block-level with deterministic width.
  - Ensure crop frame is `position: relative` and width-constrained.
  - Prevent collapse when cropped image is absolutely positioned.
- Regression test:
  - Added test ensuring cropped frame is rendered with a non-empty aspect ratio style when `crop_region` exists.

## Validation
- Run frontend tests for `EditorStepCard`.
- Run full test suite + build.
- Manual replay:
  - Open editor.
  - Apply crop on a step with screenshot.
  - Verify image remains visible and marker position remains coherent.

