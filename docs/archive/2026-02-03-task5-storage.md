# Task 5 Storage Spec

Goal: persist recorder steps to disk as JSON.

API:
- write_steps(dir: &Path, steps: &[Step]) -> Result<(), String>

Behavior:
- write JSON array to dir/steps.json
- use serde_json; map errors to String

Test:
- tempfile::tempdir
- write_steps creates steps.json with roundtrip parse
