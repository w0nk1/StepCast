import { memo, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import ImageLightbox from "./ImageLightbox";
import type { BoundsPercent, Step } from "../types/step";
import { useCropDrag } from "../hooks/useCropDrag";
import {
  getCropAspectRatio,
  getCroppedImageStyles,
  isFullCrop,
  markerPositionForStep,
  normalizeCropRegion,
} from "../utils/stepCrop";

type StepScreenshotProps = {
  step: Step;
  index: number;
  isAuthPlaceholder: boolean;
  onUpdateCrop: (stepId: string, cropRegion: BoundsPercent | null) => void;
  onImageNaturalSize: (size: { width: number; height: number }) => void;
  imageNaturalSize: { width: number; height: number };
};

export default memo(function StepScreenshot({
  step,
  index,
  isAuthPlaceholder,
  onUpdateCrop,
  onImageNaturalSize,
  imageNaturalSize,
}: StepScreenshotProps) {
  const [imageRetry, setImageRetry] = useState(0);
  const [lightboxOpen, setLightboxOpen] = useState(false);

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

  const { dragCrop, isCropDragging, handlePointerDown, handlePointerMove, handlePointerUp } =
    useCropDrag((crop) => onUpdateCrop(step.id, crop));

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

  const markerClass =
    step.action === "DoubleClick"
      ? "click-indicator double-click"
      : step.action === "RightClick"
        ? "click-indicator right-click"
        : "click-indicator";

  const handleImageError = () => {
    if (imageRetry >= 2) return;
    window.setTimeout(() => {
      setImageRetry((prev) => (prev < 2 ? prev + 1 : prev));
    }, 120);
  };

  if (!screenshotSrc) return null;

  return (
    <>
      <div className="editor-step-image">
        <div className="editor-image-wrapper">
          <button
            className="editor-image-expand"
            onClick={() => setLightboxOpen(true)}
            title="View full size"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <polyline points="15 3 21 3 21 9" />
              <polyline points="9 21 3 21 3 15" />
              <line x1="21" y1="3" x2="14" y2="10" />
              <line x1="3" y1="21" x2="10" y2="14" />
            </svg>
          </button>
          <div
            className={frameClasses.join(" ")}
            style={frameStyle}
            onPointerDown={shouldApplyCrop && normalizedCrop
              ? (e) => handlePointerDown(e, normalizedCrop)
              : undefined}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onPointerCancel={handlePointerUp}
            onLostPointerCapture={handlePointerUp}
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
                  onImageNaturalSize({ width: img.naturalWidth, height: img.naturalHeight });
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

      {lightboxOpen && screenshotBaseSrc && (
        <ImageLightbox
          src={screenshotBaseSrc}
          alt={`Step ${index + 1} full size`}
          onClose={() => setLightboxOpen(false)}
        />
      )}
    </>
  );
});
