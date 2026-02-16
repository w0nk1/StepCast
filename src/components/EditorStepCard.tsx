import { memo, useEffect, useMemo, useRef, useState } from "react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { convertFileSrc } from "@tauri-apps/api/core";
import CropEditorModal from "./CropEditorModal";
import StepScreenshot from "./StepScreenshot";
import type { BoundsPercent, Step } from "../types/step";
import { useI18n } from "../i18n";

type EditorStepCardProps = {
  step: Step;
  index: number;
  onUpdateNote: (stepId: string, note: string | null) => void;
  onUpdateDescription: (stepId: string, description: string | null) => void;
  onGenerateDescription: (stepId: string) => void;
  onUpdateCrop: (stepId: string, cropRegion: BoundsPercent | null) => void;
  aiEnabled: boolean;
  onDelete: (stepId: string) => void;
  collapsed?: boolean;
  onToggleCollapse?: (stepId: string) => void;
  isFocused?: boolean;
  isSelected?: boolean;
  isSelectionActive?: boolean;
  onToggleSelect?: (stepId: string, shiftKey: boolean) => void;
};

export default memo(function EditorStepCard({
  step,
  index,
  onUpdateNote,
  onUpdateDescription,
  onGenerateDescription,
  onUpdateCrop,
  aiEnabled,
  onDelete,
  collapsed = false,
  onToggleCollapse,
  isFocused = false,
  isSelected = false,
  isSelectionActive = false,
  onToggleSelect,
}: EditorStepCardProps) {
  const { t } = useI18n();
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: step.id });

  const sortableStyle = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : undefined,
  };

  const articleRef = useRef<HTMLElement>(null);

  useEffect(() => {
    if (isFocused && articleRef.current) {
      articleRef.current.scrollIntoView?.({ block: "nearest", behavior: "smooth" });
    }
  }, [isFocused]);

  const [noteEditing, setNoteEditing] = useState(false);
  const [noteDraft, setNoteDraft] = useState(step.note ?? "");
  const noteTextareaRef = useRef<HTMLTextAreaElement>(null);

  const [descEditing, setDescEditing] = useState(false);
  const [descDraft, setDescDraft] = useState("");
  const descInputRef = useRef<HTMLInputElement>(null);

  const [cropOpen, setCropOpen] = useState(false);
  const [imageNaturalSize, setImageNaturalSize] = useState<{ width: number; height: number }>({
    width: 0,
    height: 0,
  });

  const screenshotSrc = useMemo(
    () => (step.screenshot_path ? convertFileSrc(step.screenshot_path) : null),
    [step.screenshot_path],
  );

  const isAuthPlaceholder =
    step.window_title === "Authentication dialog (secure)" ||
    step.app.toLowerCase() === "authentication";

  const actionDesc =
    step.action === "DoubleClick"
      ? t("step.action.double_clicked_in")
      : step.action === "RightClick"
        ? t("step.action.right_clicked_in")
        : step.action === "Shortcut"
          ? t("step.action.shortcut_in")
          : t("step.action.clicked_in");

  const authDescription =
    step.description && step.description.trim().length > 0
      ? step.description.trim()
      : t("step.auth.default");

  const effectiveDescription = isAuthPlaceholder
    ? authDescription
    : (step.description && step.description.trim().length > 0
        ? step.description.trim()
        : `${actionDesc} ${step.app}`);

  const isGenerating = step.description_status === "generating";
  const isFailed = step.description_status === "failed";
  const isAi = step.description_source === "ai";

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
    if (e.key === "Escape") setNoteEditing(false);
  };

  const handleStartDescEdit = () => {
    if (isAuthPlaceholder || isGenerating) return;
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
    if (e.key === "Enter") { e.preventDefault(); handleSaveDesc(); }
    if (e.key === "Escape") setDescEditing(false);
  };

  const shouldShowCropBtn = !isAuthPlaceholder && Boolean(step.screenshot_path);

  const articleClass = [
    "editor-step",
    isGenerating && "is-generating",
    isFocused && "is-focused",
    isSelected && "is-selected",
  ].filter(Boolean).join(" ");

  return (
    <div className="editor-timeline-item" ref={setNodeRef} style={sortableStyle} {...attributes}>
      <button className="editor-drag-handle" {...listeners} title={t("step.drag_reorder_title")}>
        <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor">
          <circle cx="3" cy="2" r="1.5"/><circle cx="7" cy="2" r="1.5"/>
          <circle cx="3" cy="7" r="1.5"/><circle cx="7" cy="7" r="1.5"/>
          <circle cx="3" cy="12" r="1.5"/><circle cx="7" cy="12" r="1.5"/>
        </svg>
      </button>
      <div className="editor-timeline-badge">
        {step.action === "Note" ? (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
            <path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
          </svg>
        ) : (
          index + 1
        )}
        {(step.action === "DoubleClick" || step.action === "RightClick" || step.action === "Shortcut") && (
          <span className="editor-badge-action" title={step.action === "DoubleClick" ? t("step.badge.double_click") : step.action === "RightClick" ? t("step.badge.right_click") : t("step.badge.shortcut")}>
            {step.action === "DoubleClick" ? (
              <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                <circle cx="12" cy="12" r="10" /><circle cx="12" cy="12" r="4" />
              </svg>
            ) : step.action === "RightClick" ? (
              <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                <rect x="3" y="3" width="18" height="18" rx="2" strokeDasharray="4 2" />
              </svg>
            ) : (
              <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                <rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h0M10 10h4" />
              </svg>
            )}
          </span>
        )}
      </div>

      <article ref={articleRef} className={articleClass}>
        <div className="editor-step-header">
          {onToggleSelect && (
            <button
              className={`editor-step-checkbox${isSelected ? " is-checked" : ""}${isSelectionActive ? " is-visible" : ""}`}
              onClick={(e) => onToggleSelect(step.id, e.shiftKey)}
              title={isSelected ? t("step.deselect") : t("step.select")}
            >
              {isSelected && (
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              )}
            </button>
          )}
          {onToggleCollapse && (
            <button
              className={`editor-step-collapse${collapsed ? " is-collapsed" : ""}`}
              onClick={() => onToggleCollapse(step.id)}
              title={collapsed ? t("step.expand") : t("step.collapse")}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                <path d="M6 9l6 6 6-6" />
              </svg>
            </button>
          )}
          {descEditing ? (
            <input
              ref={descInputRef}
              className="editor-step-desc-input"
              value={descDraft}
              onChange={(e) => setDescDraft(e.target.value)}
              onBlur={handleSaveDesc}
              onKeyDown={handleDescKeyDown}
              placeholder={t("step.description.placeholder")}
            />
          ) : (
            <button
              className="editor-step-desc editor-step-desc-btn"
              onClick={handleStartDescEdit}
              title={isAuthPlaceholder ? undefined : t("step.description.edit_title")}
              disabled={isAuthPlaceholder || isGenerating}
            >
              {isGenerating ? (
                <span className="editor-step-desc-shimmer">
                  <span className="shimmer-bar" />
                  <span className="shimmer-bar short" />
                </span>
              ) : (
                effectiveDescription
              )}
            </button>
          )}

          <div className="editor-step-header-actions">
            {step.note && (
              <span
                className="editor-step-note-indicator"
                title={step.note.length > 50 ? `${step.note.slice(0, 50)}…` : step.note}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
                </svg>
              </span>
            )}
            {shouldShowCropBtn && (
              <button
                className={`editor-step-crop${step.crop_region ? " is-cropped" : ""}`}
                onClick={() => setCropOpen(true)}
                title={step.crop_region ? t("step.crop.adjusted_title") : t("step.crop.adjust_title")}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M6 2v14a2 2 0 0 0 2 2h14" />
                  <path d="M18 22V8a2 2 0 0 0-2-2H2" />
                </svg>
              </button>
            )}
            {isGenerating && (
              <span className="editor-step-pill generating" title={t("step.ai.generating_title")}>{t("step.ai.generating_pill")}</span>
            )}
            {!isGenerating && isAi && (
              <span className="editor-step-pill ai" title={t("step.ai.generated_pill")}>{t("step.ai.generated_pill")}</span>
            )}
            {isFailed && (
              <>
                <span className="editor-step-pill error" title={step.description_error ?? t("step.ai.failed_default")}>{t("step.ai.failed_pill")}</span>
                <button
                  className="editor-step-retry"
                  onClick={() => onGenerateDescription(step.id)}
                  disabled={!aiEnabled}
                  title={aiEnabled ? t("step.ai.retry_enabled_title") : t("step.ai.retry_disabled_title")}
                >
                  {t("step.ai.retry")}
                </button>
              </>
            )}
            <button
              className="editor-step-ai"
              onClick={() => onGenerateDescription(step.id)}
              disabled={!aiEnabled || isGenerating || isAuthPlaceholder}
              title={!aiEnabled ? t("step.ai.button_disabled_title") : isGenerating ? t("step.ai.button_generating_title") : t("step.ai.button_default_title")}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" focusable="false">
                <path d="M10 2l1.5 4.5L16 8l-4.5 1.5L10 14l-1.5-4.5L4 8l4.5-1.5L10 2z" />
                <path d="M18 12l1 3 3 1-3 1-1 3-1-3-3-1 3-1 1-3z" />
              </svg>
            </button>
            <button className="editor-step-delete" onClick={() => onDelete(step.id)} title={t("step.delete.remove_title")}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>

        {!collapsed && (
          <>
            <StepScreenshot
              step={step}
              index={index}
              isAuthPlaceholder={isAuthPlaceholder}
              onUpdateCrop={onUpdateCrop}
              onImageNaturalSize={setImageNaturalSize}
              imageNaturalSize={imageNaturalSize}
            />
            <div className="editor-step-note-row">
              {noteEditing ? (
                <textarea
                  ref={noteTextareaRef}
                  className="editor-step-note-input"
                  value={noteDraft}
                  onChange={(e) => setNoteDraft(e.target.value)}
                  onBlur={handleSaveNote}
                  onKeyDown={handleNoteKeyDown}
                  placeholder={t("step.note.placeholder")}
                  rows={2}
                />
              ) : (
                <button
                  className={`editor-step-note-btn${step.note ? " has-note" : ""}`}
                  onClick={handleStartNoteEdit}
                >
                  {step.note || t("step.note.button_default")}
                </button>
              )}
            </div>
          </>
        )}
      </article>

      {cropOpen && screenshotSrc && (
        <CropEditorModal
          screenshotSrc={screenshotSrc}
          stepIndex={index}
          initialCropRegion={step.crop_region}
          clickXPercent={step.click_x_percent}
          clickYPercent={step.click_y_percent}
          action={step.action}
          onSave={(crop) => { onUpdateCrop(step.id, crop); setCropOpen(false); }}
          onReset={() => { onUpdateCrop(step.id, null); setCropOpen(false); }}
          onClose={() => setCropOpen(false)}
          onImageError={() => {}}
          onImageLoad={(w, h) => setImageNaturalSize({ width: w, height: h })}
        />
      )}
    </div>
  );
});
