import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { renderHtml, renderMarkdown } from "../export/render";

type PermissionStatus = {
  screen_recording: boolean;
  accessibility: boolean;
};

type RecorderStatus = "idle" | "recording" | "paused" | "stopped";

const STATUS_LABELS: Record<RecorderStatus, string> = {
  idle: "Idle",
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

  const canRecord = permissionsReady && (status === "idle" || status === "stopped");
  const canPause = permissionsReady && status === "recording";
  const canResume = permissionsReady && status === "paused";
  const canStop = permissionsReady && (status === "recording" || status === "paused");

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
      <header className="panel-header">
        <div>
          <p className="eyebrow">StepCast</p>
          <h1 className="panel-title">Capture a clean how-to.</h1>
        </div>
        <div className="status-chip" data-tone={STATUS_TONES[status]}>
          {STATUS_LABELS[status]}
        </div>
      </header>

      <section className="panel-card">
        <label className="field">
          <span>Guide title</span>
          <input
            value={title}
            onChange={(event) => setTitle(event.currentTarget.value)}
            placeholder="Give this guide a name"
          />
        </label>

        <div className="permissions">
          {permissions && missingPermissions.length > 0 && (
            <div className="permission-banner warn">
              Missing: {missingPermissions.join(", ")}. Recording will not start until granted.
            </div>
          )}
          {permissions && missingPermissions.length === 0 && (
            <div className="permission-banner ok">
              All permissions granted. Ready to record.
            </div>
          )}
          <div className="permission-row">
            <span>Screen Recording</span>
            <span className={permissions?.screen_recording ? "pill ok" : "pill warn"}>
              {permissions?.screen_recording ? "Granted" : "Missing"}
            </span>
          </div>
          <div className="permission-row">
            <span>Accessibility</span>
            <span className={permissions?.accessibility ? "pill ok" : "pill warn"}>
              {permissions?.accessibility ? "Granted" : "Missing"}
            </span>
          </div>
          {missingPermissions.length > 0 && (
            <button className="button ghost" onClick={handleRequestPermissions}>
              Request permissions
            </button>
          )}
        </div>
      </section>

      <section className="panel-card">
        <div className="controls">
          <button
            className="button primary"
            onClick={() => handleCommand("start", "recording")}
            disabled={!canRecord}
          >
            Record
          </button>
          <button
            className="button"
            onClick={() => handleCommand("pause", "paused")}
            disabled={!canPause}
          >
            Pause
          </button>
          <button
            className="button"
            onClick={() => handleCommand("resume", "recording")}
            disabled={!canResume}
          >
            Resume
          </button>
          <button
            className="button danger"
            onClick={() => handleCommand("stop", "stopped")}
            disabled={!canStop}
          >
            Stop
          </button>
        </div>

        <div className="steps">
          <div className="steps-header">
            <h2>Steps</h2>
            <span className="muted">0 captured</span>
          </div>
          <div className="steps-empty">
            Waiting for your first click. The list will fill as you record.
          </div>
        </div>
      </section>

      <section className="panel-card export-card">
        <div>
          <h2>Export</h2>
          <p className="muted">Generate HTML, Markdown, or print-ready PDF.</p>
        </div>
        <div className="export-actions">
          <button className="button" onClick={handleExportHtml}>
            HTML
          </button>
          <button className="button" onClick={handleExportMarkdown}>
            Markdown
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
