# Recorder Test Naming + Test-only Module Gate Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Align recorder test naming with behavior and make the recorder module test-only via cfg.

**Architecture:** Keep changes minimal: rename the test function to describe its pass condition and gate the recorder module import in `src-tauri/src/lib.rs` behind `#[cfg(test)]` so it only compiles in tests.

**Tech Stack:** Rust, Cargo tests, Tauri workspace.

---

### Task 1: Rename recorder test to match behavior

**Files:**
- Modify: `src-tauri/src/recorder/mod.rs`
- Test: `src-tauri/src/recorder/mod.rs`

**Step 1: Read current test name and behavior**

**Step 2: Update test name to `harness_runs`**

```rust
#[test]
fn harness_runs() {
    // existing test body unchanged
}
```

**Step 3: Run test to confirm it still passes**

Run: `cargo test`
Expected: PASS

### Task 2: Gate recorder module import to tests only

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

**Step 1: Read module imports and current cfg attributes**

**Step 2: Gate recorder module with `#[cfg(test)]`**

```rust
#[cfg(test)]
mod recorder;
```

**Step 3: Run tests to confirm no regressions**

Run: `cargo test`
Expected: PASS
