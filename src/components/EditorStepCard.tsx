import { memo, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import ReactCrop, { type PercentCrop } from "react-image-crop";
import type { BoundsPercent, Step } from "../types/step";
import {
  getCropAspectRatio,
  getCroppedImageStyles,
  isFullCrop,
  markerPositionForStep,
  normalizeCropRegion,
} from "../utils/stepCrop";

type EditorStepCardProps = {
  step: Step;
  index: number;
  onUpdateNote: (stepId: string, note: string | null) => void;
  onUpdateDescription: (stepId: string, description: string | null) => void;
  onGenerateDescription: (stepId: string) => void;
  onUpdateCrop: (stepId: string, cropRegion: BoundsPercent | null) => void;
  aiEnabled: boolean;
  onDelete: (stepId: string) => void;
};

const FULL_PERCENT_CROP: PercentCrop = {
  unit: "%",
  x: 0,
  y: 0,
  width: 100,
  height: 100,
};

function toPercentCrop(cropRegion?: BoundsPercent | null): PercentCrop {
  const crop = normalizeCropRegion(cropRegion);
  if (!crop) return { ...FULL_PERCENT_CROP };
  return {
    unit: "%",
    x: crop.x_percent,
    y: crop.y_percent,
    width: crop.width_percent,
    height: crop.height_percent,
  };
}

function toBoundsPercent(crop: PercentCrop): BoundsPercent | null {
  const normalized = normalizeCropRegion({
    x_percent: crop.x ?? 0,
    y_percent: crop.y ?? 0,
    width_percent: crop.width ?? 0,
    height_percent: crop.height ?? 0,
  });
  if (!normalized) return null;
  return isFullCrop(normalized) ? null : normalized;
}

export default memo(function EditorStepCard({
  step,
  index,
  onUpdateNote,
  onUpdateDescription,
  onGenerateDescription,
  onUpdateCrop,
  aiEnabled,
  onDelete,
}: EditorStepCardProps) {
  const [noteEditing, setNoteEditing] = useState(false);
  const [noteDraft, setNoteDraft] = useState(step.note ?? "");
  const noteTextareaRef = useRef<HTMLTextAreaElement>(null);

  const [descEditing, setDescEditing] = useState(false);
  const [descDraft, setDescDraft] = useState("");
  const descInputRef = useRef<HTMLInputElement>(null);

  const [cropOpen, setCropOpen] = useState(false);
  const [cropDraft, setCropDraft] = useState<PercentCrop>(() => toPercentCrop(step.crop_region));
  const [dragCrop, setDragCrop] = useState<BoundsPercent | null>(null);
  const [isCropDragging, setIsCropDragging] = useState(false);
  const [imageRetry, setImageRetry] = useState(0);
  const [imageNaturalSize, setImageNaturalSize] = useState<{ width: number; height: number }>({
    width: 0,
    height: 0,
  });
  const dragRafRef = useRef<number | null>(null);
  const dragPendingCropRef = useRef<BoundsPercent | null>(null);
  const cropDragRef = useRef<{
    pointerId: number;
    startClientX: number;
    startClientY: number;
    frameWidth: number;
    frameHeight: number;
    startCrop: BoundsPercent;
    moved: boolean;
    latestCrop: BoundsPercent;
  } | null>(null);

  useEffect(() => {
    setImageRetry(0);
  }, [step.screenshot_path]);

  useEffect(() => {
    return () => {
      if (dragRafRef.current != null) {
        cancelAnimationFrame(dragRafRef.current);
        dragRafRef.current = null;
      }
    };
  }, []);

  const screenshotBaseSrc = useMemo(
    () => (step.screenshot_path ? convertFileSrc(step.screenshot_path) : null),
    [step.screenshot_path],
  );
  const screenshotSrc = useMemo(() => {
    if (!screenshotBaseSrc) return null;
    if (imageRetry === 0) return screenshotBaseSrc;
    const sep = screenshotBaseSrc.includes("?") ? "&" : "?";
    return `${screenshotBaseSrc}${sep}retry=${imageRetry}`;
  }, [imageRetry, screenshotBaseSrc]);

  const isAuthPlaceholder =
    step.window_title === "Authentication dialog (secure)" ||
    step.app.toLowerCase() === "authentication";

  const actionDesc =
    step.action === "DoubleClick"
      ? "Double-clicked in"
      : step.action === "RightClick"
        ? "Right-clicked in"
        : step.action === "Shortcut"
          ? "Used keyboard shortcut in"
          : "Clicked in";

  const authDescription =
    step.description && step.description.trim().length > 0
      ? step.description.trim()
      : "Authenticate with Touch ID or enter your password to continue.";

  const effectiveDescription = isAuthPlaceholder
    ? authDescription
    : (step.description && step.description.trim().length > 0
        ? step.description.trim()
        : `${actionDesc} ${step.app}`);

  const markerClass =
    step.action === "DoubleClick"
      ? "click-indicator double-click"
      : step.action === "RightClick"
        ? "click-indicator right-click"
        : "click-indicator";

  const isGenerating = step.description_status === "generating";
  const isFailed = step.description_status === "failed";
  const isAi = step.description_source === "ai";

  const stepCrop = normalizeCropRegion(step.crop_region);
  const normalizedCrop = dragCrop ?? stepCrop;

  const frameAspect = useMemo(
    () => getCropAspectRatio(imageNaturalSize.width, imageNaturalSize.height, normalizedCrop),
    [imageNaturalSize.height, imageNaturalSize.width, normalizedCrop],
  );

  const shouldApplyCrop = Boolean(normalizedCrop && !isFullCrop(normalizedCrop));
  const cropStyles = getCroppedImageStyles(shouldApplyCrop ? normalizedCrop : null);
  const marker = markerPositionForStep(
    shouldApplyCrop ? step : { ...step, crop_region: null },
  );
  const frameStyle = shouldApplyCrop
    ? { width: "100%", aspectRatio: `${frameAspect ?? 16 / 9}` }
    : undefined;
  const frameClasses = [cropStyles.frameClassName];
  if (shouldApplyCrop) frameClasses.push("is-crop-draggable");
  if (isCropDragging) frameClasses.push("is-crop-dragging");

  const clampCropPosition = (crop: BoundsPercent): BoundsPercent => {
    const width = Math.max(2, Math.min(100, crop.width_percent));
    const height = Math.max(2, Math.min(100, crop.height_percent));
    const maxX = Math.max(0, 100 - width);
    const maxY = Math.max(0, 100 - height);
    return {
      x_percent: Math.max(0, Math.min(maxX, crop.x_percent)),
      y_percent: Math.max(0, Math.min(maxY, crop.y_percent)),
      width_percent: width,
      height_percent: height,
    };
  };
  const cropDistance = (a: BoundsPercent, b: BoundsPercent) =>
    Math.max(
      Math.abs(a.x_percent - b.x_percent),
      Math.abs(a.y_percent - b.y_percent),
      Math.abs(a.width_percent - b.width_percent),
      Math.abs(a.height_percent - b.height_percent),
    );

  const handleCropDragStart = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!shouldApplyCrop || !normalizedCrop) return;
    if (e.button !== 0) return;
    if (e.cancelable) e.preventDefault();

    const rect = e.currentTarget.getBoundingClientRect();
    if (rect.width <= 1 || rect.height <= 1) return;

    cropDragRef.current = {
      pointerId: e.pointerId,
      startClientX: e.clientX,
      startClientY: e.clientY,
      frameWidth: rect.width,
      frameHeight: rect.height,
      startCrop: normalizedCrop,
      moved: false,
      latestCrop: normalizedCrop,
    };
    dragPendingCropRef.current = normalizedCrop;
    setIsCropDragging(true);
    setDragCrop(normalizedCrop);
    if (typeof e.currentTarget.setPointerCapture === "function") {
      e.currentTarget.setPointerCapture(e.pointerId);
    }
  };

  const handleCropDragMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const drag = cropDragRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;

    const dxPx = e.clientX - drag.startClientX;
    const dyPx = e.clientY - drag.startClientY;

    // Keep cursor/content movement direct: dragging right moves visible content right.
    const dxCrop = -(dxPx / drag.frameWidth) * drag.startCrop.width_percent;
    const dyCrop = -(dyPx / drag.frameHeight) * drag.startCrop.height_percent;

    const nextCrop = clampCropPosition({
      ...drag.startCrop,
      x_percent: drag.startCrop.x_percent + dxCrop,
      y_percent: drag.startCrop.y_percent + dyCrop,
    });
    drag.moved = cropDistance(nextCrop, drag.startCrop) > 0.02;
    drag.latestCrop = nextCrop;
    dragPendingCropRef.current = nextCrop;
    if (dragRafRef.current == null) {
      dragRafRef.current = requestAnimationFrame(() => {
        dragRafRef.current = null;
        if (dragPendingCropRef.current) {
          setDragCrop(dragPendingCropRef.current);
        }
      });
    }
  };

  const finishCropDrag = () => {
    const drag = cropDragRef.current;
    const finalCrop = drag?.latestCrop ?? null;
    if (dragRafRef.current != null) {
      cancelAnimationFrame(dragRafRef.current);
      dragRafRef.current = null;
    }
    dragPendingCropRef.current = null;
    cropDragRef.current = null;
    setIsCropDragging(false);
    if (finalCrop && drag?.moved) {
      onUpdateCrop(step.id, finalCrop);
    }
    setDragCrop(null);
  };

  const handleCropDragEnd = (e: React.PointerEvent<HTMLDivElement>) => {
    const drag = cropDragRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;
    finishCropDrag();
  };

  const handleStartNoteEdit = () => {
    setNoteDraft(step.note ?? "");
    setNoteEditing(true);
    requestAnimationFrame(() => noteTextareaRef.current?.focus());
  };

  const handleSaveNote = () => {
    const trimmed = noteDraft.trim();
    onUpdateNote(step.id, trimmed || null);
    setNoteEditing(false);
  };

  const handleNoteKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSaveNote();
    }
    if (e.key === "Escape") {
      setNoteEditing(false);
    }
  };

  const handleStartDescEdit = () => {
    if (isAuthPlaceholder) return;
    if (isGenerating) return;
    setDescDraft(effectiveDescription);
    setDescEditing(true);
    requestAnimationFrame(() => descInputRef.current?.focus());
  };

  const handleSaveDesc = () => {
    const trimmed = descDraft.trim();
    onUpdateDescription(step.id, trimmed || null);
    setDescEditing(false);
  };

  const handleDescKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSaveDesc();
    }
    if (e.key === "Escape") {
      setDescEditing(false);
    }
  };

  const handleOpenCropEditor = () => {
    if (!screenshotSrc || isAuthPlaceholder) return;
    setCropDraft(toPercentCrop(step.crop_region));
    setCropOpen(true);
  };

  const handleSaveCrop = () => {
    onUpdateCrop(step.id, toBoundsPercent(cropDraft));
    setCropOpen(false);
  };

  const handleResetCrop = () => {
    onUpdateCrop(step.id, null);
    setCropDraft({ ...FULL_PERCENT_CROP });
    setCropOpen(false);
  };
  const handleImageError = () => {
    if (imageRetry >= 2) return;
    window.setTimeout(() => {
      setImageRetry((prev) => (prev < 2 ? prev + 1 : prev));
    }, 120);
  };

  return (
    <div className="editor-timeline-item">
      <div className="editor-timeline-badge">{index + 1}</div>

      <article className="editor-step">
        <div className="editor-step-header">
          {descEditing ? (
            <input
              ref={descInputRef}
              className="editor-step-desc-input"
              value={descDraft}
              onChange={(e) => setDescDraft(e.target.value)}
              onBlur={handleSaveDesc}
              onKeyDown={handleDescKeyDown}
              placeholder="Step description..."
            />
          ) : (
            <button
              className="editor-step-desc editor-step-desc-btn"
              onClick={handleStartDescEdit}
              title={isAuthPlaceholder ? undefined : "Edit description"}
              disabled={isAuthPlaceholder || isGenerating}
            >
              {effectiveDescription}
            </button>
          )}

          <div className="editor-step-header-actions">
            {!isAuthPlaceholder && screenshotSrc && (
              <button
                className="editor-step-crop"
                onClick={handleOpenCropEditor}
                title="Adjust visible screenshot area"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M6 2v14a2 2 0 0 0 2 2h14" />
                  <path d="M18 22V8a2 2 0 0 0-2-2H2" />
                </svg>
              </button>
            )}
            {isGenerating && (
              <span className="editor-step-pill generating" title="Generating with Apple Intelligence...">
                AI…
              </span>
            )}
            {!isGenerating && isAi && (
              <span className="editor-step-pill ai" title="Generated by Apple Intelligence">
                AI
              </span>
            )}
            {isFailed && (
              <span
                className="editor-step-pill error"
                title={step.description_error ?? "Apple Intelligence generation failed"}
              >
                AI!
              </span>
            )}
            <button
              className="editor-step-ai"
              onClick={() => onGenerateDescription(step.id)}
              disabled={!aiEnabled || isGenerating || isAuthPlaceholder}
              title={
                !aiEnabled
                  ? "Enable Apple Intelligence descriptions in StepCast Settings"
                  : isGenerating
                    ? "Generating..."
                    : "Generate with Apple Intelligence"
              }
            >
              <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                <path
                  fill="currentColor"
                  d="M12 2.2l1.55 5.05 5.05 1.55-5.05 1.55L12 15.4l-1.55-5.05L5.4 8.8l5.05-1.55L12 2.2z"
                />
              </svg>
            </button>
            <button
              className="editor-step-delete"
              onClick={() => onDelete(step.id)}
              title="Remove step"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>

        {screenshotSrc && (
          <div className="editor-step-image">
            <div className="editor-image-wrapper">
              <div
                className={frameClasses.join(" ")}
                style={frameStyle}
                onPointerDown={handleCropDragStart}
                onPointerMove={handleCropDragMove}
                onPointerUp={handleCropDragEnd}
                onPointerCancel={handleCropDragEnd}
                onLostPointerCapture={handleCropDragEnd}
                onDragStart={(e) => e.preventDefault()}
              >
                <img
                  src={screenshotSrc}
                  alt={`Step ${index + 1}`}
                  loading="lazy"
                  decoding="async"
                  style={cropStyles.imageStyle}
                  onError={handleImageError}
                  draggable={false}
                  onDragStart={(e) => e.preventDefault()}
                  onLoad={(e) => {
                    const img = e.currentTarget;
                    if (img.naturalWidth > 0 && img.naturalHeight > 0) {
                      setImageNaturalSize({ width: img.naturalWidth, height: img.naturalHeight });
                    }
                  }}
                />
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
            </div>
          </div>
        )}

        <div className="editor-step-note-row">
          {noteEditing ? (
            <textarea
              ref={noteTextareaRef}
              className="editor-step-note-input"
              value={noteDraft}
              onChange={(e) => setNoteDraft(e.target.value)}
              onBlur={handleSaveNote}
              onKeyDown={handleNoteKeyDown}
              placeholder="Add a note..."
              rows={2}
            />
          ) : (
            <button
              className={`editor-step-note-btn${step.note ? " has-note" : ""}`}
              onClick={handleStartNoteEdit}
            >
              {step.note || "Add a note..."}
            </button>
          )}
        </div>
      </article>

      {cropOpen && screenshotSrc && (
        <div className="editor-crop-overlay" onClick={() => setCropOpen(false)}>
          <div className="editor-crop-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
            <div className="editor-crop-header">
              <div className="editor-crop-title">Adjust Focus Crop</div>
              <button className="editor-crop-close" onClick={() => setCropOpen(false)} title="Close crop editor">
                ×
              </button>
            </div>
            <div className="editor-crop-body">
              <ReactCrop
                crop={cropDraft}
                minWidth={8}
                minHeight={8}
                keepSelection
                onChange={(_, percentCrop) => {
                  setCropDraft({
                    unit: "%",
                    x: percentCrop.x ?? 0,
                    y: percentCrop.y ?? 0,
                    width: percentCrop.width ?? 100,
                    height: percentCrop.height ?? 100,
                  });
                }}
              >
                <img
                  src={screenshotSrc}
                  alt={`Adjust crop for step ${index + 1}`}
                  onError={handleImageError}
                  draggable={false}
                  onDragStart={(e) => e.preventDefault()}
                  onLoad={(e) => {
                    const img = e.currentTarget;
                    if (img.naturalWidth > 0 && img.naturalHeight > 0) {
                      setImageNaturalSize({ width: img.naturalWidth, height: img.naturalHeight });
                    }
                  }}
                />
              </ReactCrop>
            </div>
            <div className="editor-crop-actions">
              <button className="button ghost" onClick={handleResetCrop}>Reset</button>
              <button className="button ghost" onClick={() => setCropOpen(false)}>Cancel</button>
              <button className="button primary" onClick={handleSaveCrop}>Apply</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
});
