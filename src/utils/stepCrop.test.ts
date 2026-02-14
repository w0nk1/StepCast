import { describe, expect, it } from "vitest";
import type { Step } from "../types/step";
import { markerPositionForStep, normalizeCropRegion } from "./stepCrop";

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
});
