import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { convertFileSrc } from "@tauri-apps/api/core";
import EditorStepCard from "./EditorStepCard";
import type { Step } from "../types/step";

const mockConvertFileSrc = vi.mocked(convertFileSrc);

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

describe("EditorStepCard", () => {
  it("renders timeline badge and description", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    const badge = container.querySelector(".editor-timeline-badge");
    expect(badge).toHaveTextContent("1");
    expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
  });

  it("renders screenshot with convertFileSrc", () => {
    mockConvertFileSrc.mockReturnValue("asset://localhost//tmp/screenshot.png");
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(mockConvertFileSrc).toHaveBeenCalledWith("/tmp/screenshot.png");
    const img = screen.getByAltText("Step 1");
    expect(img).toHaveAttribute("src", "asset://localhost//tmp/screenshot.png");
  });

  it("shows 'Add a note...' placeholder when no note", () => {
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("Add a note...")).toBeInTheDocument();
  });

  it("shows existing note text", () => {
    render(
      <EditorStepCard
        step={makeStep({ note: "My note" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("My note")).toBeInTheDocument();
  });

  it("enters editing mode on click and saves on blur", async () => {
    const user = userEvent.setup();
    const onUpdateNote = vi.fn();
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={onUpdateNote}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getByText("Add a note..."));
    const textarea = screen.getByPlaceholderText("Add a note...");
    await user.type(textarea, "New note");
    await user.tab(); // blur
    expect(onUpdateNote).toHaveBeenCalledWith("step-1", "New note");
  });

  it("saves on Enter", async () => {
    const user = userEvent.setup();
    const onUpdateNote = vi.fn();
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={onUpdateNote}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getByText("Add a note..."));
    const textarea = screen.getByPlaceholderText("Add a note...");
    await user.type(textarea, "Note text{Enter}");
    expect(onUpdateNote).toHaveBeenCalledWith("step-1", "Note text");
  });

  it("cancels editing on Escape", async () => {
    const user = userEvent.setup();
    const onUpdateNote = vi.fn();
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={onUpdateNote}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getByText("Add a note..."));
    const textarea = screen.getByPlaceholderText("Add a note...");
    await user.type(textarea, "Temp");
    await user.keyboard("{Escape}");
    expect(onUpdateNote).not.toHaveBeenCalled();
    // Button should be back
    expect(screen.getByText("Add a note...")).toBeInTheDocument();
  });

  it("shows click marker at correct position", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep({ click_x_percent: 75, click_y_percent: 25 })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    const marker = container.querySelector(".click-indicator") as HTMLElement;
    expect(marker.style.left).toBe("75%");
    expect(marker.style.top).toBe("25%");
  });

  it("hides marker for auth placeholder", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep({ window_title: "Authentication dialog (secure)" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(container.querySelector(".click-indicator")).not.toBeInTheDocument();
  });

  it("shows double-click description and marker", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep({ action: "DoubleClick" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("Double-clicked in Finder")).toBeInTheDocument();
    expect(container.querySelector(".click-indicator.double-click")).toBeInTheDocument();
  });

  it("shows right-click description and marker", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep({ action: "RightClick" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("Right-clicked in Finder")).toBeInTheDocument();
    expect(container.querySelector(".click-indicator.right-click")).toBeInTheDocument();
  });

  it("calls onDelete when delete button clicked", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={onDelete}
      />,
    );

    await user.click(screen.getByTitle("Remove step"));
    expect(onDelete).toHaveBeenCalledWith("step-1");
  });

  it("opens crop editor and applies crop", async () => {
    const user = userEvent.setup();
    const onUpdateCrop = vi.fn();
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={onUpdateCrop}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getByTitle("Adjust visible screenshot area"));
    expect(screen.getByText("Adjust Focus Crop")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Apply" }));
    expect(onUpdateCrop).toHaveBeenCalledWith("step-1", null);
  });

  it("renders a stable cropped frame when crop region is present", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep({
          crop_region: {
            x_percent: 10,
            y_percent: 10,
            width_percent: 60,
            height_percent: 60,
          },
        })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );

    const frame = container.querySelector(".step-image-frame-cropped") as HTMLElement | null;
    expect(frame).toBeInTheDocument();
    expect(frame?.style.aspectRatio).not.toBe("");
  });

  it("repositions an existing crop by dragging in the step preview", () => {
    const onUpdateCrop = vi.fn();
    const { container } = render(
      <EditorStepCard
        step={makeStep({
          crop_region: {
            x_percent: 20,
            y_percent: 20,
            width_percent: 50,
            height_percent: 50,
          },
        })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={onUpdateCrop}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );

    const frame = container.querySelector(".step-image-frame-cropped") as HTMLDivElement | null;
    expect(frame).toBeInTheDocument();
    if (!frame) return;

    Object.defineProperty(frame, "getBoundingClientRect", {
      value: () =>
        ({
          x: 0,
          y: 0,
          width: 500,
          height: 250,
          top: 0,
          left: 0,
          right: 500,
          bottom: 250,
          toJSON: () => ({}),
        }) as DOMRect,
    });

    fireEvent.pointerDown(frame, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerMove(frame, { pointerId: 1, clientX: 150, clientY: 120 });
    fireEvent.pointerUp(frame, { pointerId: 1, clientX: 150, clientY: 120 });

    expect(onUpdateCrop).toHaveBeenCalledTimes(1);
    const [, cropArg] = onUpdateCrop.mock.calls[0];
    expect(cropArg).toMatchObject({
      width_percent: 50,
      height_percent: 50,
    });
    expect(cropArg.x_percent).toBeLessThan(20);
    expect(cropArg.y_percent).toBeLessThan(20);
  });

  it("does not persist crop when pointer is pressed/released without movement", () => {
    const onUpdateCrop = vi.fn();
    const { container } = render(
      <EditorStepCard
        step={makeStep({
          crop_region: {
            x_percent: 20,
            y_percent: 20,
            width_percent: 50,
            height_percent: 50,
          },
        })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={onUpdateCrop}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );

    const frame = container.querySelector(".step-image-frame-cropped") as HTMLDivElement | null;
    expect(frame).toBeInTheDocument();
    if (!frame) return;

    Object.defineProperty(frame, "getBoundingClientRect", {
      value: () =>
        ({
          x: 0,
          y: 0,
          width: 500,
          height: 250,
          top: 0,
          left: 0,
          right: 500,
          bottom: 250,
          toJSON: () => ({}),
        }) as DOMRect,
    });

    fireEvent.pointerDown(frame, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerUp(frame, { pointerId: 1, clientX: 100, clientY: 100 });

    expect(onUpdateCrop).not.toHaveBeenCalled();
  });
});
