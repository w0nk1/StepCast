import { describe, expect, it } from "vitest";
import type { Step } from "../types/step";
import {
  getCropAspectRatio,
  getCroppedImageStyles,
  isFullCrop,
  markerPositionForStep,
  normalizeCropRegion,
} from "./stepCrop";

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

describe("stepCrop", () => {
  it("normalizes and clamps crop bounds", () => {
    const crop = normalizeCropRegion({
      x_percent: -10,
      y_percent: 10,
      width_percent: 120,
      height_percent: 95,
    });
    expect(crop).toEqual({
      x_percent: 0,
      y_percent: 10,
      width_percent: 100,
      height_percent: 90,
    });
  });

  it("returns null for invalid/non-finite crop data", () => {
    expect(normalizeCropRegion()).toBeNull();
    expect(
      normalizeCropRegion({
        x_percent: Number.NaN,
        y_percent: 0,
        width_percent: 50,
        height_percent: 50,
      }),
    ).toBeNull();
  });

  it("returns null when crop is smaller than minimum size", () => {
    expect(
      normalizeCropRegion({
        x_percent: 10,
        y_percent: 10,
        width_percent: 1,
        height_percent: 50,
      }),
    ).toBeNull();
  });

  it("returns full marker position when no crop", () => {
    const marker = markerPositionForStep(makeStep({ click_x_percent: 70, click_y_percent: 30 }));
    expect(marker).toEqual({ xPercent: 70, yPercent: 30, visible: true });
  });

  it("remaps marker position within crop", () => {
    const marker = markerPositionForStep(
      makeStep({
        click_x_percent: 50,
        click_y_percent: 50,
        crop_region: {
          x_percent: 25,
          y_percent: 25,
          width_percent: 50,
          height_percent: 50,
        },
      }),
    );
    expect(marker.visible).toBe(true);
    expect(marker.xPercent).toBe(50);
    expect(marker.yPercent).toBe(50);
  });

  it("hides marker when click is outside crop", () => {
    const marker = markerPositionForStep(
      makeStep({
        click_x_percent: 90,
        click_y_percent: 90,
        crop_region: {
          x_percent: 10,
          y_percent: 10,
          width_percent: 30,
          height_percent: 30,
        },
      }),
    );
    expect(marker.visible).toBe(false);
  });

  it("detects full crop correctly", () => {
    expect(isFullCrop(null)).toBe(true);
    expect(
      isFullCrop({
        x_percent: 0,
        y_percent: 0,
        width_percent: 100,
        height_percent: 100,
      }),
    ).toBe(true);
    expect(
      isFullCrop({
        x_percent: 1,
        y_percent: 0,
        width_percent: 99,
        height_percent: 100,
      }),
    ).toBe(false);
  });

  it("returns default styles for full crop and transformed styles for partial crop", () => {
    const full = getCroppedImageStyles(null);
    expect(full.frameClassName).toBe("step-image-frame");
    expect(full.imageStyle).toEqual({});

    const partial = getCroppedImageStyles({
      x_percent: 10,
      y_percent: 20,
      width_percent: 50,
      height_percent: 40,
    });
    expect(partial.frameClassName).toContain("step-image-frame-cropped");
    expect(partial.imageStyle).toMatchObject({
      position: "absolute",
      left: 0,
      top: 0,
      maxWidth: "none",
      transform: "translate(-10%, -20%)",
      transformOrigin: "top left",
    });
    expect(partial.imageStyle.width).toBe("200%");
    expect(partial.imageStyle.height).toBe("250%");
  });

  it("computes aspect ratio for image and crop variants", () => {
    expect(getCropAspectRatio(0, 100, null)).toBeNull();
    expect(getCropAspectRatio(200, 100, null)).toBe(2);
    expect(
      getCropAspectRatio(1000, 500, {
        x_percent: 0,
        y_percent: 0,
        width_percent: 50,
        height_percent: 50,
      }),
    ).toBe(2);
    expect(
      getCropAspectRatio(1000, 500, {
        x_percent: 0,
        y_percent: 0,
        width_percent: 1,
        height_percent: 50,
      }),
    ).toBe(2);
  });
});
