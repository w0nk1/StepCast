import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { DndContext, closestCenter, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy, arrayMove } from "@dnd-kit/sortable";
import EditorStepCard from "./EditorStepCard";
import UndoToast from "./UndoToast";
import type { BoundsPercent, Step } from "../types/step";
import { mergeUpdatedStep } from "../utils/stepEvents";

type PendingDelete = { step: Step; index: number; timerId: ReturnType<typeof setTimeout> };

function isAuthPlaceholder(step: Step) {
  return (
    step.window_title === "Authentication dialog (secure)" ||
    step.app.toLowerCase() === "authentication"
  );
}

export default function EditorWindow() {
  const [steps, setSteps] = useState<Step[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [collapsedIds, setCollapsedIds] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<PendingDelete | null>(null);
  const pendingDeleteRef = useRef<PendingDelete | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [focusedIndex, setFocusedIndex] = useState<number | null>(null);
  const [selection, setSelection] = useState<Set<string>>(new Set());
  const lastSelectedRef = useRef<string | null>(null);
  const [aiEnabled, setAiEnabled] = useState(
    () => localStorage.getItem("appleIntelligenceDescriptions") === "true",
  );

  // Load initial steps from backend
  useEffect(() => {
    invoke<Step[]>("get_steps")
      .then((result) => {
        setSteps(result);
        setLoaded(true);
      })
      .catch(() => {
        setLoaded(true);
      });
  }, []);

  // Sync global AI toggle (stored in localStorage) across windows.
  useEffect(() => {
    const handler = (e: StorageEvent) => {
      if (e.key !== "appleIntelligenceDescriptions") return;
      setAiEnabled(e.newValue === "true");
    };
    window.addEventListener("storage", handler);
    return () => window.removeEventListener("storage", handler);
  }, []);

  // Tauri event fallback: storage events don't consistently propagate between Tauri webviews.
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    listen<{ enabled?: boolean }>("ai-toggle-changed", (event) => {
      setAiEnabled(Boolean(event.payload?.enabled));
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Listen for step-captured, step-updated, steps-discarded events
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    listen<Step>("step-captured", (event) => {
      setSteps((prev) => {
        if (prev.some((s) => s.id === event.payload.id)) return prev;
        setTimeout(() => {
          const el = scrollRef.current;
          if (el) el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
        }, 50);
        return [...prev, event.payload];
      });
    }).then((fn) => unlisteners.push(fn));

    listen<Step>("step-updated", (event) => {
      setSteps((prev) =>
        prev.map((s) => (s.id === event.payload.id ? mergeUpdatedStep(s, event.payload) : s)),
      );
    }).then((fn) => unlisteners.push(fn));

    listen("steps-discarded", () => {
      setSteps([]);
    }).then((fn) => unlisteners.push(fn));

    listen<string>("step-deleted", (event) => {
      setSteps((prev) => prev.filter((s) => s.id !== event.payload));
    }).then((fn) => unlisteners.push(fn));

    listen<Step[]>("steps-reordered", (event) => {
      setSteps(event.payload);
    }).then((fn) => unlisteners.push(fn));

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  const handleUpdateNote = useCallback((stepId: string, note: string | null) => {
    invoke("update_step_note", { stepId, note }).catch(() => {});
    setSteps((prev) =>
      prev.map((s) => (s.id === stepId ? { ...s, note } : s)),
    );
  }, []);

  const handleUpdateDescription = useCallback((stepId: string, description: string | null) => {
    invoke("update_step_description", { stepId, description }).catch(() => {});
    setSteps((prev) =>
      prev.map((s) =>
        s.id === stepId
          ? {
              ...s,
              description,
              description_source: description ? "manual" : null,
              description_status: null,
              description_error: null,
            }
          : s,
      ),
    );
  }, []);

  const flushPendingDelete = useCallback((pending: PendingDelete) => {
    clearTimeout(pending.timerId);
    invoke("delete_step", { stepId: pending.step.id }).catch(() => {});
    setPendingDelete((cur) => (cur === pending ? null : cur));
    pendingDeleteRef.current = null;
  }, []);

  const handleDelete = useCallback((stepId: string) => {
    setSteps((prev) => {
      const idx = prev.findIndex((s) => s.id === stepId);
      if (idx === -1) return prev;
      const deletedStep = prev[idx];

      // Flush any existing pending delete first
      const existing = pendingDeleteRef.current;
      if (existing) {
        clearTimeout(existing.timerId);
        invoke("delete_step", { stepId: existing.step.id }).catch(() => {});
      }

      const timerId = setTimeout(() => {
        const cur = pendingDeleteRef.current;
        if (cur && cur.step.id === deletedStep.id) {
          invoke("delete_step", { stepId: deletedStep.id }).catch(() => {});
          setPendingDelete(null);
          pendingDeleteRef.current = null;
        }
      }, 3000);

      const newPending: PendingDelete = { step: deletedStep, index: idx, timerId };
      pendingDeleteRef.current = newPending;
      setPendingDelete(newPending);

      return prev.filter((s) => s.id !== stepId);
    });
  }, []);

  const handleUndoDelete = useCallback(() => {
    const pending = pendingDeleteRef.current;
    if (!pending) return;
    clearTimeout(pending.timerId);
    pendingDeleteRef.current = null;
    setPendingDelete(null);
    setSteps((prev) => {
      const insertIdx = Math.min(pending.index, prev.length);
      const next = [...prev];
      next.splice(insertIdx, 0, pending.step);
      return next;
    });
  }, []);

  const handleDismissUndo = useCallback(() => {
    const pending = pendingDeleteRef.current;
    if (!pending) return;
    flushPendingDelete(pending);
  }, [flushPendingDelete]);

  // Flush pending delete on unmount
  useEffect(() => {
    return () => {
      const pending = pendingDeleteRef.current;
      if (pending) {
        clearTimeout(pending.timerId);
        invoke("delete_step", { stepId: pending.step.id }).catch(() => {});
      }
    };
  }, []);

  const handleUpdateCrop = useCallback((stepId: string, cropRegion: BoundsPercent | null) => {
    invoke("update_step_crop", { stepId, cropRegion }).catch(() => {});
    setSteps((prev) =>
      prev.map((s) => (s.id === stepId ? { ...s, crop_region: cropRegion } : s)),
    );
  }, []);

  const handleGenerateDescription = useCallback(
    (stepId: string) => {
      if (!aiEnabled) return;
      invoke("generate_step_descriptions", { stepIds: [stepId] }).catch(() => {});
    },
    [aiEnabled],
  );

  const handleToggleCollapse = useCallback((stepId: string) => {
    setCollapsedIds((prev) => {
      const next = new Set(prev);
      if (next.has(stepId)) next.delete(stepId);
      else next.add(stepId);
      return next;
    });
  }, []);

  const handleCollapseAll = useCallback(() => {
    setCollapsedIds(new Set(steps.map((s) => s.id)));
  }, [steps]);

  const handleExpandAll = useCallback(() => {
    setCollapsedIds(new Set());
  }, []);

  const handleToggleSelect = useCallback((stepId: string, shiftKey: boolean) => {
    setSelection((prev) => {
      const next = new Set(prev);
      if (shiftKey && lastSelectedRef.current) {
        const lastIdx = steps.findIndex((s) => s.id === lastSelectedRef.current);
        const curIdx = steps.findIndex((s) => s.id === stepId);
        if (lastIdx !== -1 && curIdx !== -1) {
          const [from, to] = lastIdx < curIdx ? [lastIdx, curIdx] : [curIdx, lastIdx];
          for (let i = from; i <= to; i++) next.add(steps[i].id);
        }
      } else if (next.has(stepId)) {
        next.delete(stepId);
      } else {
        next.add(stepId);
      }
      lastSelectedRef.current = stepId;
      return next;
    });
  }, [steps]);

  const handleDeselectAll = useCallback(() => {
    setSelection(new Set());
    lastSelectedRef.current = null;
  }, []);

  const handleBulkDelete = useCallback(() => {
    for (const id of selection) {
      invoke("delete_step", { stepId: id }).catch(() => {});
    }
    setSteps((prev) => prev.filter((s) => !selection.has(s.id)));
    setSelection(new Set());
    lastSelectedRef.current = null;
  }, [selection]);

  const handleBulkGenerate = useCallback(() => {
    if (!aiEnabled) return;
    const ids = steps.filter((s) => selection.has(s.id)).map((s) => s.id);
    if (ids.length > 0) {
      invoke("generate_step_descriptions", { stepIds: ids }).catch(() => {});
    }
  }, [aiEnabled, steps, selection]);

  // Prune stale selection IDs when steps change
  useEffect(() => {
    setSelection((prev) => {
      const stepIds = new Set(steps.map((s) => s.id));
      const pruned = new Set([...prev].filter((id) => stepIds.has(id)));
      return pruned.size === prev.size ? prev : pruned;
    });
  }, [steps]);

  const handleScrollKeyDown = useCallback((e: React.KeyboardEvent) => {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setFocusedIndex((prev) => {
        const next = prev === null ? 0 : Math.min(prev + 1, steps.length - 1);
        return next;
      });
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setFocusedIndex((prev) => {
        const next = prev === null ? steps.length - 1 : Math.max(prev - 1, 0);
        return next;
      });
    } else if (e.key === "Delete" || e.key === "Backspace") {
      if (focusedIndex !== null && focusedIndex < steps.length) {
        e.preventDefault();
        const stepId = steps[focusedIndex].id;
        handleDelete(stepId);
        if (focusedIndex >= steps.length - 1) {
          setFocusedIndex(steps.length > 1 ? steps.length - 2 : null);
        }
      }
    } else if (e.key === "Escape") {
      setFocusedIndex(null);
    }
  }, [steps, focusedIndex, handleDelete]);

  const handleDragEnd = useCallback((event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    setSteps((prev) => {
      const oldIndex = prev.findIndex((s) => s.id === active.id);
      const newIndex = prev.findIndex((s) => s.id === over.id);
      if (oldIndex === -1 || newIndex === -1) return prev;
      const reordered = arrayMove(prev, oldIndex, newIndex);
      invoke("reorder_steps", { stepIds: reordered.map((s) => s.id) }).catch(() => {});
      return reordered;
    });
  }, []);

  const handleEnhanceAll = useCallback(() => {
    if (!aiEnabled) return;

    const missing = steps.filter(
      (s) =>
        !isAuthPlaceholder(s) &&
        s.action !== "Note" &&
        (!s.description || s.description.trim().length === 0) &&
        s.description_source !== "manual",
    );
    const mode = missing.length > 0 ? "missing_only" : "all";
    invoke("generate_step_descriptions", { mode }).catch(() => {});
  }, [aiEnabled, steps]);

  const isAnyGenerating = useMemo(
    () => steps.some((s) => s.description_status === "generating"),
    [steps],
  );

  if (!loaded) return null;

  return (
    <div className="editor-body">
      <header className="editor-toolbar">
        <div className="editor-toolbar-left">
          <div className="editor-toolbar-title">Edit Steps</div>
          <div className="editor-toolbar-subtitle">
            {steps.length} step{steps.length === 1 ? "" : "s"}
          </div>
        </div>
        <div className="editor-toolbar-right">
          {selection.size > 0 ? (
            <>
              <span className="editor-toolbar-selection-count">
                {selection.size} selected
              </span>
              <button
                className="button ghost"
                onClick={handleBulkGenerate}
                disabled={!aiEnabled || isAnyGenerating}
                title="Enhance selected steps with AI"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" focusable="false">
                  <path d="M10 2l1.5 4.5L16 8l-4.5 1.5L10 14l-1.5-4.5L4 8l4.5-1.5L10 2z" />
                  <path d="M18 12l1 3 3 1-3 1-1 3-1-3-3-1 3-1 1-3z" />
                </svg>
                Enhance
              </button>
              <button className="button ghost" onClick={handleBulkDelete} title="Delete selected steps">
                Delete
              </button>
              <button className="button ghost" onClick={handleDeselectAll} title="Deselect all">
                Deselect
              </button>
            </>
          ) : (
            <>
              {isAnyGenerating && (
                <span className="editor-toolbar-generating">
                  <span className="editor-toolbar-spinner" />
                  Generating…
                </span>
              )}
              {steps.length > 5 && (
                <button
                  className="button ghost"
                  onClick={collapsedIds.size > 0 ? handleExpandAll : handleCollapseAll}
                  title={collapsedIds.size > 0 ? "Expand all steps" : "Collapse all steps"}
                >
                  {collapsedIds.size > 0 ? "Expand All" : "Collapse All"}
                </button>
              )}
              <button
                className="button ghost"
                onClick={handleEnhanceAll}
                disabled={!aiEnabled || steps.length === 0 || isAnyGenerating}
                title={
                  !aiEnabled
                    ? "Enable Apple Intelligence descriptions in StepCast Settings"
                    : isAnyGenerating
                      ? "AI generation in progress…"
                      : "Generate concise descriptions for steps"
                }
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" focusable="false">
                  <path d="M10 2l1.5 4.5L16 8l-4.5 1.5L10 14l-1.5-4.5L4 8l4.5-1.5L10 2z" />
                  <path d="M18 12l1 3 3 1-3 1-1 3-1-3-3-1 3-1 1-3z" />
                </svg>
                Enhance Steps
              </button>
            </>
          )}
        </div>
      </header>
      <div className="editor-scroll" ref={scrollRef} tabIndex={0} onKeyDown={handleScrollKeyDown}>
        {steps.length === 0 ? (
          <div className="editor-empty">No steps recorded yet</div>
        ) : (
          <DndContext collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
            <SortableContext items={steps.map((s) => s.id)} strategy={verticalListSortingStrategy}>
              <div className="editor-steps">
                {steps.map((step, index) => (
                  <EditorStepCard
                    key={step.id}
                    step={step}
                    index={index}
                    onUpdateNote={handleUpdateNote}
                    onUpdateDescription={handleUpdateDescription}
                    onGenerateDescription={handleGenerateDescription}
                    aiEnabled={aiEnabled}
                    onDelete={handleDelete}
                    onUpdateCrop={handleUpdateCrop}
                    collapsed={collapsedIds.has(step.id)}
                    onToggleCollapse={handleToggleCollapse}
                    isFocused={focusedIndex === index}
                    isSelected={selection.has(step.id)}
                    isSelectionActive={selection.size > 0}
                    onToggleSelect={handleToggleSelect}
                  />
                ))}
              </div>
            </SortableContext>
          </DndContext>
        )}
      </div>
      {pendingDelete && (
        <UndoToast
          message="Step deleted"
          onUndo={handleUndoDelete}
          onDismiss={handleDismissUndo}
        />
      )}
    </div>
  );
}
