# Spec: AI Grounding + Debug Telemetry Hardening

Date: 2026-02-12
Status: implemented

## Problem
Recent sessions show capture correctness but unstable AI labels:
- wrong target label from OCR drift (`vat` instead of `Pepper`)
- context-menu contamination in right-click labels (`@ öffnen`)
- OCR typo in Finder sidebar (`Schreptisch`)
- icon-font glyph leakage in labels (``)
- low observability for why baseline/candidate was chosen

## Goals
1. Keep high-specificity labels when reliable.
2. Fall back deterministically when uncertain.
3. Expose per-step decision data for audits.

## Non-Goals
- app-specific rule engines
- complex ML confidence calibration

## Changes
1. Strengthen label normalization/cleanup (context-menu suffixes, icon glyph filtering, typo canonicalization).
2. Keep `window_title` label for list/sidebar contexts with list container signals.
3. Make quality gate return decision reason, not only text.
4. Include per-step debug payload in AI helper response and recorder logs.

## Acceptance
- replay latest problematic trace yields:
  - `step-005` no `vat`
  - `step-030` no `Schreptisch`
  - `step-031` no `@ öffnen`
  - `step-008/009` no icon glyph
- `cargo check` passes.
