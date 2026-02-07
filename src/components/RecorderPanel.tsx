import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { save, ask } from "@tauri-apps/plugin-dialog";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import StepItem from "./StepItem";
import ExportSheet from "./ExportSheet";
import SettingsSheet from "./SettingsSheet";
import type { Step } from "../types/step";

const SettingsIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
  </svg>
);

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

const ExportIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
    <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/>
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
  const [steps, setSteps] = useState<Step[]>([]);
  const [showExportSheet, setShowExportSheet] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [updateAvailable, setUpdateAvailable] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);
  const [showSettings, setShowSettings] = useState(false);


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

  // Poll permissions every 2s while any are missing
  useEffect(() => {
    if (permissionsReady) return;
    const id = setInterval(refreshPermissions, 2000);
    return () => clearInterval(id);
  }, [permissionsReady, refreshPermissions]);

  useEffect(() => {
    check()
      .then((update) => {
        if (update?.available) {
          setUpdateAvailable(update.version);
        }
      })
      .catch(() => {
        // Silently ignore update check errors (offline, no releases yet, etc.)
      });
  }, []);

  const handleUpdate = useCallback(async () => {
    setUpdating(true);
    try {
      const update = await check();
      if (update?.available) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } catch {
      setUpdating(false);
    }
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    listen<Step>("step-captured", (event) => {
      setSteps((prev) => {
        const exists = prev.some((s) => s.id === event.payload.id);
        if (exists) {
          return prev;
        }
        return [...prev, event.payload];
      });
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
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

  const handleRequestScreenRecording = useCallback(async () => {
    setError(null);
    try {
      const next = await invoke<PermissionStatus>("request_screen_recording");
      setPermissions(next);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const handleRequestAccessibility = useCallback(async () => {
    setError(null);
    try {
      const next = await invoke<PermissionStatus>("request_accessibility");
      setPermissions(next);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const handleExport = useCallback(async (title: string, format: "html" | "md" | "pdf") => {
    setError(null);
    setExporting(true);
    try {
      const ext = { html: "html", md: "zip", pdf: "pdf" }[format];
      const name = { html: "HTML", md: "Markdown Archive", pdf: "PDF" }[format];
      const path = await save({
        defaultPath: `${title}.${ext}`,
        filters: [{ name, extensions: [ext] }],
      });
      if (!path) return;
      await invoke("export_guide", { title, format, outputPath: path });
      setShowExportSheet(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setExporting(false);
    }
  }, []);

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
        await invoke("discard_recording");
      } catch {
        // Ignore errors when discarding
      }
      setSteps([]);
      setStatus("idle");
      setError(null);
    }
  }, [steps.length]);

  const handleNewRecording = useCallback(async () => {
    if (steps.length > 0) {
      const confirmed = await ask(
        `Starting a new recording will discard ${steps.length} captured step${steps.length === 1 ? "" : "s"}. Continue?`,
        {
          title: "New Recording",
          kind: "warning",
          okLabel: "Discard & Record",
          cancelLabel: "Cancel",
        }
      );
      if (!confirmed) return;
    }
    handleCommand("start", "recording");
  }, [steps.length, handleCommand]);

  const handleDeleteStep = useCallback((id: string) => {
    setSteps((prev) => {
      const next = prev.filter((s) => s.id !== id);
      if (next.length === 0) {
        setStatus("idle");
      }
      return next;
    });
  }, []);

  const canDiscard = steps.length > 0 || status === "recording" || status === "paused";
  const isIdle = steps.length === 0 && (status === "idle" || status === "stopped");
  const isRecordingOrPaused = status === "recording" || status === "paused";
  const isStopped = status === "stopped" && steps.length > 0;

  if (showSettings) {
    return (
      <main className="panel">
        <SettingsSheet onBack={() => setShowSettings(false)} />
      </main>
    );
  }

  return (
    <main className={`panel${isIdle ? "" : " panel-full"}`}>
      {/* Header */}
      <header className="panel-header">
        <h1 className="panel-title">StepCast</h1>
        <div className="panel-header-right">
          <div className="status-chip" data-tone={STATUS_TONES[status]}>
            {status === "recording" && <span className="rec-dot" />}
            {STATUS_LABELS[status]}
          </div>
          <button
            className="button-icon"
            onClick={() => setShowSettings(true)}
            title="Settings"
          >
            <SettingsIcon />
          </button>
        </div>
      </header>

      {/* Update banner */}
      {updateAvailable && (
        <div className="update-banner">
          <span>v{updateAvailable} available</span>
          <button
            className="button ghost"
            onClick={handleUpdate}
            disabled={updating}
          >
            {updating ? "Updating..." : "Install"}
          </button>
        </div>
      )}

      {/* Permissions - only show if missing */}
      {missingPermissions.length > 0 && (
        <section className="panel-card">
          <div className="permissions">
            <div className="permission-banner warn">
              Missing: {missingPermissions.join(", ")}
            </div>
            <div className="permission-row">
              <span>Screen Recording</span>
              {permissions?.screen_recording ? (
                <span className="pill ok">OK</span>
              ) : (
                <button className="pill-button warn" onClick={handleRequestScreenRecording}>
                  Open Settings
                </button>
              )}
            </div>
            <div className="permission-row">
              <span>Accessibility</span>
              {permissions?.accessibility ? (
                <span className="pill ok">OK</span>
              ) : (
                <button className="pill-button warn" onClick={handleRequestAccessibility}>
                  Open Settings
                </button>
              )}
            </div>
          </div>
        </section>
      )}

      {/* Idle State */}
      {isIdle && (
        <div className="idle-view">
          <button
            className="button primary idle-cta"
            onClick={() => handleCommand("start", "recording")}
            disabled={!permissionsReady}
          >
            <RecordIcon />
            Start Recording
          </button>
          <p className="idle-hint">Click anywhere on your screen to capture steps</p>
        </div>
      )}

      {/* Recording/Paused State */}
      {isRecordingOrPaused && (
        <section className="panel-card" style={{ flex: 1, minHeight: 0 }}>
          <div className="controls">
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
                  <StepItem key={step.id} step={step} index={index} onDelete={handleDeleteStep} />
                ))}
              </div>
            )}
          </div>
        </section>
      )}

      {/* Stopped State */}
      {isStopped && (
        <>
          <section className="panel-card" style={{ flex: 1, minHeight: 0 }}>
            <div className="steps">
              <div className="steps-header">
                <h2>Steps</h2>
                <span className="muted">{steps.length} captured</span>
              </div>
              <div className="steps-list">
                {steps.map((step, index) => (
                  <StepItem key={step.id} step={step} index={index} onDelete={handleDeleteStep} />
                ))}
              </div>
            </div>
          </section>

          <div className="action-bar">
            <button
              className="button"
              onClick={handleNewRecording}
              disabled={!permissionsReady}
            >
              <RecordIcon />
              New Recording
            </button>
            <button
              className="button primary"
              onClick={() => setShowExportSheet(true)}
            >
              <ExportIcon />
              Export
            </button>
          </div>
        </>
      )}

      {/* Export Sheet Overlay */}
      {showExportSheet && (
        <ExportSheet
          stepCount={steps.length}
          exporting={exporting}
          onExport={handleExport}
          onClose={() => setShowExportSheet(false)}
        />
      )}

      {error && <div className="error-banner">{error}</div>}
    </main>
  );
}
