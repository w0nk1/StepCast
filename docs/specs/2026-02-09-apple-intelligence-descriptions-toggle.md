# Apple Intelligence Descriptions (Toggle + Generation + Editor UX)

Date: 2026-02-09

## Goal

- Provide a global opt-in toggle for Apple Intelligence-powered step descriptions.
- Generate concise per-step descriptions on-device using the Foundation Models framework.
- Communicate requirements + privacy clearly (on-device; no step upload).

## Non-Goals

- Screenshot understanding (OCR/Vision / image model prompts).
- Tool-calling to external data sources (Contacts/Calendar/etc).
- Multi-language UI for descriptions (v1 uses concise English prompts).

## UX

- Settings: `Apple Intelligence -> Use for step descriptions` (global toggle, default OFF).
- Toggle UI: custom Apple-style toggle (button + CSS) to match macOS System Settings look reliably inside Tauri/WKWebView.
- Eligibility hint (runtime via Foundation Models availability):
  - Device must support Apple Intelligence.
  - Apple Intelligence must be enabled in macOS System Settings.
  - Model may be temporarily not ready (download/initialization).
- Provide a button to open macOS settings (deep link + fallback):
  - Primary deep link: `x-apple.systempreferences:com.apple.Siri-Settings.extension`
  - Fallback: `x-apple.systempreferences:com.apple.preference.siri`
- Recorder: after stopping a recording, if toggle is ON, auto-generate descriptions for steps that are missing one.
- Step Editor:
  - Toolbar button `Enhance Steps` (uses missing-only when missing exists, otherwise regenerates non-manual steps).
  - Per-step sparkle button to regenerate that step.
  - Clicking the description text allows inline manual editing (stored as manual source).
- Quick Start (welcome banner): one bullet mentioning the optional toggle.

## Storage

- `localStorage.appleIntelligenceDescriptions` = `"true" | "false"`

## Privacy Copy (must be precise)

- For Apple Intelligence descriptions: runs on-device and works offline.
- StepCast does not upload recorded steps or screenshots for this feature (no third-party LLMs).
- (Do not make broader claims about analytics/update checks.)
