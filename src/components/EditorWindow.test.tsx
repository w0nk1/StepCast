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
    return vi.fn();
  });
  return listeners;
}

describe("EditorWindow", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockListen.mockReset();
    mockListen.mockResolvedValue(vi.fn());
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

  it("deletes step via delete button", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValueOnce([makeStep({ id: "step-1" })]);
    mockInvoke.mockResolvedValue(undefined);

    render(<EditorWindow />);
    await waitFor(() => {
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
    });

    await user.click(screen.getByTitle("Remove step"));
    expect(mockInvoke).toHaveBeenCalledWith("delete_step", { stepId: "step-1" });
    expect(screen.getByText("No steps recorded yet")).toBeInTheDocument();
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
});
