import { describe, expect, it } from "vitest";
import type { Step } from "../types/step";
import { mergeUpdatedStep } from "./stepEvents";

function makeStep(overrides: Partial<Step> = {}): Step {
  return {
    id: "step-1",
    ts: Date.now(),
    action: "Click",
    x: 10,
    y: 20,
    click_x_percent: 10,
    click_y_percent: 20,
    app: "Arc",
    window_title: "Pepper",
    screenshot_path: "/tmp/step-1.png",
    note: null,
    ...overrides,
  };
}

describe("mergeUpdatedStep", () => {
  it("keeps previous screenshot_path when incoming payload omits it", () => {
    const previous = makeStep({ screenshot_path: "/tmp/prev.png" });
    const incoming = {
      ...makeStep({ screenshot_path: null, note: "updated" }),
      screenshot_path: undefined as unknown as string | null,
    } as unknown as Step;

    const merged = mergeUpdatedStep(previous, incoming);
    expect(merged.screenshot_path).toBe("/tmp/prev.png");
    expect(merged.note).toBe("updated");
  });

  it("keeps previous screenshot_path when incoming clears it without failed capture", () => {
    const previous = makeStep({ screenshot_path: "/tmp/prev.png" });
    const incoming = makeStep({ screenshot_path: null, capture_status: "Ok" });

    const merged = mergeUpdatedStep(previous, incoming);
    expect(merged.screenshot_path).toBe("/tmp/prev.png");
  });

  it("respects null screenshot_path when capture explicitly failed", () => {
    const previous = makeStep({ screenshot_path: "/tmp/prev.png" });
    const incoming = makeStep({ screenshot_path: null, capture_status: "Failed" });

    const merged = mergeUpdatedStep(previous, incoming);
    expect(merged.screenshot_path).toBeNull();
    expect(merged.capture_status).toBe("Failed");
  });
});
