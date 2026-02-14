import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { convertFileSrc } from "@tauri-apps/api/core";
import StepItem from "./StepItem";
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

describe("StepItem", () => {
  it("renders step number and description", () => {
    render(<StepItem step={makeStep()} index={0} />);
    expect(screen.getByText("Step 1")).toBeInTheDocument();
    expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
  });

  it("uses convertFileSrc for screenshot_path", () => {
    mockConvertFileSrc.mockReturnValue("asset://localhost//tmp/screenshot.png");
    render(<StepItem step={makeStep()} index={0} />);
    expect(mockConvertFileSrc).toHaveBeenCalledWith("/tmp/screenshot.png");
    const img = screen.getByAltText("Step 1");
    expect(img).toHaveAttribute(
      "src",
      "asset://localhost//tmp/screenshot.png",
    );
  });

  it("does not render image when screenshot_path is null", () => {
    render(
      <StepItem step={makeStep({ screenshot_path: null })} index={0} />,
    );
    expect(screen.queryByAltText("Step 1")).not.toBeInTheDocument();
  });

  it("shows click indicator for normal clicks", () => {
    const { container } = render(<StepItem step={makeStep()} index={0} />);
    const marker = container.querySelector(".click-indicator");
    expect(marker).toBeInTheDocument();
    expect(marker!.className).toBe("click-indicator");
  });

  it("shows double-click marker class", () => {
    const { container } = render(
      <StepItem step={makeStep({ action: "DoubleClick" })} index={0} />,
    );
    const marker = container.querySelector(".click-indicator");
    expect(marker!.className).toBe("click-indicator double-click");
  });

  it("shows right-click marker class", () => {
    const { container } = render(
      <StepItem step={makeStep({ action: "RightClick" })} index={0} />,
    );
    const marker = container.querySelector(".click-indicator");
    expect(marker!.className).toBe("click-indicator right-click");
  });

  it("shows DoubleClick description", () => {
    render(
      <StepItem step={makeStep({ action: "DoubleClick" })} index={0} />,
    );
    expect(screen.getByText("Double-clicked in Finder")).toBeInTheDocument();
  });

  it("shows RightClick description", () => {
    render(
      <StepItem step={makeStep({ action: "RightClick" })} index={0} />,
    );
    expect(screen.getByText("Right-clicked in Finder")).toBeInTheDocument();
  });

  it("hides marker for auth placeholder", () => {
    const { container } = render(
      <StepItem
        step={makeStep({
          window_title: "Authentication dialog (secure)",
          app: "Safari",
        })}
        index={0}
      />,
    );
    expect(container.querySelector(".click-indicator")).not.toBeInTheDocument();
  });

  it("shows auth placeholder description", () => {
    render(
      <StepItem
        step={makeStep({
          window_title: "Authentication dialog (secure)",
          app: "Safari",
        })}
        index={0}
      />,
    );
    expect(
      screen.getByText("Authenticate with Touch ID or enter your password to continue."),
    ).toBeInTheDocument();
  });

  it("detects auth placeholder by app name", () => {
    const { container } = render(
      <StepItem
        step={makeStep({
          window_title: "Some window",
          app: "Authentication",
        })}
        index={0}
      />,
    );
    expect(container.querySelector(".click-indicator")).not.toBeInTheDocument();
    expect(
      screen.getByText("Authenticate with Touch ID or enter your password to continue."),
    ).toBeInTheDocument();
  });

  it("does not show delete button when onDelete is not provided", () => {
    const { container } = render(
      <StepItem step={makeStep()} index={0} />,
    );
    expect(container.querySelector(".step-delete")).not.toBeInTheDocument();
  });

  it("requires two clicks to delete (confirm flow)", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    const { container } = render(
      <StepItem step={makeStep()} index={0} onDelete={onDelete} />,
    );

    const deleteBtn = container.querySelector(".step-delete")!;
    // First click enters confirming state
    await user.click(deleteBtn);
    expect(deleteBtn.className).toContain("confirming");
    expect(onDelete).not.toHaveBeenCalled();

    // Second click confirms deletion
    await user.click(deleteBtn);
    expect(onDelete).toHaveBeenCalledWith("step-1");
  });

  it("cancels confirmation on click outside", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    const { container } = render(
      <StepItem step={makeStep()} index={0} onDelete={onDelete} />,
    );

    const deleteBtn = container.querySelector(".step-delete")!;
    await user.click(deleteBtn);
    expect(deleteBtn.className).toContain("confirming");

    // Click outside
    fireEvent.mouseDown(document.body);
    expect(deleteBtn.className).not.toContain("confirming");
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("positions marker at click percentage", () => {
    const { container } = render(
      <StepItem
        step={makeStep({ click_x_percent: 75, click_y_percent: 25 })}
        index={0}
      />,
    );
    const marker = container.querySelector(".click-indicator") as HTMLElement;
    expect(marker.style.left).toBe("75%");
    expect(marker.style.top).toBe("25%");
  });

  it("remaps marker position when crop is set", () => {
    const { container } = render(
      <StepItem
        step={makeStep({
          click_x_percent: 60,
          click_y_percent: 50,
          crop_region: {
            x_percent: 40,
            y_percent: 30,
            width_percent: 40,
            height_percent: 40,
          },
        })}
        index={0}
      />,
    );
    const marker = container.querySelector(".click-indicator") as HTMLElement;
    expect(marker.style.left).toBe("50%");
    expect(marker.style.top).toBe("50%");
  });

  it("uses explicit description when provided", () => {
    render(
      <StepItem
        step={makeStep({ description: "Custom instruction", app: "Finder" })}
        index={0}
      />,
    );
    expect(screen.getByText("Custom instruction")).toBeInTheDocument();
  });

  it("renders drag handle when sortable is enabled", () => {
    const { container } = render(
      <StepItem step={makeStep()} index={0} sortable />,
    );
    expect(container.querySelector(".drag-handle")).toBeInTheDocument();
  });

  it("appends retry query on image load errors (including & branch)", async () => {
    mockConvertFileSrc.mockReturnValue("asset://localhost/screenshot.png?seed=1");
    render(<StepItem step={makeStep()} index={0} />);
    const img = screen.getByAltText("Step 1");

    expect(img).toHaveAttribute("src", "asset://localhost/screenshot.png?seed=1");
    fireEvent.error(img);
    await waitFor(() =>
      expect(screen.getByAltText("Step 1")).toHaveAttribute(
        "src",
        "asset://localhost/screenshot.png?seed=1&retry=1",
      ),
    );

    fireEvent.error(screen.getByAltText("Step 1"));
    await waitFor(() =>
      expect(screen.getByAltText("Step 1")).toHaveAttribute(
        "src",
        "asset://localhost/screenshot.png?seed=1&retry=2",
      ),
    );

    fireEvent.error(screen.getByAltText("Step 1"));
    await new Promise((resolve) => window.setTimeout(resolve, 150));
    expect(screen.getByAltText("Step 1")).toHaveAttribute(
      "src",
      "asset://localhost/screenshot.png?seed=1&retry=2",
    );
  });
});
