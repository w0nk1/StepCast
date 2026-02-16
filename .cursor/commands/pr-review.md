---
description: Review a PR by number - checkout, compare against main, run security checks, and return merge guidance
args:
  - name: pr
    description: PR number (e.g. 42)
    required: true
---

Review PR #{{pr}} with this workflow:

## 1. Gather context

Run in parallel:
- `gh pr view {{pr}} --json title,body,headRefName,baseRefName,files,additions,deletions,author`
- Read relevant baseline files in current `main` (affected frontend components, tauri commands, workflows, tests)
- Read translation/i18n and release policy docs if touched (`CONTRIBUTING.md`, `docs/choices.md`, `docs/breadcrumbs.md`)

## 2. Checkout and read complete diff

- `gh pr checkout {{pr}}`
- Read all changed files in the PR, not only highlighted hunks
- Validate tests and workflows impacted by the change

## 3. Review checklist

Check:
- Correctness and regression risk (frontend + tauri + swift bridge where applicable)
- Security and privacy impact (permissions, commands, external calls, secrets)
- i18n consistency (no mixed-language output for selected app language)
- CI compatibility (coverage thresholds, clippy, build)
- Contributor ergonomics (docs/scripts updated when contributor workflow changes)
- Test coverage for changed behavior (new branches and failure paths)

## 4. Classify findings

Group by severity:
- Blocker: security/data loss/broken release behavior
- High: likely user-visible bug or broken contributor flow
- Medium: convention mismatch, missing test, maintainability risk
- Low: minor cleanup/nits

## 5. Present result and action options

Return:
- Findings first, sorted by severity, with file references
- Explicit recommendation:
  1. Request changes
  2. Comment-only follow-up
  3. Safe to merge

If user chooses review posting:
- Use `gh pr review {{pr}} --comment` (or `--request-changes` if needed)
- Keep comment concise and actionable

After review actions:
- `git checkout main`
