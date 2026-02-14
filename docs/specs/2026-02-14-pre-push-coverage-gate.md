# 2026-02-14 pre-push coverage gate hardening

## Context
Pre-push hook blocks branch push due per-file 80% coverage thresholds on:
- `src/components/EditorWindow.tsx`
- `src/components/EditorStepCard.tsx`
- `src/components/SettingsSheet.tsx`
- `src/components/StepItem.tsx`
- `src/utils/stepCrop.ts`

## Goal
Raise coverage via behavior tests (no threshold relaxation, no ignore hints), keep runtime logic unchanged.

## Scope
- Add targeted tests for unexecuted branches:
  - AI toggle sync/fallback and generation branching
  - crop modal controls + retry branches
  - settings fallback/openUrl/toggle branches
  - thumbnail retry and sortable branches
  - crop helper utility branches
- Validate with exact pre-push chain:
  - `cargo clippy -- -D warnings`
  - `npm run test:coverage`
  - `npx tsc --noEmit`

## Non-goals
- No feature/UI behavior change
- No runtime code refactor
- No threshold config changes
