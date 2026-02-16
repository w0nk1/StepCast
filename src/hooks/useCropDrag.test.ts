import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useCropDrag } from "./useCropDrag";
import type { BoundsPercent } from "../types/step";

function makeCrop(overrides: Partial<BoundsPercent> = {}): BoundsPercent {
  return {
    x_percent: 20,
    y_percent: 20,
    width_percent: 50,
    height_percent: 50,
    ...overrides,
  };
}

/** Build a minimal synthetic PointerEvent matching React.PointerEvent<HTMLDivElement>. */
function makePtrEvent(
  overrides: Partial<{
    button: number;
    pointerId: number;
    clientX: number;
    clientY: number;
    cancelable: boolean;
    rectWidth: number;
    rectHeight: number;
  }> = {},
) {
  const rectW = overrides.rectWidth ?? 500;
  const rectH = overrides.rectHeight ?? 250;
  return {
    button: overrides.button ?? 0,
    pointerId: overrides.pointerId ?? 1,
    clientX: overrides.clientX ?? 100,
    clientY: overrides.clientY ?? 100,
    cancelable: overrides.cancelable ?? true,
    preventDefault: vi.fn(),
    currentTarget: {
      getBoundingClientRect: () => ({
        x: 0,
        y: 0,
        width: rectW,
        height: rectH,
        top: 0,
        left: 0,
        right: rectW,
        bottom: rectH,
        toJSON: () => ({}),
      }),
      setPointerCapture: vi.fn(),
    },
  } as unknown as React.PointerEvent<HTMLDivElement>;
}

