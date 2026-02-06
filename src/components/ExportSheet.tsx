import { useState } from "react";

type ExportFormat = "html" | "md" | "pdf";

interface ExportSheetProps {
  stepCount: number;
  exporting: boolean;
  onExport: (title: string, format: ExportFormat) => void;
  onClose: () => void;
}

const FORMAT_OPTIONS: { value: ExportFormat; label: string }[] = [
  { value: "html", label: "HTML" },
  { value: "md", label: "MD" },
  { value: "pdf", label: "PDF" },
];

export default function ExportSheet({ stepCount, exporting, onExport, onClose }: ExportSheetProps) {
  const [title, setTitle] = useState("New StepCast Guide");
  const [format, setFormat] = useState<ExportFormat>(
    () => (localStorage.getItem("exportFormat") as ExportFormat) || "pdf"
  );

  const selectFormat = (f: ExportFormat) => {
    setFormat(f);
    localStorage.setItem("exportFormat", f);
  };

  const titleValid = title.trim().length > 0;

  return (
    <div className="export-overlay" onClick={exporting ? undefined : onClose}>
      <div className="export-sheet" onClick={(e) => e.stopPropagation()}>
        <h2 className="export-sheet-title">Export Guide</h2>

        <label className="field">
          Title
          <input
            className="title-input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Guide title..."
            autoFocus
            disabled={exporting}
          />
        </label>

        <div className="field">
          Format
          <div className="segmented-control">
            {FORMAT_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                className={`segmented-option${format === opt.value ? " active" : ""}`}
                onClick={() => selectFormat(opt.value)}
                disabled={exporting}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        <div className="muted">{stepCount} step{stepCount !== 1 ? "s" : ""}</div>

        <div className="export-sheet-actions">
          <button className="button" onClick={onClose} disabled={exporting}>
            Cancel
          </button>
          <button
            className="button primary"
            onClick={() => onExport(title.trim(), format)}
            disabled={!titleValid || exporting}
          >
            {exporting ? "Exporting..." : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
