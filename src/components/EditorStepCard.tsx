import { memo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Step } from "../types/step";

type EditorStepCardProps = {
  step: Step;
  index: number;
  onUpdateNote: (stepId: string, note: string | null) => void;
  onDelete: (stepId: string) => void;
};

export default memo(function EditorStepCard({
  step,
  index,
  onUpdateNote,
  onDelete,
}: EditorStepCardProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(step.note ?? "");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const screenshotSrc = step.screenshot_path
    ? convertFileSrc(step.screenshot_path)
    : null;

  const isAuthPlaceholder =
    step.window_title === "Authentication dialog (secure)" ||
    step.app.toLowerCase() === "authentication";

  const actionDesc =
    step.action === "DoubleClick"
      ? "Double-clicked in"
      : step.action === "RightClick"
        ? "Right-clicked in"
        : "Clicked in";

  const description = isAuthPlaceholder
    ? "Authentication required (secure dialog)"
    : `${actionDesc} ${step.app}`;

  const markerClass =
    step.action === "DoubleClick"
      ? "click-indicator double-click"
      : step.action === "RightClick"
        ? "click-indicator right-click"
        : "click-indicator";

  const handleStartEdit = () => {
    setDraft(step.note ?? "");
    setEditing(true);
    requestAnimationFrame(() => textareaRef.current?.focus());
  };

  const handleSave = () => {
    const trimmed = draft.trim();
    onUpdateNote(step.id, trimmed || null);
    setEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSave();
    }
    if (e.key === "Escape") {
      setEditing(false);
    }
  };

  return (
    <div className="editor-timeline-item">
      {/* Timeline badge */}
      <div className="editor-timeline-badge">{index + 1}</div>

      {/* Card */}
      <article className="editor-step">
        <div className="editor-step-header">
          <span className="editor-step-desc">{description}</span>
          <button
            className="editor-step-delete"
            onClick={() => onDelete(step.id)}
            title="Remove step"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 6L6 18M6 6l12 12"/>
            </svg>
          </button>
        </div>

        {/* Screenshot with click marker */}
        {screenshotSrc && (
          <div className="editor-step-image">
            <div className="editor-image-wrapper">
              <img src={screenshotSrc} alt={`Step ${index + 1}`} />
              {!isAuthPlaceholder && (
                <div
                  className={markerClass}
                  style={{
                    left: `${step.click_x_percent}%`,
                    top: `${step.click_y_percent}%`,
                  }}
                />
              )}
            </div>
          </div>
        )}

        {/* Note editing */}
        <div className="editor-step-note-row">
          {editing ? (
            <textarea
              ref={textareaRef}
              className="editor-step-note-input"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={handleSave}
              onKeyDown={handleKeyDown}
              placeholder="Add a note..."
              rows={2}
            />
          ) : (
            <button
              className={`editor-step-note-btn${step.note ? " has-note" : ""}`}
              onClick={handleStartEdit}
            >
              {step.note || "Add a note..."}
            </button>
          )}
        </div>
      </article>
    </div>
  );
});
