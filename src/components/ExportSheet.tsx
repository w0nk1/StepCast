import { useState } from "react";
import { useI18n } from "../i18n";

type ExportFormat = "html" | "md" | "pdf";

interface ExportSheetProps {
  stepCount: number;
  exporting: boolean;
  onExport: (title: string, format: ExportFormat) => void;
  onClose: () => void;
}

const FORMAT_OPTIONS: ExportFormat[] = ["html", "md", "pdf"];

export default function ExportSheet({ stepCount, exporting, onExport, onClose }: ExportSheetProps) {
  const { t } = useI18n();
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
        <h2 className="export-sheet-title">{t("export.title")}</h2>

        <label className="field">
          {t("export.field.title")}
          <input
            className="title-input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={t("export.placeholder.title")}
            autoFocus
            disabled={exporting}
          />
        </label>

        <div className="field">
          {t("export.field.format")}
          <div className="segmented-control">
            {FORMAT_OPTIONS.map((opt) => (
              <button
                key={opt}
                className={`segmented-option${format === opt ? " active" : ""}`}
                onClick={() => selectFormat(opt)}
                disabled={exporting}
              >
                {t(`export.format.${opt}`)}
              </button>
            ))}
          </div>
        </div>

        <div className="muted">{t("export.steps_count", { count: stepCount })}</div>

        <div className="export-sheet-actions">
          <button className="button" onClick={onClose} disabled={exporting}>
            {t("common.cancel")}
          </button>
          <button
            className="button primary"
            onClick={() => onExport(title.trim(), format)}
            disabled={!titleValid || exporting}
          >
            {exporting ? t("common.exporting") : t("common.export")}
          </button>
        </div>
      </div>
    </div>
  );
}
