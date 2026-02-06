import { useCallback, useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { openUrl } from "@tauri-apps/plugin-opener";

type Theme = "light" | "dark" | "system";
type UpdateStatus = "idle" | "checking" | "available" | "installing" | "up-to-date" | "error";

interface SettingsSheetProps {
  onBack: () => void;
}

const THEME_OPTIONS: { value: Theme; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

const APP_VERSION = "0.1.0";

function applyTheme(theme: Theme) {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", theme);
  }
  localStorage.setItem("theme", theme);
}

export function initTheme() {
  const saved = localStorage.getItem("theme") as Theme | null;
  if (saved && saved !== "system") {
    document.documentElement.setAttribute("data-theme", saved);
  }
}

export default function SettingsSheet({ onBack }: SettingsSheetProps) {
  const [theme, setTheme] = useState<Theme>(
    () => (localStorage.getItem("theme") as Theme) || "system"
  );
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);

  const selectTheme = useCallback((t: Theme) => {
    setTheme(t);
    applyTheme(t);
  }, []);

  const handleCheckUpdate = useCallback(async () => {
    setUpdateStatus("checking");
    try {
      const update = await check();
      if (update?.available) {
        setUpdateVersion(update.version);
        setUpdateStatus("available");
      } else {
        setUpdateStatus("up-to-date");
      }
    } catch {
      setUpdateStatus("error");
    }
  }, []);

  const handleInstallUpdate = useCallback(async () => {
    setUpdateStatus("installing");
    try {
      const update = await check();
      if (update?.available) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } catch {
      setUpdateStatus("error");
    }
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onBack();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onBack]);

  return (
    <>
      {/* Header with back button */}
      <header className="panel-header">
        <button className="settings-back" onClick={onBack} title="Back">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M15 18l-6-6 6-6" />
          </svg>
          <span>Settings</span>
        </button>
      </header>

      {/* Appearance */}
      <section className="panel-card">
        <div className="settings-section">
          <div className="settings-label">Appearance</div>
          <div className="segmented-control">
            {THEME_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                className={`segmented-option${theme === opt.value ? " active" : ""}`}
                onClick={() => selectTheme(opt.value)}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>
      </section>

      {/* Updates */}
      <section className="panel-card">
        <div className="settings-section">
          <div className="settings-label">Updates</div>
          <div className="settings-row">
            <span className="muted">Version {APP_VERSION}</span>
          </div>
          {updateStatus === "available" && updateVersion ? (
            <button className="button primary" onClick={handleInstallUpdate}>
              Install v{updateVersion}
            </button>
          ) : (
            <button
              className="button ghost"
              onClick={handleCheckUpdate}
              disabled={updateStatus === "checking" || updateStatus === "installing"}
            >
              {updateStatus === "checking" ? "Checking..." : updateStatus === "installing" ? "Installing..." : "Check for Updates"}
            </button>
          )}
          {updateStatus === "up-to-date" && (
            <span className="settings-status success">You're up to date</span>
          )}
          {updateStatus === "error" && (
            <span className="settings-status error">Could not check for updates</span>
          )}
        </div>
      </section>

      {/* About */}
      <section className="panel-card">
        <div className="settings-section">
          <div className="settings-label">About</div>
          <div className="settings-links">
            <button className="settings-link" onClick={() => openUrl("https://github.com/w0nk1/StepCast")}>
              GitHub
            </button>
            <span className="settings-link-sep" />
            <button className="settings-link" onClick={() => openUrl("https://github.com/w0nk1/StepCast/issues")}>
              Report a Bug
            </button>
          </div>
        </div>
      </section>
    </>
  );
}
