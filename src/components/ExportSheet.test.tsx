import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ExportSheet from "./ExportSheet";

const defaultProps = {
  stepCount: 3,
  exporting: false,
  onExport: vi.fn(),
  onClose: vi.fn(),
};

function renderSheet(overrides: Partial<typeof defaultProps> = {}) {
  const props = { ...defaultProps, ...overrides };
  // Reset mocks each render
  props.onExport = overrides.onExport ?? vi.fn();
  props.onClose = overrides.onClose ?? vi.fn();
  return { ...render(<ExportSheet {...props} />), props };
}

describe("ExportSheet", () => {
  it("renders title input and format options", () => {
    renderSheet();
    expect(screen.getByDisplayValue("New StepCast Guide")).toBeInTheDocument();
    expect(screen.getByText("HTML")).toBeInTheDocument();
    expect(screen.getByText("MD")).toBeInTheDocument();
    expect(screen.getByText("PDF")).toBeInTheDocument();
  });

  it("shows step count", () => {
    renderSheet({ stepCount: 5 });
    expect(screen.getByText("5 steps")).toBeInTheDocument();
  });

  it("shows singular step count", () => {
    renderSheet({ stepCount: 1 });
    expect(screen.getByText("1 step")).toBeInTheDocument();
  });

  it("defaults format to pdf when localStorage is empty", () => {
    renderSheet();
    const pdfButton = screen.getByText("PDF");
    expect(pdfButton.className).toContain("active");
  });

  it("loads format from localStorage", () => {
    localStorage.setItem("exportFormat", "html");
    renderSheet();
    const htmlButton = screen.getByText("HTML");
    expect(htmlButton.className).toContain("active");
  });

  it("updates localStorage when format changes", async () => {
    const user = userEvent.setup();
    renderSheet();
    await user.click(screen.getByText("MD"));
    expect(localStorage.getItem("exportFormat")).toBe("md");
    expect(screen.getByText("MD").className).toContain("active");
  });

  it("disables export when title is empty", async () => {
    const user = userEvent.setup();
    renderSheet();
    const input = screen.getByDisplayValue("New StepCast Guide");
    await user.clear(input);
    expect(screen.getByText("Export")).toBeDisabled();
  });

  it("passes trimmed title to onExport", async () => {
    const user = userEvent.setup();
    const { props } = renderSheet();
    const input = screen.getByDisplayValue("New StepCast Guide");
    await user.clear(input);
    await user.type(input, "  My Guide  ");
    await user.click(screen.getByText("Export"));
    expect(props.onExport).toHaveBeenCalledWith("My Guide", "pdf");
  });

  it("calls onClose when overlay is clicked", async () => {
    const user = userEvent.setup();
    const { props } = renderSheet();
    // Click the overlay (outermost div)
    const overlay = document.querySelector(".export-overlay")!;
    await user.click(overlay);
    expect(props.onClose).toHaveBeenCalled();
  });

  it("does not call onClose when exporting", async () => {
    const user = userEvent.setup();
    const { props } = renderSheet({ exporting: true });
    const overlay = document.querySelector(".export-overlay")!;
    await user.click(overlay);
    expect(props.onClose).not.toHaveBeenCalled();
  });

  it("disables inputs while exporting", () => {
    renderSheet({ exporting: true });
    expect(screen.getByDisplayValue("New StepCast Guide")).toBeDisabled();
    expect(screen.getByText("Exporting...")).toBeDisabled();
    expect(screen.getByText("Cancel")).toBeDisabled();
  });
});
