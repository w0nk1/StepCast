2026-02-02 Task 1 Spec

Goal
- Scaffold Tauri 2 app (React + TS, bun) in repo root.
- Verify Rust test harness by failing then passing unit test.

Scope
- Use create-tauri-app non-interactively.
- Add recorder module file with minimal test.

Steps
- Run create-tauri-app with flags: Tauri v2, React + TS, bun.
- Add failing test in src-tauri/src/recorder/mod.rs.
- Run cargo test (expect fail), fix test, run cargo test (expect pass).

Verification
- cargo test --manifest-path src-tauri/Cargo.toml

2026-02-02 Task 2 Spec

Goal
- Add recorder data model with JSON roundtrip test.

Scope
- New ActionType + Step types with serde.
- Sample constructor for tests only.

Steps
- Write failing JSON roundtrip test for Step.
- Implement ActionType/Step + sample, update recorder mod.

Verification
- cargo test --manifest-path src-tauri/Cargo.toml recorder::types::tests::step_roundtrip_json
