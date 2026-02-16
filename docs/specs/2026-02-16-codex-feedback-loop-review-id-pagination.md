# Codex Feedback Loop Review-ID Pagination Fix

## Scope
- Fix `/codex-fix` and `workflow_dispatch` review-id resolution for PRs with paginated review history.
- Prevent multiline `review_id` output corruption in GitHub Actions step outputs.

## Design
- Update `.github/workflows/codex-feedback-loop.yml` review lookup to use `gh api --paginate --slurp | jq -r ...`.
- Flatten paginated pages inside `jq` before filtering codex `COMMENTED` reviews and selecting the latest id.
- Keep closed-PR no-op echo YAML-safe by removing inline `#` from plain-scalar `run:` command.
- Add a regression test that asserts the workflow keeps `--slurp` in the `pulls/$PR/reviews` lookup command.

## Verification
- `npm test -- src/utils/codexFeedbackWorkflow.test.ts`
