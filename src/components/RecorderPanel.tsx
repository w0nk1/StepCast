import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { renderHtml, renderMarkdown } from "../export/render";
import StepItem from "./StepItem";
import type { Step } from "../types/step";

type PermissionStatus = {
  screen_recording: boolean;
  accessibility: boolean;
};

type RecorderStatus = "idle" | "recording" | "paused" | "stopped";

const STATUS_LABELS: Record<RecorderStatus, string> = {
  idle: "Ready",
  recording: "Recording",
  paused: "Paused",
  stopped: "Stopped",
};

const STATUS_TONES: Record<RecorderStatus, "quiet" | "active" | "warn"> = {
  idle: "quiet",
  recording: "active",
  paused: "warn",
  stopped: "quiet",
};

const COMMANDS = {
  start: "start_recording",
  pause: "pause_recording",
  resume: "resume_recording",
  stop: "stop_recording",
} as const;

type RecorderCommand = keyof typeof COMMANDS;

function downloadText(filename: string, contents: string, mime: string) {
  const blob = new Blob([contents], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

export default function RecorderPanel() {
  const [permissions, setPermissions] = useState<PermissionStatus | null>(null);
  const [status, setStatus] = useState<RecorderStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [title, setTitle] = useState("New StepCast Guide");
  const [steps, setSteps] = useState<Step[]>([]);

  const permissionsReady = Boolean(
    permissions && permissions.screen_recording && permissions.accessibility,
  );

  const refreshPermissions = useCallback(async () => {
    try {
      const next = await invoke<PermissionStatus>("check_permissions");
      setPermissions(next);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    refreshPermissions();
  }, [refreshPermissions]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    listen<Step>("step-captured", (event) => {
      setSteps((prev) => [...prev, event.payload]);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  const missingPermissions = useMemo(() => {
    if (!permissions) {
      return [] as string[];
    }
    const missing = [] as string[];
    if (!permissions.screen_recording) missing.push("Screen Recording");
    if (!permissions.accessibility) missing.push("Accessibility");
    return missing;
  }, [permissions]);

  const handleCommand = useCallback(
    async (command: RecorderCommand, nextStatus?: RecorderStatus) => {
      setError(null);
      if (command === "start") {
        setSteps([]);
      }
      try {
        await invoke(COMMANDS[command]);
        if (nextStatus) {
          setStatus(nextStatus);
        }
      } catch (err) {
        const message = String(err);
        if (message.includes("missing screen recording")) {
          setError("Grant Screen Recording and Accessibility permissions to record.");
        } else {
          setError(message);
        }
      }
    },
    [],
  );

  const handleRequestPermissions = useCallback(async () => {
    setError(null);
    try {
      const next = await invoke<PermissionStatus>("request_permissions");
      setPermissions(next);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const handleExportHtml = useCallback(() => {
    const html = renderHtml(title);
    downloadText("stepcast-guide.html", html, "text/html");
  }, [title]);

  const handleExportMarkdown = useCallback(() => {
    const markdown = renderMarkdown(title);
    downloadText("stepcast-guide.md", markdown, "text/markdown");
  }, [title]);

  const handleExportPdf = useCallback(() => {
    const html = renderHtml(title);
    const preview = window.open("", "_blank", "width=960,height=720");
    if (!preview) {
      setError("Popup blocked. Allow new windows to print.");
      return;
    }
    preview.document.open();
    preview.document.write(html);
    preview.document.close();
    preview.focus();
    setTimeout(() => {
      preview.print();
    }, 120);
  }, [title]);

  return (
    <main className="panel">
      {/* Minimal Header */}
      <header className="panel-header">
        <h1 className="panel-title">StepCast</h1>
        <div className="status-chip" data-tone={STATUS_TONES[status]}>
          {STATUS_LABELS[status]}
        </div>
      </header>

      {/* Permissions - only show if missing */}
      {missingPermissions.length > 0 && (
        <section className="panel-card">
          <div className="permissions">
            <div className="permission-banner warn">
              Missing: {missingPermissions.join(", ")}
            </div>
            <div className="permission-row">
              <span>Screen Recording</span>
              <span className={permissions?.screen_recording ? "pill ok" : "pill warn"}>
                {permissions?.screen_recording ? "OK" : "Missing"}
              </span>
            </div>
            <div className="permission-row">
              <span>Accessibility</span>
              <span className={permissions?.accessibility ? "pill ok" : "pill warn"}>
                {permissions?.accessibility ? "OK" : "Missing"}
              </span>
            </div>
            <button className="button ghost" onClick={handleRequestPermissions}>
              Grant Permissions
            </button>
          </div>
        </section>
      )}

      {/* Controls & Steps */}
      <section className="panel-card" style={{ flex: 1, minHeight: 0 }}>
        {/* Context-dependent buttons */}
        <div className="controls">
          {(status === "idle" || status === "stopped") && (
            <button
              className="button primary"
              onClick={() => handleCommand("start", "recording")}
              disabled={!permissionsReady}
            >
              Start Recording
            </button>
          )}

          {status === "recording" && (
            <>
              <button
                className="button"
                onClick={() => handleCommand("pause", "paused")}
              >
                Pause
              </button>
              <button
                className="button danger"
                onClick={() => handleCommand("stop", "stopped")}
              >
                Stop
              </button>
            </>
          )}

          {status === "paused" && (
            <>
              <button
                className="button primary"
                onClick={() => handleCommand("resume", "recording")}
              >
                Resume
              </button>
              <button
                className="button danger"
                onClick={() => handleCommand("stop", "stopped")}
              >
                Stop
              </button>
            </>
          )}
        </div>

        {/* Steps List */}
        <div className="steps">
          <div className="steps-header">
            <h2>Steps</h2>
            <span className="muted">{steps.length} captured</span>
          </div>
          {steps.length === 0 ? (
            <div className="steps-empty">
              Click anywhere to capture steps.
            </div>
          ) : (
            <div className="steps-list">
              {steps.map((step, index) => (
                <StepItem key={step.id} step={step} index={index} />
              ))}
            </div>
          )}
        </div>
      </section>

      {/* Export with title input */}
      <section className="panel-card export-card">
        <h2>Export</h2>
        <input
          className="title-input"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Guide title..."
        />
        <div className="export-actions">
          <button className="button" onClick={handleExportHtml}>
            HTML
          </button>
          <button className="button" onClick={handleExportMarkdown}>
            MD
          </button>
          <button className="button primary" onClick={handleExportPdf}>
            PDF
          </button>
        </div>
      </section>

      {error && <div className="error-banner">{error}</div>}
    </main>
  );
}
