import { useEffect, useRef, useState } from "react";
import type { BoundsPercent } from "../types/step";

type CropDragState = {
  pointerId: number;
  startClientX: number;
  startClientY: number;
  frameWidth: number;
  frameHeight: number;
  startCrop: BoundsPercent;
  moved: boolean;
  latestCrop: BoundsPercent;
};

function clampCropPosition(crop: BoundsPercent): BoundsPercent {
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
}

function cropDistance(a: BoundsPercent, b: BoundsPercent) {
  return Math.max(
    Math.abs(a.x_percent - b.x_percent),
    Math.abs(a.y_percent - b.y_percent),
    Math.abs(a.width_percent - b.width_percent),
    Math.abs(a.height_percent - b.height_percent),
  );
}

export function useCropDrag(onCommit: (crop: BoundsPercent) => void) {
  const [dragCrop, setDragCrop] = useState<BoundsPercent | null>(null);
  const [isCropDragging, setIsCropDragging] = useState(false);
  const dragRafRef = useRef<number | null>(null);
  const dragPendingCropRef = useRef<BoundsPercent | null>(null);
  const cropDragRef = useRef<CropDragState | null>(null);

  useEffect(() => {
    return () => {
      if (dragRafRef.current != null) {
        cancelAnimationFrame(dragRafRef.current);
        dragRafRef.current = null;
      }
    };
  }, []);

  const handlePointerDown = (
    e: React.PointerEvent<HTMLDivElement>,
    normalizedCrop: BoundsPercent,
  ) => {
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

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const drag = cropDragRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;

    const dxPx = e.clientX - drag.startClientX;
    const dyPx = e.clientY - drag.startClientY;
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

  const finish = () => {
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
      onCommit(finalCrop);
    }
    setDragCrop(null);
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    const drag = cropDragRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;
    finish();
  };

  return {
    dragCrop,
    isCropDragging,
    handlePointerDown,
    handlePointerMove,
    handlePointerUp,
  };
}
