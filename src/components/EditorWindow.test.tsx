import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import EditorWindow from "./EditorWindow";
import type { Step } from "../types/step";

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
});
