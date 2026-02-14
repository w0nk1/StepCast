# Spec: Step Focus Crop System

Date: 2026-02-13
Status: Done

## Goal
Provide reliable, non-destructive focus crops for recorded step screenshots so guides remain readable and actionable, especially when source captures are full-screen.

## Requirements
1. Non-destructive: always keep original screenshot file untouched.
2. Persisted crop metadata per step as percentages (`x/y/width/height`) so crops are resolution-independent.
3. Recorder should set a sensible default focus crop for very large captures (not only exact full-screen) to keep dialogs/overlays readable.
4. Editor must allow manual crop adjust + reset-to-full.
5. Click marker must remain accurate after crop in panel/editor/export.
6. Export (HTML/Markdown/PDF) must apply crop to image output.
7. Backward compatibility: steps without crop render/export as full image.

## Data Model
- Add optional `crop_region: BoundsPercent` to step model (frontend + backend).
- Validation constraints: clamp values to [0..100], enforce min width/height.

## Recorder Default Focus Heuristic
Apply when capture bounds are large display-like frames (coverage heuristic against clicked display).
- Base around click location.
- If AX element bounds exist, expand around element with margin.
- Clamp to display bounds in percent.
- Leave `None` for auth placeholders and already-tight captures.

## Editor UX
- Add "Crop" action on step card.
- Open modal cropper with live preview and click marker context.
- Save => persist `crop_region`.
- Reset => clear `crop_region` (full image).

## Export Rules
- If `crop_region` exists, crop image bytes during export pipeline before encoding.
- Marker coordinates remapped to cropped coordinate space for HTML/PDF overlays.
- Markdown image output uses cropped bitmap.

## Edge Cases
- Invalid crop metadata -> ignore crop (fallback full image).
- Marker outside crop -> hide marker (avoid misleading point).
- Missing screenshot -> no image/crop rendering.

## Verification
- Rust unit tests for crop normalization + exported marker remap.
- Frontend tests for marker remap and crop UI command calls.
- Existing tests must remain green.
