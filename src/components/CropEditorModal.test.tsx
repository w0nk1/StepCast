import { describe, it, expect, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CropEditorModal, { toBoundsPercent, toPercentCrop } from "./CropEditorModal";
import type { BoundsPercent } from "../types/step";

let mockCropOnChange: ((...args: any[]) => void) | null = null;

vi.mock("react-image-crop", () => ({
  default: ({ children, onChange }: any) => {
    mockCropOnChange = onChange;
    return (
      <div
        data-testid="react-crop"
        onClick={() =>
          onChange?.({}, { unit: "%", x: 10, y: 20, width: 60, height: 70 })
        }
      >
        {children}
      </div>
    );
  },
}));

const defaultProps = {
  screenshotSrc: "asset://localhost/tmp/shot.png",
  stepIndex: 0,
  initialCropRegion: null as BoundsPercent | null | undefined,
  clickXPercent: undefined as number | undefined,
  clickYPercent: undefined as number | undefined,
  action: undefined as string | undefined,
  onSave: vi.fn(),
  onReset: vi.fn(),
  onClose: vi.fn(),
  onImageError: vi.fn(),
  onImageLoad: vi.fn(),
};

function renderModal(overrides: Partial<typeof defaultProps> = {}) {
  const props = {
    ...defaultProps,
    onSave: vi.fn(),
    onReset: vi.fn(),
    onClose: vi.fn(),
    onImageError: vi.fn(),
    onImageLoad: vi.fn(),
    ...overrides,
  };
  const result = render(<CropEditorModal {...props} />);
  return { ...result, props };
}

// ---------------------------------------------------------------------------
// Unit tests for toPercentCrop / toBoundsPercent
// ---------------------------------------------------------------------------

describe("toPercentCrop", () => {
  it("returns full crop for null input", () => {
    expect(toPercentCrop(null)).toEqual({
      unit: "%",
      x: 0,
      y: 0,
      width: 100,
      height: 100,
    });
  });

  it("returns full crop for undefined input", () => {
    expect(toPercentCrop(undefined)).toEqual({
      unit: "%",
      x: 0,
      y: 0,
      width: 100,
      height: 100,
    });
  });

  it("converts valid BoundsPercent to PercentCrop", () => {
    const bounds: BoundsPercent = {
      x_percent: 10,
      y_percent: 20,
      width_percent: 60,
      height_percent: 50,
    };
    expect(toPercentCrop(bounds)).toEqual({
      unit: "%",
      x: 10,
      y: 20,
      width: 60,
      height: 50,
    });
  });

  it("returns full crop for tiny region that normalizes to null", () => {
    // width < 2% => normalizeCropRegion returns null
    const bounds: BoundsPercent = {
      x_percent: 0,
      y_percent: 0,
      width_percent: 1,
      height_percent: 1,
    };
    expect(toPercentCrop(bounds)).toEqual({
      unit: "%",
      x: 0,
      y: 0,
      width: 100,
      height: 100,
    });
  });
});

describe("toBoundsPercent", () => {
  it("returns null for a full crop (0,0,100,100)", () => {
    expect(
      toBoundsPercent({ unit: "%", x: 0, y: 0, width: 100, height: 100 }),
    ).toBeNull();
  });

  it("returns BoundsPercent for a partial crop", () => {
    const result = toBoundsPercent({
      unit: "%",
      x: 10,
      y: 20,
      width: 60,
      height: 50,
    });
    expect(result).toEqual({
      x_percent: 10,
      y_percent: 20,
      width_percent: 60,
      height_percent: 50,
    });
  });

  it("returns null when crop is too small to normalize", () => {
    expect(
      toBoundsPercent({ unit: "%", x: 0, y: 0, width: 0.5, height: 0.5 }),
    ).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Component tests for CropEditorModal
// ---------------------------------------------------------------------------

describe("CropEditorModal", () => {
  it("renders the modal with title and buttons", () => {
    renderModal();
    expect(screen.getByText("Adjust Focus Crop")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Apply" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reset" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
  });

  it("renders the image with correct alt text", () => {
    renderModal({ stepIndex: 2 });
    const img = screen.getByAltText("Adjust crop for step 3");
    expect(img).toHaveAttribute("src", "asset://localhost/tmp/shot.png");
  });

  it("calls onSave with toBoundsPercent of current crop when Apply clicked", async () => {
    const user = userEvent.setup();
    const { props } = renderModal();

    // Default crop is full (null initial) => Apply should call onSave(null)
    await user.click(screen.getByRole("button", { name: "Apply" }));
    expect(props.onSave).toHaveBeenCalledWith(null);
  });

  it("calls onSave with partial crop after ReactCrop onChange", async () => {
    const user = userEvent.setup();
    const { props } = renderModal();

    // Simulate a crop change via the mock ReactCrop (click triggers onChange)
    fireEvent.click(screen.getByTestId("react-crop"));

    await user.click(screen.getByRole("button", { name: "Apply" }));
    // The mock onChange gives {x:10, y:20, width:60, height:70}
    expect(props.onSave).toHaveBeenCalledWith({
      x_percent: 10,
      y_percent: 20,
      width_percent: 60,
      height_percent: 70,
    });
  });

  it("calls onReset and resets crop to full when Reset clicked", async () => {
    const user = userEvent.setup();
    const { props } = renderModal({
      initialCropRegion: {
        x_percent: 10,
        y_percent: 10,
        width_percent: 50,
        height_percent: 50,
      },
    });

    await user.click(screen.getByRole("button", { name: "Reset" }));
    expect(props.onReset).toHaveBeenCalled();

    // After reset, Apply should give null (full crop)
    await user.click(screen.getByRole("button", { name: "Apply" }));
    expect(props.onSave).toHaveBeenCalledWith(null);
  });

  it("calls onClose when Cancel clicked", async () => {
    const user = userEvent.setup();
    const { props } = renderModal();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(props.onClose).toHaveBeenCalled();
  });

  it("calls onClose when close button (x) clicked", async () => {
    const user = userEvent.setup();
    const { props } = renderModal();
    await user.click(screen.getByTitle("Close crop editor"));
    expect(props.onClose).toHaveBeenCalled();
  });

  it("calls onClose when overlay backdrop clicked", () => {
    const { props, container } = renderModal();
    const overlay = container.querySelector(".editor-crop-overlay") as HTMLElement;
    fireEvent.click(overlay);
    expect(props.onClose).toHaveBeenCalled();
  });

  it("does not call onClose when clicking inside the modal dialog", () => {
    const { props } = renderModal();
    const dialog = screen.getByRole("dialog");
    fireEvent.click(dialog);
    expect(props.onClose).not.toHaveBeenCalled();
  });

  it("calls onImageLoad when image loads with valid dimensions", () => {
    const { props } = renderModal();
    const img = screen.getByAltText("Adjust crop for step 1");
    Object.defineProperty(img, "naturalWidth", { value: 1920, configurable: true });
    Object.defineProperty(img, "naturalHeight", { value: 1080, configurable: true });
    fireEvent.load(img);
    expect(props.onImageLoad).toHaveBeenCalledWith(1920, 1080);
  });

  it("does not call onImageLoad when image has zero dimensions", () => {
    const { props } = renderModal();
    const img = screen.getByAltText("Adjust crop for step 1");
    Object.defineProperty(img, "naturalWidth", { value: 0, configurable: true });
    Object.defineProperty(img, "naturalHeight", { value: 0, configurable: true });
    fireEvent.load(img);
    expect(props.onImageLoad).not.toHaveBeenCalled();
  });

  it("calls onImageError when image fails to load", () => {
    const { props } = renderModal();
    const img = screen.getByAltText("Adjust crop for step 1");
    fireEvent.error(img);
    expect(props.onImageError).toHaveBeenCalled();
  });

  it("renders click indicator when clickXPercent and clickYPercent provided", () => {
    const { container } = renderModal({
      clickXPercent: 45,
      clickYPercent: 55,
    });
    const indicator = container.querySelector(".click-indicator") as HTMLElement;
    expect(indicator).toBeInTheDocument();
    expect(indicator.style.left).toBe("45%");
    expect(indicator.style.top).toBe("55%");
  });

  it("does not render click indicator when clickXPercent/clickYPercent absent", () => {
    const { container } = renderModal();
    expect(container.querySelector(".click-indicator")).not.toBeInTheDocument();
  });

  it("renders double-click class for DoubleClick action", () => {
    const { container } = renderModal({
      clickXPercent: 50,
      clickYPercent: 50,
      action: "DoubleClick",
    });
    expect(
      container.querySelector(".click-indicator.double-click"),
    ).toBeInTheDocument();
  });

  it("renders right-click class for RightClick action", () => {
    const { container } = renderModal({
      clickXPercent: 50,
      clickYPercent: 50,
      action: "RightClick",
    });
    expect(
      container.querySelector(".click-indicator.right-click"),
    ).toBeInTheDocument();
  });

  it("renders default click-indicator class for Click action", () => {
    const { container } = renderModal({
      clickXPercent: 50,
      clickYPercent: 50,
      action: "Click",
    });
    const indicator = container.querySelector(".click-indicator") as HTMLElement;
    expect(indicator).toBeInTheDocument();
    expect(indicator.classList.contains("double-click")).toBe(false);
    expect(indicator.classList.contains("right-click")).toBe(false);
  });

  it("initializes crop from initialCropRegion prop", async () => {
    const user = userEvent.setup();
    const { props } = renderModal({
      initialCropRegion: {
        x_percent: 15,
        y_percent: 25,
        width_percent: 40,
        height_percent: 50,
      },
    });

    // Apply without changing the crop => should return the initial crop
    await user.click(screen.getByRole("button", { name: "Apply" }));
    expect(props.onSave).toHaveBeenCalledWith({
      x_percent: 15,
      y_percent: 25,
      width_percent: 40,
      height_percent: 50,
    });
  });

  it("uses fallback values when onChange provides nullish crop fields", async () => {
    const user = userEvent.setup();
    const { props } = renderModal();

    // Trigger onChange with undefined x/y/width/height to hit ?? fallback branches
    act(() => {
      mockCropOnChange?.({}, { unit: "%", x: undefined, y: undefined, width: undefined, height: undefined });
    });

    await user.click(screen.getByRole("button", { name: "Apply" }));
    // Fallbacks: x=0, y=0, width=100, height=100 => full crop => null
    expect(props.onSave).toHaveBeenCalledWith(null);
  });

  it("prevents native drag on image", () => {
    renderModal();
    const img = screen.getByAltText("Adjust crop for step 1");
    const prevented = !fireEvent.dragStart(img);
    expect(prevented).toBe(true);
  });
});
