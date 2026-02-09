#!/bin/sh
# Pre-push: run clippy, tests with coverage, and type-check

set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

echo "Running pre-push checks..."

echo "-> cargo clippy"
(cd src-tauri && cargo clippy -- -D warnings)

echo "-> npm test:coverage"
npm run test:coverage

echo "-> tsc (type-check)"
npx tsc --noEmit

echo "All checks passed."
