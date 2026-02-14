import type { CSSProperties } from "react";
import type { BoundsPercent, Step } from "../types/step";

const MIN_CROP_SIZE_PERCENT = 2;

export function normalizeCropRegion(cropRegion?: BoundsPercent | null): BoundsPercent | null {
  if (!cropRegion) return null;

  const values = [
    cropRegion.x_percent,
    cropRegion.y_percent,
    cropRegion.width_percent,
    cropRegion.height_percent,
  ];
  if (values.some((v) => !Number.isFinite(v))) return null;

  const x = clamp(cropRegion.x_percent, 0, 100);
  const y = clamp(cropRegion.y_percent, 0, 100);
  let width = clamp(cropRegion.width_percent, 0, 100);
  let height = clamp(cropRegion.height_percent, 0, 100);

  if (x + width > 100) {
    width = Math.max(0, 100 - x);
  }
  if (y + height > 100) {
    height = Math.max(0, 100 - y);
  }

  if (width < MIN_CROP_SIZE_PERCENT || height < MIN_CROP_SIZE_PERCENT) {
    return null;
  }

  return {
    x_percent: round3(x),
    y_percent: round3(y),
    width_percent: round3(width),
    height_percent: round3(height),
  };
}

export function isFullCrop(cropRegion?: BoundsPercent | null): boolean {
  const crop = normalizeCropRegion(cropRegion);
  if (!crop) return true;
  return (
    nearlyEqual(crop.x_percent, 0) &&
    nearlyEqual(crop.y_percent, 0) &&
    nearlyEqual(crop.width_percent, 100) &&
    nearlyEqual(crop.height_percent, 100)
  );
}

export function markerPositionForStep(step: Step): {
  xPercent: number;
  yPercent: number;
  visible: boolean;
} {
  const crop = normalizeCropRegion(step.crop_region);
  if (!crop) {
    return {
      xPercent: clamp(step.click_x_percent, 0, 100),
      yPercent: clamp(step.click_y_percent, 0, 100),
      visible: true,
    };
  }

  const x = ((step.click_x_percent - crop.x_percent) / crop.width_percent) * 100;
  const y = ((step.click_y_percent - crop.y_percent) / crop.height_percent) * 100;
  const visible = x >= 0 && x <= 100 && y >= 0 && y <= 100;

  return {
    xPercent: clamp(x, 0, 100),
    yPercent: clamp(y, 0, 100),
    visible,
  };
}

export function getCroppedImageStyles(cropRegion?: BoundsPercent | null): {
  frameClassName: string;
  imageStyle: CSSProperties;
} {
  const crop = normalizeCropRegion(cropRegion);
  if (!crop || isFullCrop(crop)) {
    return {
      frameClassName: "step-image-frame",
      imageStyle: {},
    };
  }

  const widthScale = 10000 / crop.width_percent;
  const heightScale = 10000 / crop.height_percent;

  return {
    frameClassName: "step-image-frame step-image-frame-cropped",
    imageStyle: {
      position: "absolute",
      left: 0,
      top: 0,
      width: `${widthScale}%`,
      height: `${heightScale}%`,
      maxWidth: "none",
      transform: `translate(-${crop.x_percent}%, -${crop.y_percent}%)`,
      transformOrigin: "top left",
    },
  };
}

export function getCropAspectRatio(
  imageWidth: number,
  imageHeight: number,
  cropRegion?: BoundsPercent | null,
): number | null {
  if (imageWidth <= 0 || imageHeight <= 0) return null;

  const crop = normalizeCropRegion(cropRegion);
  if (!crop || isFullCrop(crop)) {
    return imageWidth / imageHeight;
  }

  const cropWidth = (crop.width_percent / 100) * imageWidth;
  const cropHeight = (crop.height_percent / 100) * imageHeight;
  if (cropWidth <= 0 || cropHeight <= 0) return null;
  return cropWidth / cropHeight;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function nearlyEqual(a: number, b: number): boolean {
  return Math.abs(a - b) < 0.01;
}

function round3(value: number): number {
  return Math.round(value * 1000) / 1000;
}
