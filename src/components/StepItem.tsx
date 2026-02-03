import { convertFileSrc } from "@tauri-apps/api/core";
import type { Step } from "../types/step";

type StepItemProps = {
  step: Step;
  index: number;
};

export default function StepItem({ step, index }: StepItemProps) {
  // Convert local file path to tauri asset URL
  const thumbnailSrc = step.screenshot_path
    ? convertFileSrc(step.screenshot_path)
    : null;

  return (
    <div className="step-item">
      <div className="step-thumb">
        {thumbnailSrc && (
          <img src={thumbnailSrc} alt={`Step ${index + 1}`} />
        )}
        {/* Click indicator positioned using percentage */}
        <div
          className="click-indicator"
          style={{
            left: `${step.click_x_percent}%`,
            top: `${step.click_y_percent}%`,
          }}
        />
      </div>
      <div className="step-content">
        <span className="step-number">Step {index + 1}</span>
        <span className="step-desc">
          {step.app} - "{step.window_title}"
        </span>
      </div>
    </div>
  );
}
