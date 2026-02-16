# Double-Click List Grounding Safety

## Scope
- Prevent wrong filename references in AI descriptions for double-clicked list rows.
- Target cases where AX returns generic file-kind labels and OCR drifts to adjacent rows.

## Design
- In `chooseGroundingLabel`:
  - keep existing right-click behavior (OCR-preferred for context menu targets),
  - for `kind == "list item"` + `action == "DoubleClick"`, avoid OCR-as-source when AX label is generic/empty,
  - fall back to unlabeled baseline instead of guessing a filename.

## Verification
- `swiftc -typecheck src-tauri/swift/stepcast_ai_helper.swift src-tauri/swift/stepcast_ai_helper_descriptions.swift src-tauri/swift/stepcast_ai_helper_vision.swift`
