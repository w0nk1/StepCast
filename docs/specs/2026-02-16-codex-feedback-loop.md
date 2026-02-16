# Codex Feedback Loop + Review Playbook

## Scope
- Add a reusable PR review playbook command (`.cursor/commands/pr-review.md`).
- Add automated codex follow-up workflow for review findings.
- Fix mixed-language AI eligibility reasons by honoring selected app language.

## Design
- Workflow `.github/workflows/codex-feedback-loop.yml`:
  - Triggers on codex review submission, `/codex-fix` PR comments, and manual dispatch.
  - Collects codex inline findings from PR review comments.
  - Posts a deduplicated follow-up comment: `@codex address that feedback`.
- Settings frontend passes resolved app language into `get_apple_intelligence_eligibility`.
- Rust backend resolves that language and localizes eligibility reason strings.

## Verification
- `npm test`
- `cd src-tauri && cargo test --quiet`
- Workflow syntax sanity via local YAML inspection
