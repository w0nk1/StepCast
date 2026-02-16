import { useState } from "react";
import ReactCrop, { type PercentCrop } from "react-image-crop";
import type { BoundsPercent } from "../types/step";
import { isFullCrop, normalizeCropRegion } from "../utils/stepCrop";
import { useI18n } from "../i18n";

const FULL_PERCENT_CROP: PercentCrop = {
  unit: "%",
  x: 0,
  y: 0,
  width: 100,
  height: 100,
};

export function toPercentCrop(cropRegion?: BoundsPercent | null): PercentCrop {
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

export function toBoundsPercent(crop: PercentCrop): BoundsPercent | null {
  const normalized = normalizeCropRegion({
    x_percent: crop.x ?? 0,
    y_percent: crop.y ?? 0,
    width_percent: crop.width ?? 0,
    height_percent: crop.height ?? 0,
  });
  if (!normalized) return null;
  return isFullCrop(normalized) ? null : normalized;
}

type CropEditorModalProps = {
  screenshotSrc: string;
  stepIndex: number;
  initialCropRegion: BoundsPercent | null | undefined;
  clickXPercent?: number;
  clickYPercent?: number;
  action?: string;
  onSave: (crop: BoundsPercent | null) => void;
  onReset: () => void;
  onClose: () => void;
  onImageError: () => void;
  onImageLoad: (width: number, height: number) => void;
};

export default function CropEditorModal({
  screenshotSrc,
  stepIndex,
  initialCropRegion,
  clickXPercent,
  clickYPercent,
  action,
  onSave,
  onReset,
  onClose,
  onImageError,
  onImageLoad,
}: CropEditorModalProps) {
  const { t } = useI18n();
  const [cropDraft, setCropDraft] = useState<PercentCrop>(() =>
    toPercentCrop(initialCropRegion),
  );

  const handleSave = () => {
    onSave(toBoundsPercent(cropDraft));
  };

  const handleReset = () => {
    onReset();
    setCropDraft({ ...FULL_PERCENT_CROP });
  };

  return (
    <div className="editor-crop-overlay" onClick={onClose}>
      <div className="editor-crop-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="editor-crop-header">
          <div className="editor-crop-title">{t("step.crop.modal_title")}</div>
          <button className="editor-crop-close" onClick={onClose} title={t("step.crop.close_title")}>
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
            <div style={{ position: "relative", lineHeight: 0 }}>
              <img
                src={screenshotSrc}
                alt={t("step.crop.image_alt", { num: stepIndex + 1 })}
                onError={onImageError}
                draggable={false}
                onDragStart={(e) => e.preventDefault()}
                onLoad={(e) => {
                  const img = e.currentTarget;
                  if (img.naturalWidth > 0 && img.naturalHeight > 0) {
                    onImageLoad(img.naturalWidth, img.naturalHeight);
                  }
                }}
              />
              {clickXPercent != null && clickYPercent != null && (
                <div
                  className={
                    action === "DoubleClick"
                      ? "click-indicator double-click"
                      : action === "RightClick"
                        ? "click-indicator right-click"
                        : "click-indicator"
                  }
                  style={{
                    left: `${clickXPercent}%`,
                    top: `${clickYPercent}%`,
                    pointerEvents: "none",
                  }}
                />
              )}
            </div>
          </ReactCrop>
        </div>
        <div className="editor-crop-actions">
          <button className="button ghost" onClick={handleReset}>{t("common.reset")}</button>
          <button className="button ghost" onClick={onClose}>{t("common.cancel")}</button>
          <button className="button primary" onClick={handleSave}>{t("common.apply")}</button>
        </div>
      </div>
    </div>
  );
}
