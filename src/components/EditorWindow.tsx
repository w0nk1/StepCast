import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import EditorStepCard from "./EditorStepCard";
import type { BoundsPercent, Step } from "../types/step";
import { mergeUpdatedStep } from "../utils/stepEvents";

function isAuthPlaceholder(step: Step) {
  return (
    step.window_title === "Authentication dialog (secure)" ||
    step.app.toLowerCase() === "authentication"
  );
}

export default function EditorWindow() {
  const [steps, setSteps] = useState<Step[]>([]);
  const [loaded, setLoaded] = useState(false);
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

  const handleDelete = useCallback((stepId: string) => {
    invoke("delete_step", { stepId }).catch(() => {});
    setSteps((prev) => prev.filter((s) => s.id !== stepId));
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
          <button
            className="button ghost"
            onClick={handleEnhanceAll}
            disabled={!aiEnabled || steps.length === 0}
            title={
              !aiEnabled
                ? "Enable Apple Intelligence descriptions in StepCast Settings"
                : "Generate concise descriptions for steps"
            }
          >
            <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path
                fill="currentColor"
                d="M12 2.2l1.55 5.05 5.05 1.55-5.05 1.55L12 15.4l-1.55-5.05L5.4 8.8l5.05-1.55L12 2.2z"
              />
            </svg>
            Enhance Steps
          </button>
        </div>
      </header>
      <div className="editor-scroll">
        {steps.length === 0 ? (
          <div className="editor-empty">No steps recorded yet</div>
        ) : (
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
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