describe("useCropDrag", () => {
  let rafId = 0;
  let rafCallbacks: Map<number, FrameRequestCallback>;

  beforeEach(() => {
    rafId = 0;
    rafCallbacks = new Map();
    vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation((cb) => {
      rafId += 1;
      rafCallbacks.set(rafId, cb);
      return rafId;
    });
    vi.spyOn(globalThis, "cancelAnimationFrame").mockImplementation((id) => {
      rafCallbacks.delete(id);
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function flushRaf() {
    for (const [id, cb] of rafCallbacks) {
      rafCallbacks.delete(id);
      cb(performance.now());
    }
  }

  it("returns initial state (dragCrop null, isCropDragging false)", () => {
    const { result } = renderHook(() => useCropDrag(vi.fn()));
    expect(result.current.dragCrop).toBeNull();
    expect(result.current.isCropDragging).toBe(false);
  });

  it("handlePointerDown with non-zero button does nothing", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const e = makePtrEvent({ button: 2 });

    act(() => {
      result.current.handlePointerDown(e, makeCrop());
    });

    expect(result.current.isCropDragging).toBe(false);
    expect(result.current.dragCrop).toBeNull();
  });

  it("handlePointerDown sets isCropDragging true", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const e = makePtrEvent({ button: 0 });

    act(() => {
      result.current.handlePointerDown(e, makeCrop());
    });

    expect(result.current.isCropDragging).toBe(true);
    expect(result.current.dragCrop).toEqual(makeCrop());
  });

  it("handlePointerDown with tiny rect (width <= 1) does nothing", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const e = makePtrEvent({ button: 0, rectWidth: 0.5, rectHeight: 0.5 });

    act(() => {
      result.current.handlePointerDown(e, makeCrop());
    });

    expect(result.current.isCropDragging).toBe(false);
    expect(result.current.dragCrop).toBeNull();
  });

  it("handlePointerMove without active drag does nothing", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const moveEvent = makePtrEvent({ clientX: 200, clientY: 200 });

    act(() => {
      result.current.handlePointerMove(moveEvent);
    });

    expect(result.current.dragCrop).toBeNull();
    expect(result.current.isCropDragging).toBe(false);
  });

  it("handlePointerUp commits if moved", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const crop = makeCrop();

    // Start drag
    act(() => {
      result.current.handlePointerDown(
        makePtrEvent({ button: 0, clientX: 100, clientY: 100 }),
        crop,
      );
    });

    // Move far enough to set moved = true (need > 0.02 distance)
    act(() => {
      result.current.handlePointerMove(
        makePtrEvent({ clientX: 200, clientY: 200 }),
      );
    });

    // Flush RAF so dragCrop state is updated
    act(() => {
      flushRaf();
    });

    // Release
    act(() => {
      result.current.handlePointerUp(
        makePtrEvent({ clientX: 200, clientY: 200 }),
      );
    });

    expect(onCommit).toHaveBeenCalledTimes(1);
    const committed = onCommit.mock.calls[0][0] as BoundsPercent;
    expect(committed.width_percent).toBe(50);
    expect(committed.height_percent).toBe(50);
  });

  it("handlePointerUp does nothing without active drag", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const e = makePtrEvent();

    act(() => {
      result.current.handlePointerUp(e);
    });

    expect(onCommit).not.toHaveBeenCalled();
    expect(result.current.isCropDragging).toBe(false);
  });

  it("cleanup on unmount cancels animation frame", () => {
    const onCommit = vi.fn();
    const { result, unmount } = renderHook(() => useCropDrag(onCommit));
    const crop = makeCrop();

    // Start drag + move to schedule a RAF
    act(() => {
      result.current.handlePointerDown(
        makePtrEvent({ button: 0, clientX: 100, clientY: 100 }),
        crop,
      );
    });

    act(() => {
      result.current.handlePointerMove(
        makePtrEvent({ clientX: 200, clientY: 200 }),
      );
    });

    // RAF is scheduled but not flushed
    expect(cancelAnimationFrame).not.toHaveBeenCalled();

    // Unmount should cancel the pending RAF
    unmount();

    expect(cancelAnimationFrame).toHaveBeenCalled();
  });

  it("handlePointerMove with mismatched pointerId does nothing", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const crop = makeCrop();

    act(() => {
      result.current.handlePointerDown(
        makePtrEvent({ button: 0, pointerId: 1, clientX: 100, clientY: 100 }),
        crop,
      );
    });

    // Move with different pointerId
    act(() => {
      result.current.handlePointerMove(
        makePtrEvent({ pointerId: 99, clientX: 200, clientY: 200 }),
      );
    });

    // No RAF should have been scheduled
    expect(requestAnimationFrame).not.toHaveBeenCalled();
  });

  it("handlePointerUp with mismatched pointerId does nothing", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const crop = makeCrop();

    act(() => {
      result.current.handlePointerDown(
        makePtrEvent({ button: 0, pointerId: 1, clientX: 100, clientY: 100 }),
        crop,
      );
    });

    // Up with different pointerId
    act(() => {
      result.current.handlePointerUp(
        makePtrEvent({ pointerId: 99, clientX: 100, clientY: 100 }),
      );
    });

    // Should still be dragging
    expect(result.current.isCropDragging).toBe(true);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("does not commit when pointer released without movement", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const crop = makeCrop();

    act(() => {
      result.current.handlePointerDown(
        makePtrEvent({ button: 0, clientX: 100, clientY: 100 }),
        crop,
      );
    });

    // Release at same position (no move event)
    act(() => {
      result.current.handlePointerUp(
        makePtrEvent({ clientX: 100, clientY: 100 }),
      );
    });

    expect(onCommit).not.toHaveBeenCalled();
    expect(result.current.isCropDragging).toBe(false);
    expect(result.current.dragCrop).toBeNull();
  });

  it("calls setPointerCapture when available", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const e = makePtrEvent({ button: 0, pointerId: 5 });

    act(() => {
      result.current.handlePointerDown(e, makeCrop());
    });

    expect(e.currentTarget.setPointerCapture).toHaveBeenCalledWith(5);
  });

  it("coalesces multiple moves into one RAF update", () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useCropDrag(onCommit));
    const crop = makeCrop();

    act(() => {
      result.current.handlePointerDown(
        makePtrEvent({ button: 0, clientX: 100, clientY: 100 }),
        crop,
      );
    });

    // Two moves before RAF fires
    act(() => {
      result.current.handlePointerMove(makePtrEvent({ clientX: 150, clientY: 150 }));
      result.current.handlePointerMove(makePtrEvent({ clientX: 200, clientY: 200 }));
    });

    // Only one RAF should have been requested
    expect(requestAnimationFrame).toHaveBeenCalledTimes(1);

    // Flush it
    act(() => {
      flushRaf();
    });

    // dragCrop should reflect the latest move
    expect(result.current.dragCrop).not.toBeNull();
  });
});
