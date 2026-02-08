# Task 7 Panel Code Quality Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove unused monitor helper and ensure panel hide errors are propagated.

**Architecture:** Delete the unused monitor resolution helper and its call to keep panel positioning simple. Propagate panel hide errors by using the `?` operator.

**Tech Stack:** Rust, Tauri 2, tauri-nspanel.

---

### Task 1: Remove unused monitor helper + call

**Files:**
- Modify: `src-tauri/src/panel.rs`

**Step 1: (Optional) Write the failing test**
No deterministic monitor API unit test. Skip.

**Step 2: Implement minimal change**

```rust
// Remove resolve_monitor and its call in position_panel_at_tray_icon.
```

**Step 3: Manual check**
Open/close panel; ensure no compile errors.

**Step 4: Commit**
Skip (user requested no commits).

---

### Task 2: Propagate panel.hide() error

**Files:**
- Modify: `src-tauri/src/panel.rs`

**Step 1: (Optional) Write the failing test**
No panel hide unit test. Skip.

**Step 2: Implement minimal change**

```rust
panel.hide()?;
```

**Step 3: Manual check**
Build/run; ensure no unused result warnings.

**Step 4: Commit**
Skip (user requested no commits).
