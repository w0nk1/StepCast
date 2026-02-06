2026-02-02 Task 1 Spec Gaps

Goal
- Restore missing .worktrees/ ignore entry while keeping scaffold ignores.
- Re-run recorder harness TDD proof: failing test then passing test.

Scope
- Edit .gitignore to include .worktrees/.
- Temporarily flip recorder harness assertion to fail, run cargo test, revert, run again.

Verification
- cargo test --manifest-path src-tauri/Cargo.toml (fail then pass)
