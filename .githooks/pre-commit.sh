#!/bin/sh
# Ensure Cargo.lock is staged whenever Cargo.toml changes

STAGED=$(git diff --cached --name-only --diff-filter=ACMR)

TOML_STAGED=$(echo "$STAGED" | grep 'Cargo\.toml$' || true)
LOCK_STAGED=$(echo "$STAGED" | grep 'Cargo\.lock$' || true)

if [ -n "$TOML_STAGED" ] && [ -z "$LOCK_STAGED" ]; then
  echo "ERROR: Cargo.toml is staged but Cargo.lock is not."
  echo "Run: git add src-tauri/Cargo.lock"
  exit 1
fi
