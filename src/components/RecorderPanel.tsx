import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { save, ask } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import StepItem from "./StepItem";
import type { Step } from "../types/step";

// Icon components
const RecordIcon = () => (
  <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
    <path d="M12 22C6.47715 22 2 17.5228 2 12C2 6.47715 6.47715 2 12 2C17.5228 2 22 6.47715 22 12C22 17.5228 17.5228 22 12 22ZM12 15C13.6569 15 15 13.6569 15 12C15 10.3431 13.6569 9 12 9C10.3431 9 9 10.3431 9 12C9 13.6569 10.3431 15 12 15Z"/>
  </svg>
);

const StopIcon = () => (
  <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
    <path d="M12 22C6.47715 22 2 17.5228 2 12C2 6.47715 6.47715 2 12 2C17.5228 2 22 6.47715 22 12C22 17.5228 17.5228 22 12 22ZM9 9V15H15V9H9Z"/>
  </svg>
);

const PauseIcon = () => (
  <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
    <path d="M12 22C6.47715 22 2 17.5228 2 12C2 6.47715 6.47715 2 12 2C17.5228 2 22 6.47715 22 12C22 17.5228 17.5228 22 12 22ZM9 9V15H11V9H9ZM13 9V15H15V9H13Z"/>
  </svg>
);

const PlayIcon = () => (
  <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
    <path d="M12 22C6.47715 22 2 17.5228 2 12C2 6.47715 6.47715 2 12 2C17.5228 2 22 6.47715 22 12C22 17.5228 17.5228 22 12 22ZM10.6219 8.41459C10.5562 8.37078 10.479 8.34741 10.4 8.34741C10.1791 8.34741 10 8.52649 10 8.74741V15.2526C10 15.3316 10.0234 15.4088 10.0672 15.4745C10.1897 15.6583 10.4381 15.708 10.6219 15.5854L15.5008 12.3328C15.5447 12.3035 15.5824 12.2658 15.6117 12.2219C15.7343 12.0381 15.6846 11.7897 15.5008 11.6672L10.6219 8.41459Z"/>
  </svg>
);

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
      setSteps((prev) => {
        // Deduplicate by ID to prevent accumulation bugs
        const exists = prev.some((s) => s.id === event.payload.id);
        if (exists) {
          return prev;
        }
        return [...prev, event.payload];
      });
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

  const handleExportHtml = useCallback(async () => {
    setError(null);
    try {
      const path = await save({
        defaultPath: "stepcast-guide.html",
        filters: [{ name: "HTML", extensions: ["html"] }],
      });
      if (path) {
        await invoke("export_html", { title, outputPath: path });
      }
    } catch (err) {
      setError(String(err));
    }
  }, [title]);

  const handleExportMarkdown = useCallback(async () => {
    setError(null);
    try {
      const path = await save({
        defaultPath: "stepcast-guide.md",
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (path) {
        await invoke("export_markdown", { title, outputPath: path });
      }
    } catch (err) {
      setError(String(err));
    }
  }, [title]);

  const handleExportPdf = useCallback(async () => {
    setError(null);
    try {
      // For PDF: export HTML to temp, then open in browser for Print to PDF
      const tempPath = await invoke<string>("export_html_temp", { title });
      await openPath(tempPath);
    } catch (err) {
      setError(String(err));
    }
  }, [title]);

  const handleDiscard = useCallback(async () => {
    const confirmed = await ask(
      `Are you sure you want to discard ${steps.length} captured step${steps.length === 1 ? "" : "s"}? This cannot be undone.`,
      {
        title: "Discard Recording",
        kind: "warning",
        okLabel: "Discard",
        cancelLabel: "Cancel",
      }
    );

    if (confirmed) {
      try {
        // Use dedicated discard command that properly clears everything
        await invoke("discard_recording");
      } catch {
        // Ignore errors when discarding
      }
      // Clear frontend state
      setSteps([]);
      setStatus("idle");
      setError(null);
    }
  }, [steps.length]);

  const canDiscard = steps.length > 0 || status === "recording" || status === "paused";

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
              <RecordIcon />
              Start Recording
            </button>
          )}

          {status === "recording" && (
            <>
              <button
                className="button"
                onClick={() => handleCommand("pause", "paused")}
              >
                <PauseIcon />
                Pause
              </button>
              <button
                className="button danger"
                onClick={() => handleCommand("stop", "stopped")}
              >
                <StopIcon />
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
                <PlayIcon />
                Resume
              </button>
              <button
                className="button danger"
                onClick={() => handleCommand("stop", "stopped")}
              >
                <StopIcon />
                Stop
              </button>
            </>
          )}
        </div>

        {/* Steps List */}
        <div className="steps">
          <div className="steps-header">
            <h2>Steps</h2>
            <div className="steps-header-right">
              <span className="muted">{steps.length} captured</span>
              {canDiscard && (
                <button
                  className="button-icon danger"
                  onClick={handleDiscard}
                  title="Discard recording"
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/>
                  </svg>
                </button>
              )}
            </div>
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
