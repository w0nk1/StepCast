import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import EditorStepCard from "./EditorStepCard";
import type { Step } from "../types/step";

export default function EditorWindow() {
  const [steps, setSteps] = useState<Step[]>([]);
  const [loaded, setLoaded] = useState(false);

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
        prev.map((s) => (s.id === event.payload.id ? event.payload : s)),
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

  const handleDelete = useCallback((stepId: string) => {
    invoke("delete_step", { stepId }).catch(() => {});
    setSteps((prev) => prev.filter((s) => s.id !== stepId));
  }, []);

  if (!loaded) return null;

  return (
    <div className="editor-body">
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
              onDelete={handleDelete}
            />
          ))}
        </div>
      )}
    </div>
  );
}
