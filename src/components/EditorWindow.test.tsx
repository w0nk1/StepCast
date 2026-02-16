import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import EditorWindow from "./EditorWindow";
import type { Step } from "../types/step";

let capturedDragEnd: ((event: any) => void) | null = null;

vi.mock("@dnd-kit/core", async () => {
  const actual = await vi.importActual("@dnd-kit/core");
  return {
    ...actual,
    DndContext: ({ children, onDragEnd }: any) => {
      capturedDragEnd = onDragEnd;
      return children;
    },
  };
});

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

function makeStep(overrides: Partial<Step> = {}): Step {
  return {
    id: "step-1",
    ts: Date.now(),
    action: "Click",
    x: 100,
    y: 200,
    click_x_percent: 50,
    click_y_percent: 50,
    app: "Finder",
    window_title: "Documents",
    screenshot_path: "/tmp/screenshot.png",
    note: null,
    ...overrides,
  };
}

type ListenerMap = Record<string, (event: unknown) => void>;

function captureListeners(): ListenerMap {
  const listeners: ListenerMap = {};
  mockListen.mockImplementation(async (event, handler) => {
    listeners[event as string] = handler as (event: unknown) => void;
    return () => {};
  });
  return listeners;
}

describe("EditorWindow", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockListen.mockReset();
    mockListen.mockResolvedValue(() => {});
  });

  it("shows empty state when no steps", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("No steps recorded yet")).toBeInTheDocument();
    });
  });

  it("renders steps from get_steps", async () => {
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);
    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
      expect(screen.getByText("Clicked in Safari")).toBeInTheDocument();
    });
  });

  it("still loads when get_steps fails", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("session lock poisoned"));
    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("No steps recorded yet")).toBeInTheDocument();
    });
  });

  it("appends new step from step-captured event", async () => {
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    const listeners = captureListeners();

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    act(() => {
      listeners["step-captured"]({ payload: makeStep({ id: "step-2", app: "Safari" }) });
    });

    expect(screen.getByText("Clicked in Safari")).toBeInTheDocument();
  });

  it("ignores duplicate step-captured events", async () => {
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    const listeners = captureListeners();

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    act(() => {
      listeners["step-captured"]({ payload: makeStep({ id: "step-1", app: "Safari" }) });
    });

    // Should still show Finder (original), not Safari (duplicate)
    expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    expect(screen.queryByText("Clicked in Safari")).not.toBeInTheDocument();
  });

  it("updates step from step-updated event", async () => {
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1", note: null })]);
    const listeners = captureListeners();

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    act(() => {
      listeners["step-updated"]({
        payload: makeStep({ id: "step-1", note: "Updated note" }),
      });
    });

    expect(screen.getByText("Updated note")).toBeInTheDocument();
  });

  it("clears steps on steps-discarded event", async () => {
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    const listeners = captureListeners();

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    act(() => {
      listeners["steps-discarded"]({});
    });

    expect(screen.getByText("No steps recorded yet")).toBeInTheDocument();
  });

  it("calls update_step_note on note edit", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined); // for update_step_note

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getByText("Add a note..."));
    const textarea = screen.getByPlaceholderText("Add a note...");
    await user.type(textarea, "My note{Enter}");

    expect(mockInvoke).toHaveBeenCalledWith("update_step_note", {
      stepId: "step-1",
      note: "My note",
    });
  });

  it("renders editor body", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(container.querySelector(".editor-body")).toBeInTheDocument();
    });
  });

  it("deletes step via delete button with undo toast", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getByTitle("Remove step"));
    // Step removed from UI immediately
    expect(screen.getByText("No steps recorded yet")).toBeInTheDocument();
    // Undo toast shown
    expect(screen.getByText("Step deleted")).toBeInTheDocument();
    // Backend not called yet (soft delete)
    expect(mockInvoke).not.toHaveBeenCalledWith("delete_step", { stepId: "step-1" });

    // After 3s, backend is called
    act(() => { vi.advanceTimersByTime(3000); });
    expect(mockInvoke).toHaveBeenCalledWith("delete_step", { stepId: "step-1" });
    vi.useRealTimers();
  });

  it("restores step on undo", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getByTitle("Remove step"));
    expect(screen.getByText("No steps recorded yet")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Undo" }));
    expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    expect(screen.queryByText("Step deleted")).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("delete_step", { stepId: "step-1" });
    vi.useRealTimers();
  });

  it("removes step on step-deleted event", async () => {
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    const listeners = captureListeners();

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    act(() => {
      listeners["step-deleted"]({ payload: "step-1" });
    });

    expect(screen.getByText("No steps recorded yet")).toBeInTheDocument();
  });

  it("reorders steps on steps-reordered event", async () => {
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);
    const listeners = captureListeners();

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    act(() => {
      listeners["steps-reordered"]({
        payload: [
          makeStep({ id: "step-2", app: "Safari" }),
          makeStep({ id: "step-1", app: "Finder" }),
        ],
      });
    });

    const descs = container.querySelectorAll(".editor-step-desc");
    expect(descs[0].textContent).toBe("Clicked in Safari");
    expect(descs[1].textContent).toBe("Clicked in Finder");
  });

  it("updates description through editor card callback path", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getByText("Clicked in Finder"));
    const input = screen.getByPlaceholderText("Step description...");
    await user.clear(input);
    await user.type(input, "Open Finder quickly{Enter}");

    expect(mockInvoke).toHaveBeenCalledWith("update_step_description", {
      stepId: "step-1",
      description: "Open Finder quickly",
    });
  });

  it("updates crop through editor card callback path", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getByTitle("Adjust visible screenshot area"));
    await user.click(screen.getByRole("button", { name: "Apply" }));

    expect(mockInvoke).toHaveBeenCalledWith("update_step_crop", {
      stepId: "step-1",
      cropRegion: null,
    });
  });

  it("generates single-step descriptions only when AI is enabled", async () => {
    const user = userEvent.setup();
    localStorage.setItem("appleIntelligenceDescriptions", "false");
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getAllByTitle("Enable Apple Intelligence descriptions in StepCast Settings")[1]);
    expect(mockInvoke).not.toHaveBeenCalledWith("generate_step_descriptions", {
      stepIds: ["step-1"],
    });

    act(() => {
      window.dispatchEvent(
        new StorageEvent("storage", {
          key: "appleIntelligenceDescriptions",
          newValue: "true",
        }),
      );
    });
    await user.click(screen.getByTitle("Generate with Apple Intelligence"));
    expect(mockInvoke).toHaveBeenCalledWith("generate_step_descriptions", { stepIds: ["step-1"] });
  });

  it("enhances missing descriptions first, then all when none are missing", async () => {
    const user = userEvent.setup();
    localStorage.setItem("appleIntelligenceDescriptions", "true");
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", description: "" }),
      makeStep({ id: "step-2", description: "already there", app: "Safari" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    const listeners = captureListeners();
    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("2 steps")).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /Enhance Steps/i }));
    expect(mockInvoke).toHaveBeenCalledWith("generate_step_descriptions", { mode: "missing_only" });

    act(() => {
      listeners["steps-reordered"]({
        payload: [
          makeStep({ id: "step-1", description: "done" }),
          makeStep({ id: "step-2", description: "done too", app: "Safari" }),
        ],
      });
    });
    await user.click(screen.getByRole("button", { name: /Enhance Steps/i }));
    expect(mockInvoke).toHaveBeenCalledWith("generate_step_descriptions", { mode: "all" });
  });

  it("syncs AI toggle from storage and tauri event listeners", async () => {
    localStorage.setItem("appleIntelligenceDescriptions", "false");
    const listeners = captureListeners();
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    expect(screen.getByRole("button", { name: /Enhance Steps/i })).toBeDisabled();

    act(() => {
      window.dispatchEvent(
        new StorageEvent("storage", {
          key: "some-other-key",
          newValue: "true",
        }),
      );
    });
    expect(screen.getByRole("button", { name: /Enhance Steps/i })).toBeDisabled();

    act(() => {
      window.dispatchEvent(
        new StorageEvent("storage", {
          key: "appleIntelligenceDescriptions",
          newValue: "true",
        }),
      );
    });
    expect(screen.getByRole("button", { name: /Enhance Steps/i })).toBeEnabled();

    act(() => {
      listeners["ai-toggle-changed"]({ payload: { enabled: false } });
    });
    expect(screen.getByRole("button", { name: /Enhance Steps/i })).toBeDisabled();
  });

  it("handles ai-toggle listener registration failure gracefully", async () => {
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockListen.mockImplementation(async (event, handler) => {
      if (event === "ai-toggle-changed") {
        void handler;
        return Promise.reject(new Error("listen failed"));
      }
      return () => {};
    });

    const { unmount } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });
    expect(() => unmount()).not.toThrow();
  });

  it("navigates steps with arrow keys and shows focus ring", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    const scrollEl = container.querySelector(".editor-scroll") as HTMLElement;
    scrollEl.focus();
    await user.keyboard("{ArrowDown}");
    expect(container.querySelector(".editor-step.is-focused")).toBeInTheDocument();

    await user.keyboard("{ArrowDown}");
    const focused = container.querySelectorAll(".editor-step");
    expect(focused[1].classList.contains("is-focused")).toBe(true);

    await user.keyboard("{Escape}");
    expect(container.querySelector(".editor-step.is-focused")).not.toBeInTheDocument();
  });

  it("shows bulk toolbar when steps are selected", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    const checkboxes = container.querySelectorAll(".editor-step-checkbox");
    await user.click(checkboxes[0]);

    expect(screen.getByText("1 selected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deselect" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Deselect" }));
    expect(screen.queryByText("1 selected")).not.toBeInTheDocument();
  });

  it("shows Collapse All / Expand All button when more than 5 steps", async () => {
    const user = userEvent.setup();
    const sixSteps = Array.from({ length: 6 }, (_, i) =>
      makeStep({ id: `step-${i + 1}`, app: `App${i + 1}` }),
    );
    mockInvoke.mockResolvedValueOnce(sixSteps);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("6 steps")).toBeInTheDocument();
    });

    // Should show "Collapse All" initially (nothing collapsed yet)
    const collapseBtn = screen.getByRole("button", { name: /Collapse All/i });
    expect(collapseBtn).toBeInTheDocument();

    // Click Collapse All -> all steps collapsed, button text changes to "Expand All"
    await user.click(collapseBtn);
    expect(screen.getByRole("button", { name: /Expand All/i })).toBeInTheDocument();

    // Note areas should be hidden when collapsed (checked via absence of "Add a note..." buttons)
    const noteButtons = container.querySelectorAll(".editor-step-note-btn");
    expect(noteButtons.length).toBe(0);

    // Click Expand All -> all expanded again
    await user.click(screen.getByRole("button", { name: /Expand All/i }));
    expect(screen.getByRole("button", { name: /Collapse All/i })).toBeInTheDocument();
    const noteButtonsAfter = container.querySelectorAll(".editor-step-note-btn");
    expect(noteButtonsAfter.length).toBe(6);
  });

  it("does not show Collapse All button when 5 or fewer steps", async () => {
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1" }),
      makeStep({ id: "step-2" }),
    ]);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("2 steps")).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: /Collapse All/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Expand All/i })).not.toBeInTheDocument();
  });

  it("toggles collapse on individual step", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    // Click the collapse chevron on the step card
    const collapseBtn = container.querySelector(".editor-step-collapse") as HTMLElement;
    expect(collapseBtn).toBeInTheDocument();
    await user.click(collapseBtn);

    // Note area should be hidden
    expect(container.querySelector(".editor-step-note-btn")).not.toBeInTheDocument();

    // Click again to expand
    await user.click(collapseBtn);
    expect(container.querySelector(".editor-step-note-btn")).toBeInTheDocument();
  });

  it("bulk-deletes selected steps", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
      makeStep({ id: "step-3", app: "Notes" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("3 steps")).toBeInTheDocument();
    });

    // Select first two steps
    const checkboxes = container.querySelectorAll(".editor-step-checkbox");
    await user.click(checkboxes[0]);
    await user.click(checkboxes[1]);
    expect(screen.getByText("2 selected")).toBeInTheDocument();

    // Click bulk Delete
    await user.click(screen.getByRole("button", { name: "Delete" }));

    // Both should be immediately deleted from UI (bulk delete is permanent, no undo)
    expect(screen.queryByText("Clicked in Finder")).not.toBeInTheDocument();
    expect(screen.queryByText("Clicked in Safari")).not.toBeInTheDocument();
    expect(screen.getByText("Clicked in Notes")).toBeInTheDocument();

    // Backend called for each
    expect(mockInvoke).toHaveBeenCalledWith("delete_step", { stepId: "step-1" });
    expect(mockInvoke).toHaveBeenCalledWith("delete_step", { stepId: "step-2" });

    // Selection cleared
    expect(screen.queryByText("selected")).not.toBeInTheDocument();
  });

  it("bulk-generates descriptions for selected steps when AI is enabled", async () => {
    const user = userEvent.setup();
    localStorage.setItem("appleIntelligenceDescriptions", "true");
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("2 steps")).toBeInTheDocument();
    });

    // Select both steps
    const checkboxes = container.querySelectorAll(".editor-step-checkbox");
    await user.click(checkboxes[0]);
    await user.click(checkboxes[1]);

    // Click bulk Enhance button
    await user.click(screen.getByRole("button", { name: "Enhance" }));

    expect(mockInvoke).toHaveBeenCalledWith("generate_step_descriptions", {
      stepIds: ["step-1", "step-2"],
    });
  });

  it("bulk-generate does nothing when AI is disabled", async () => {
    const user = userEvent.setup();
    localStorage.setItem("appleIntelligenceDescriptions", "false");
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("2 steps")).toBeInTheDocument();
    });

    const checkboxes = container.querySelectorAll(".editor-step-checkbox");
    await user.click(checkboxes[0]);

    // The Enhance button should be disabled
    const enhanceBtn = screen.getByRole("button", { name: "Enhance" });
    expect(enhanceBtn).toBeDisabled();
  });

  it("deletes focused step with Delete key", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    const scrollEl = container.querySelector(".editor-scroll") as HTMLElement;
    scrollEl.focus();

    // Arrow down to focus first step
    await user.keyboard("{ArrowDown}");
    expect(container.querySelector(".editor-step.is-focused")).toBeInTheDocument();

    // Press Delete key to remove focused step
    await user.keyboard("{Delete}");

    // Step 1 removed, step 2 remains
    expect(screen.queryByText("Clicked in Finder")).not.toBeInTheDocument();
    expect(screen.getByText("Clicked in Safari")).toBeInTheDocument();

    // Undo toast should appear
    expect(screen.getByText("Step deleted")).toBeInTheDocument();

    vi.useRealTimers();
  });

  it("deletes focused step with Backspace key", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    const scrollEl = container.querySelector(".editor-scroll") as HTMLElement;
    scrollEl.focus();

    // Focus the only step
    await user.keyboard("{ArrowDown}");
    // Press Backspace to delete
    await user.keyboard("{Backspace}");

    // Step removed - empty state
    expect(screen.getByText("No steps recorded yet")).toBeInTheDocument();
    expect(screen.getByText("Step deleted")).toBeInTheDocument();

    vi.useRealTimers();
  });

  it("adjusts focusedIndex when deleting last step via keyboard", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    const scrollEl = container.querySelector(".editor-scroll") as HTMLElement;
    scrollEl.focus();

    // Focus second (last) step
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{ArrowDown}");

    // Delete last step — focus index should move back
    await user.keyboard("{Delete}");

    expect(screen.queryByText("Clicked in Safari")).not.toBeInTheDocument();
    expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();

    vi.useRealTimers();
  });

  it("does not delete when keyboard events come from input/textarea", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    // Click to edit description which opens an input
    await user.click(screen.getByText("Clicked in Finder"));
    const input = screen.getByPlaceholderText("Step description...");

    // Type Backspace inside the input — should NOT delete the step
    await user.type(input, "{Backspace}");
    expect(screen.queryByText("Step deleted")).not.toBeInTheDocument();
  });

  it("handles drag-end reorder by invoking reorder_steps", async () => {
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
      makeStep({ id: "step-3", app: "Notes" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    // Simulate drag of step-1 over step-3 via captured DndContext onDragEnd
    act(() => {
      capturedDragEnd?.({ active: { id: "step-1" }, over: { id: "step-3" } });
    });

    // Should invoke reorder_steps with new order
    expect(mockInvoke).toHaveBeenCalledWith("reorder_steps", {
      stepIds: ["step-2", "step-3", "step-1"],
    });

    // UI should reflect the new order
    const descs = container.querySelectorAll(".editor-step-desc");
    expect(descs[0].textContent).toBe("Clicked in Safari");
    expect(descs[2].textContent).toBe("Clicked in Finder");
  });

  it("ignores drag-end when dropped on same position or no over target", async () => {
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    mockInvoke.mockClear();

    // No over target
    act(() => {
      capturedDragEnd?.({ active: { id: "step-1" }, over: null });
    });
    expect(mockInvoke).not.toHaveBeenCalledWith("reorder_steps", expect.anything());

    // Same position
    act(() => {
      capturedDragEnd?.({ active: { id: "step-1" }, over: { id: "step-1" } });
    });
    expect(mockInvoke).not.toHaveBeenCalledWith("reorder_steps", expect.anything());
  });

  it("flushes pending delete on unmount", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    const { unmount } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    // Delete step (soft delete, timer pending)
    await user.click(screen.getByTitle("Remove step"));
    expect(screen.getByText("Step deleted")).toBeInTheDocument();

    // Backend NOT called yet (still in 3s undo window)
    expect(mockInvoke).not.toHaveBeenCalledWith("delete_step", { stepId: "step-1" });

    // Unmount triggers flush
    unmount();

    // Now the backend should have been called
    expect(mockInvoke).toHaveBeenCalledWith("delete_step", { stepId: "step-1" });

    vi.useRealTimers();
  });

  it("prunes stale selection IDs when steps are removed", async () => {
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);
    const listeners = captureListeners();
    const user = userEvent.setup();

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    // Select step-1
    const checkboxes = container.querySelectorAll(".editor-step-checkbox");
    await user.click(checkboxes[0]);
    expect(screen.getByText("1 selected")).toBeInTheDocument();

    // Externally remove step-1 via event
    act(() => {
      listeners["step-deleted"]({ payload: "step-1" });
    });

    // Selection should be pruned — step-1 no longer exists
    expect(screen.queryByText("1 selected")).not.toBeInTheDocument();
  });

  it("shift-click selects a range of steps", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
      makeStep({ id: "step-3", app: "Notes" }),
      makeStep({ id: "step-4", app: "Mail" }),
    ]);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("4 steps")).toBeInTheDocument();
    });

    const checkboxes = container.querySelectorAll(".editor-step-checkbox");

    // Click first step normally
    await user.click(checkboxes[0]);
    expect(screen.getByText("1 selected")).toBeInTheDocument();

    // Shift-click third step to select range 1-3
    await user.keyboard("{Shift>}");
    await user.click(checkboxes[2]);
    await user.keyboard("{/Shift}");

    expect(screen.getByText("3 selected")).toBeInTheDocument();
  });

  it("dismisses undo toast to flush pending delete immediately", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getByTitle("Remove step"));
    expect(screen.getByText("Step deleted")).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("delete_step", { stepId: "step-1" });

    // Dismiss the undo toast (click the X / dismiss button)
    const dismissBtn = screen.getByTitle("Dismiss");
    await user.click(dismissBtn);

    // Backend should be called immediately on dismiss
    expect(mockInvoke).toHaveBeenCalledWith("delete_step", { stepId: "step-1" });

    vi.useRealTimers();
  });

  it("auto-scrolls to bottom when new step is captured", async () => {
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    const listeners = captureListeners();

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    const scrollEl = container.querySelector(".editor-scroll") as HTMLElement;
    const scrollToSpy = vi.fn();
    scrollEl.scrollTo = scrollToSpy;

    act(() => {
      listeners["step-captured"]({ payload: makeStep({ id: "step-2", app: "Safari" }) });
    });

    // The scroll happens after a 50ms setTimeout
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    expect(scrollToSpy).toHaveBeenCalledWith(
      expect.objectContaining({ behavior: "smooth" }),
    );
  });

  it("ArrowUp with no focus starts at last step", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    const scrollEl = container.querySelector(".editor-scroll") as HTMLElement;
    scrollEl.focus();

    // ArrowUp with no current focus -> focuses last step
    await user.keyboard("{ArrowUp}");

    const steps = container.querySelectorAll(".editor-step");
    expect(steps[1].classList.contains("is-focused")).toBe(true);
  });

  it("clamps ArrowDown at last step and ArrowUp at first step", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    const scrollEl = container.querySelector(".editor-scroll") as HTMLElement;
    scrollEl.focus();

    // Navigate to last step
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{ArrowDown}");
    // Try going past last
    await user.keyboard("{ArrowDown}");

    const stepsAfterDown = container.querySelectorAll(".editor-step");
    expect(stepsAfterDown[1].classList.contains("is-focused")).toBe(true);

    // Navigate to first step
    await user.keyboard("{ArrowUp}");
    // Try going past first
    await user.keyboard("{ArrowUp}");

    const stepsAfterUp = container.querySelectorAll(".editor-step");
    expect(stepsAfterUp[0].classList.contains("is-focused")).toBe(true);
  });

  it("generates description for single step when AI is enabled", async () => {
    const user = userEvent.setup();
    localStorage.setItem("appleIntelligenceDescriptions", "true");
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getByTitle("Generate with Apple Intelligence"));
    expect(mockInvoke).toHaveBeenCalledWith("generate_step_descriptions", {
      stepIds: ["step-1"],
    });
  });

  it("replacing pending delete flushes the previous one first", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
      makeStep({ id: "step-3", app: "Notes" }),
    ]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("3 steps")).toBeInTheDocument();
    });

    // Delete step-1 (soft delete)
    await user.click(screen.getAllByTitle("Remove step")[0]);
    expect(screen.getByText("Step deleted")).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("delete_step", { stepId: "step-1" });

    // Delete step-2 before step-1's timer expires — should flush step-1 immediately
    await user.click(screen.getAllByTitle("Remove step")[0]);
    expect(mockInvoke).toHaveBeenCalledWith("delete_step", { stepId: "step-1" });

    // Step-2 now pending; after timer it too gets flushed
    act(() => { vi.advanceTimersByTime(3000); });
    expect(mockInvoke).toHaveBeenCalledWith("delete_step", { stepId: "step-2" });

    vi.useRealTimers();
  });

  it("deselects a step when clicking its checkbox again without shift", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([
      makeStep({ id: "step-1", app: "Finder" }),
      makeStep({ id: "step-2", app: "Safari" }),
    ]);

    const { container } = render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("2 steps")).toBeInTheDocument();
    });

    const checkboxes = container.querySelectorAll(".editor-step-checkbox");

    // Select step-1
    await user.click(checkboxes[0]);
    expect(screen.getByText("1 selected")).toBeInTheDocument();

    // Click same checkbox again -> deselect
    await user.click(checkboxes[0]);
    expect(screen.queryByText("1 selected")).not.toBeInTheDocument();
  });
});
