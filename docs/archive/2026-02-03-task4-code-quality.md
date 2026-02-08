# Spec: Task 4 Recorder Capture Errors

**Goal:** Add structured capture errors and validation for macOS screencapture backend.

**Scope:**
- Add `CaptureError` enum and use it in `CaptureBackend`.
- Validate capture region and return structured errors.
- Capture stderr on command failure.

**Out of scope:** New capture behavior beyond validation and error shaping.
