import { memo, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Step } from "../types/step";

type StepItemProps = {
  step: Step;
  index: number;
  onDelete?: (id: string) => void;
};

export default memo(function StepItem({ step, index, onDelete }: StepItemProps) {
  const [confirming, setConfirming] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  // Click outside cancels confirmation
  useEffect(() => {
    if (!confirming) return;
    const handler = (e: MouseEvent) => {
      if (buttonRef.current && !buttonRef.current.contains(e.target as Node)) {
        setConfirming(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [confirming]);

  // Convert local file path to tauri asset URL
  const thumbnailSrc = step.screenshot_path
    ? convertFileSrc(step.screenshot_path)
    : null;

  const isAuthPlaceholder =
    step.window_title === "Authentication dialog (secure)" ||
    step.app.toLowerCase() === "authentication";

  // Determine marker class based on action type
  const markerClass =
    step.action === "DoubleClick"
      ? "click-indicator double-click"
      : step.action === "RightClick"
        ? "click-indicator right-click"
        : "click-indicator";

  // Action description
  const actionDesc =
    step.action === "DoubleClick"
      ? "Double-clicked in"
      : step.action === "RightClick"
        ? "Right-clicked in"
        : "Clicked in";

  const description = isAuthPlaceholder
    ? "Authentication required (secure dialog)"
    : `${actionDesc} ${step.app}`;

  return (
    <div className="step-item">
      <div className="step-thumb">
        {thumbnailSrc && (
          <img src={thumbnailSrc} alt={`Step ${index + 1}`} />
        )}
        {/* Click indicator positioned using percentage */}
        {!isAuthPlaceholder && (
          <div
            className={markerClass}
            style={{
              left: `${step.click_x_percent}%`,
              top: `${step.click_y_percent}%`,
            }}
          />
        )}
      </div>
      <div className="step-content">
        <span className="step-number">Step {index + 1}</span>
        <span className="step-desc">{description}</span>
      </div>
      {onDelete && (
        <button
          ref={buttonRef}
          className={`step-delete${confirming ? " confirming" : ""}`}
          onClick={() => {
            if (confirming) {
              onDelete(step.id);
            } else {
              setConfirming(true);
            }
          }}
          title={confirming ? "Confirm delete" : "Remove step"}
        >
          {confirming ? (
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <path d="M20 6L9 17l-5-5"/>
            </svg>
          ) : (
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 6L6 18M6 6l12 12"/>
            </svg>
          )}
        </button>
      )}
    </div>
  );
});
