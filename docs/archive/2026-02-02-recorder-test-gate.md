# Spec: Recorder Test Naming + Test-only Module Gate

**Goal:** Rename recorder test to match behavior and gate recorder module behind `#[cfg(test)]`.

**Scope:**
- Rename test in `src-tauri/src/recorder/mod.rs` to `harness_runs`.
- Gate `mod recorder` in `src-tauri/src/lib.rs` with `#[cfg(test)]`.

**Out of scope:** Any recorder behavior change or new functionality.
