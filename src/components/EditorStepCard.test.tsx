import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

  it("shows shortcut description", () => {
    render(
      <EditorStepCard
        step={makeStep({ action: "Shortcut" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("Used keyboard shortcut in Finder")).toBeInTheDocument();
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

  it("supports description editing save and escape cancel", async () => {
    const user = userEvent.setup();
    const onUpdateDescription = vi.fn();
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={onUpdateDescription}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getByText("Clicked in Finder"));
    const input = screen.getByPlaceholderText("Step description...");
    await user.clear(input);
    await user.type(input, "Do it now{Enter}");
    expect(onUpdateDescription).toHaveBeenCalledWith("step-1", "Do it now");

    await user.click(screen.getByText("Clicked in Finder"));
    const input2 = screen.getByPlaceholderText("Step description...");
    await user.type(input2, "temp");
    await user.keyboard("{Escape}");
    expect(onUpdateDescription).toHaveBeenCalledTimes(1);
  });

  it("disables description editing and crop for auth placeholder", async () => {
    const user = userEvent.setup();
    render(
      <EditorStepCard
        step={makeStep({
          app: "Authentication",
          window_title: "Some secure dialog",
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

    const descButton = screen.getByRole("button", {
      name: "Authenticate with Touch ID or enter your password to continue.",
    });
    expect(descButton).toBeDisabled();
    expect(screen.queryByTitle("Adjust visible screenshot area")).not.toBeInTheDocument();
    expect(screen.getByTitle("Generate with Apple Intelligence")).toBeDisabled();
    await user.click(descButton);
    expect(screen.queryByPlaceholderText("Step description...")).not.toBeInTheDocument();
  });

  it("shows AI status pills for generating, ai and failure states", () => {
    const { rerender } = render(
      <EditorStepCard
        step={makeStep({ description_status: "generating", description_source: null })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("AI…")).toBeInTheDocument();

    rerender(
      <EditorStepCard
        step={makeStep({ description_status: null, description_source: "ai" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("AI")).toBeInTheDocument();

    rerender(
      <EditorStepCard
        step={makeStep({
          description_status: "failed",
          description_source: null,
          description_error: "model timeout",
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
    expect(screen.getByText("AI!")).toBeInTheDocument();
  });

  it("uses default failure tooltip when no description_error exists", () => {
    render(
      <EditorStepCard
        step={makeStep({
          description_status: "failed",
          description_source: null,
          description_error: null,
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
    expect(screen.getByText("AI!")).toHaveAttribute(
      "title",
      "Apple Intelligence generation failed",
    );
  });

  it("calls generate handler from AI button when enabled", async () => {
    const user = userEvent.setup();
    const onGenerateDescription = vi.fn();
    render(
      <EditorStepCard
        step={makeStep()}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={onGenerateDescription}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    await user.click(screen.getByTitle("Generate with Apple Intelligence"));
    expect(onGenerateDescription).toHaveBeenCalledWith("step-1");
  });

  it("supports crop modal close/reset/cancel actions", async () => {
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
    await user.click(screen.getByRole("button", { name: "Reset" }));
    expect(onUpdateCrop).toHaveBeenCalledWith("step-1", null);

    await user.click(screen.getByTitle("Adjust visible screenshot area"));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByText("Adjust Focus Crop")).not.toBeInTheDocument();

    await user.click(screen.getByTitle("Adjust visible screenshot area"));
    await user.click(screen.getByTitle("Close crop editor"));
    expect(screen.queryByText("Adjust Focus Crop")).not.toBeInTheDocument();

    await user.click(screen.getByTitle("Adjust visible screenshot area"));
    fireEvent.click(screen.getByText("Adjust Focus Crop").closest(".editor-crop-overlay") as Element);
    expect(screen.queryByText("Adjust Focus Crop")).not.toBeInTheDocument();
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

  it("ignores non-left-button and invalid frame sizes for crop drag start", () => {
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
    const frame = container.querySelector(".step-image-frame-cropped") as HTMLDivElement;
    Object.defineProperty(frame, "getBoundingClientRect", {
      value: () =>
        ({
          x: 0,
          y: 0,
          width: 0.5,
          height: 0.5,
          top: 0,
          left: 0,
          right: 0.5,
          bottom: 0.5,
          toJSON: () => ({}),
        }) as DOMRect,
    });

    fireEvent.pointerDown(frame, { pointerId: 1, button: 1, clientX: 100, clientY: 100 });
    fireEvent.pointerDown(frame, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerMove(frame, { pointerId: 1, clientX: 150, clientY: 150 });
    fireEvent.pointerUp(frame, { pointerId: 1, clientX: 150, clientY: 150 });
    expect(onUpdateCrop).not.toHaveBeenCalled();
  });

  it("shows retry button next to failed AI pill", async () => {
    const user = userEvent.setup();
    const onGenerateDescription = vi.fn();
    render(
      <EditorStepCard
        step={makeStep({ description_status: "failed", description_error: "timeout" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={onGenerateDescription}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    const retryBtn = screen.getByRole("button", { name: "Retry" });
    expect(retryBtn).toBeInTheDocument();
    await user.click(retryBtn);
    expect(onGenerateDescription).toHaveBeenCalledWith("step-1");
  });

  it("disables retry button when AI is not enabled", () => {
    render(
      <EditorStepCard
        step={makeStep({ description_status: "failed" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={false}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Retry" })).toBeDisabled();
  });

  it("shows note indicator when step has a note", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep({ note: "Important info here" })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    const indicator = container.querySelector(".editor-step-note-indicator");
    expect(indicator).toBeInTheDocument();
    expect(indicator).toHaveAttribute("title", "Important info here");
  });

  it("hides note indicator when step has no note", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep({ note: null })}
        index={0}
        onUpdateNote={vi.fn()}
        onUpdateDescription={vi.fn()}
        onGenerateDescription={vi.fn()}
        onUpdateCrop={vi.fn()}
        aiEnabled={true}
        onDelete={vi.fn()}
      />,
    );
    expect(container.querySelector(".editor-step-note-indicator")).not.toBeInTheDocument();
  });

  it("shows pencil icon in badge for Note action", () => {
    const { container } = render(
      <EditorStepCard
        step={makeStep({ action: "Note" })}
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
    expect(badge?.querySelector("svg")).toBeInTheDocument();
    expect(badge?.textContent?.trim()).not.toBe("1");
  });

  it("hides screenshot and note when collapsed", () => {
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
        collapsed={true}
        onToggleCollapse={vi.fn()}
      />,
    );
    expect(container.querySelector(".editor-step-image")).not.toBeInTheDocument();
    expect(container.querySelector(".editor-step-note-row")).not.toBeInTheDocument();
  });

  it("shows collapse chevron and calls toggle on click", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
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
        collapsed={false}
        onToggleCollapse={onToggle}
      />,
    );
    await user.click(screen.getByTitle("Collapse step"));
    expect(onToggle).toHaveBeenCalledWith("step-1");
  });

  it("shows expand button on image wrapper", () => {
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
    expect(container.querySelector(".editor-image-expand")).toBeInTheDocument();
  });

  it("opens lightbox on expand button click", async () => {
    const user = userEvent.setup();
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
    await user.click(screen.getByTitle("View full size"));
    expect(container.querySelector(".editor-lightbox-overlay")).toBeInTheDocument();
    await user.click(screen.getByTitle("Close lightbox"));
    expect(container.querySelector(".editor-lightbox-overlay")).not.toBeInTheDocument();
  });

  it("shows action sub-badge for DoubleClick", () => {
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
    expect(container.querySelector(".editor-badge-action")).toBeInTheDocument();
  });

  it("shows checkbox and calls onToggleSelect", async () => {
    const user = userEvent.setup();
    const onToggleSelect = vi.fn();
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
        onToggleSelect={onToggleSelect}
        isSelected={false}
        isSelectionActive={false}
      />,
    );
    const checkbox = container.querySelector(".editor-step-checkbox") as HTMLElement;
    expect(checkbox).toBeInTheDocument();
    await user.click(checkbox);
    expect(onToggleSelect).toHaveBeenCalledWith("step-1", false);
  });

  it("shows checked state when selected", () => {
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
        onToggleSelect={vi.fn()}
        isSelected={true}
        isSelectionActive={true}
      />,
    );
    expect(container.querySelector(".editor-step-checkbox.is-checked")).toBeInTheDocument();
    expect(container.querySelector(".editor-step.is-selected")).toBeInTheDocument();
  });

  it("retries image loading on error up to max retries", async () => {
    mockConvertFileSrc.mockReturnValue("asset://localhost//tmp/screenshot.png");
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
    const img = screen.getByAltText("Step 1");
    expect(img).toHaveAttribute("src", "asset://localhost//tmp/screenshot.png");

    fireEvent.error(img);
    await waitFor(() =>
      expect(screen.getByAltText("Step 1")).toHaveAttribute(
        "src",
        "asset://localhost//tmp/screenshot.png?retry=1",
      ),
    );

    fireEvent.error(screen.getByAltText("Step 1"));
    await waitFor(() =>
      expect(screen.getByAltText("Step 1")).toHaveAttribute(
        "src",
        "asset://localhost//tmp/screenshot.png?retry=2",
      ),
    );

    fireEvent.error(screen.getByAltText("Step 1"));
    await new Promise((resolve) => window.setTimeout(resolve, 150));
    expect(screen.getByAltText("Step 1")).toHaveAttribute(
      "src",
      "asset://localhost//tmp/screenshot.png?retry=2",
    );

    const frame = container.querySelector(".step-image-frame") as HTMLElement;
    Object.defineProperty(frame, "getBoundingClientRect", {
      value: () =>
        ({
          x: 0,
          y: 0,
          width: 640,
          height: 360,
          top: 0,
          left: 0,
          right: 640,
          bottom: 360,
          toJSON: () => ({}),
        }) as DOMRect,
    });
    const loaded = screen.getByAltText("Step 1") as HTMLImageElement;
    Object.defineProperty(loaded, "naturalWidth", { value: 1920, configurable: true });
    Object.defineProperty(loaded, "naturalHeight", { value: 1080, configurable: true });
    fireEvent.load(loaded);
  });
});
