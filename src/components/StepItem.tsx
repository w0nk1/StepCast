import { memo, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { Step } from "../types/step";
import { getCroppedImageStyles, markerPositionForStep } from "../utils/stepCrop";
import { useI18n } from "../i18n";

type StepItemProps = {
  step: Step;
  index: number;
  onDelete?: (id: string) => void;
  sortable?: boolean;
};

export default memo(function StepItem({ step, index, onDelete, sortable }: StepItemProps) {
  const { t } = useI18n();
  const [confirming, setConfirming] = useState(false);
  const [thumbRetry, setThumbRetry] = useState(0);
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

  // Reset image retry state when the screenshot file changes.
  useEffect(() => {
    setThumbRetry(0);
  }, [step.screenshot_path]);

  // Convert local file path to tauri asset URL.
  const thumbnailBaseSrc = useMemo(
    () => (step.screenshot_path ? convertFileSrc(step.screenshot_path) : null),
    [step.screenshot_path],
  );
  const thumbnailSrc = useMemo(() => {
    if (!thumbnailBaseSrc) return null;
    if (thumbRetry === 0) return thumbnailBaseSrc;
    const sep = thumbnailBaseSrc.includes("?") ? "&" : "?";
    return `${thumbnailBaseSrc}${sep}retry=${thumbRetry}`;
  }, [thumbnailBaseSrc, thumbRetry]);

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
  const marker = markerPositionForStep(step);
  const cropStyles = getCroppedImageStyles(step.crop_region);
  const handleImageError = () => {
    if (thumbRetry >= 2) return;
    window.setTimeout(() => {
      setThumbRetry((prev) => (prev < 2 ? prev + 1 : prev));
    }, 120);
  };

  // Action description
  const actionDesc =
    step.action === "DoubleClick"
      ? t("step.action.double_clicked_in")
      : step.action === "RightClick"
        ? t("step.action.right_clicked_in")
        : t("step.action.clicked_in");

  const authDescription =
    step.description && step.description.trim().length > 0
      ? step.description.trim()
      : t("step.auth.default");

  const description = isAuthPlaceholder
    ? authDescription
    : (step.description && step.description.trim().length > 0 ? step.description : `${actionDesc} ${step.app}`);

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: step.id, disabled: !sortable });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : undefined,
  };

  return (
    <div className="step-item" ref={setNodeRef} style={style} {...attributes}>
      {sortable && (
        <button className="drag-handle" {...listeners} title={t("step.drag_reorder_title")}>
          <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor">
            <circle cx="3" cy="2" r="1.5"/>
            <circle cx="7" cy="2" r="1.5"/>
            <circle cx="3" cy="7" r="1.5"/>
            <circle cx="7" cy="7" r="1.5"/>
            <circle cx="3" cy="12" r="1.5"/>
            <circle cx="7" cy="12" r="1.5"/>
          </svg>
        </button>
      )}
      <div className="step-thumb">
        {thumbnailSrc && (
          <div className={cropStyles.frameClassName}>
            <img
              src={thumbnailSrc}
              alt={t("step.image_alt", { num: index + 1 })}
              loading="lazy"
              decoding="async"
              draggable={false}
              style={cropStyles.imageStyle}
              onError={handleImageError}
            />
          </div>
        )}
        {/* Click indicator positioned using percentage */}
        {!isAuthPlaceholder && marker.visible && (
          <div
            className={markerClass}
            style={{
              left: `${marker.xPercent}%`,
              top: `${marker.yPercent}%`,
            }}
          />
        )}
      </div>
      <div className="step-content">
        <span className="step-number">{t("step.number", { num: index + 1 })}</span>
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
          title={confirming ? t("step.delete.confirm_title") : t("step.delete.remove_title")}
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
