import type { Step } from "../types/step";

function hasOwn(obj: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(obj, key);
}

// Defensive merge for backend "step-updated" payloads.
// If an update accidentally arrives without screenshot_path (or with null while not failed),
// keep the previously known screenshot so thumbnails do not disappear.
export function mergeUpdatedStep(previous: Step, incoming: Step): Step {
  const merged: Step = { ...previous, ...incoming };

  const screenshotMissing = !hasOwn(incoming as unknown as object, "screenshot_path");
  const screenshotClearedUnexpectedly =
    incoming.screenshot_path == null &&
    previous.screenshot_path != null &&
    incoming.capture_status !== "Failed";

  if (screenshotMissing || screenshotClearedUnexpectedly) {
    merged.screenshot_path = previous.screenshot_path;
  }

  return merged;
}
