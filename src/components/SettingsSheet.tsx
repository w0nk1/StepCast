import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { openUrl } from "@tauri-apps/plugin-opener";

type Theme = "light" | "dark" | "system";
type UpdateStatus = "idle" | "checking" | "available" | "installing" | "up-to-date" | "error";
type AiEligibility = { eligible: boolean; reason: string; details?: string };

interface SettingsSheetProps {
  onBack: () => void;
}

const THEME_OPTIONS: { value: Theme; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

const APPLE_INTELLIGENCE_SETTINGS_URL = "x-apple.systempreferences:com.apple.Siri-Settings.extension";
const SIRI_SETTINGS_FALLBACK_URL = "x-apple.systempreferences:com.apple.preference.siri";
const AI_GRADIENT_ID = "stepcast-ai-gradient";

function AppleToggle(props: {
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
  "aria-label": string;
}) {
  const { checked, disabled, onChange } = props;
  return (
    <button
      type="button"
      className={`apple-toggle${checked ? " on" : ""}`}
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation(); // allow clicking the whole row without double-toggling
        onChange(!checked);
      }}
      aria-label={props["aria-label"]}
    >
      <span className="apple-toggle-thumb" aria-hidden="true" />
    </button>
  );
}

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
  const [appVersion, setAppVersion] = useState("");
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [aiEnabled, setAiEnabled] = useState<boolean>(
    () => localStorage.getItem("appleIntelligenceDescriptions") === "true"
  );
  const [aiEligibility, setAiEligibility] = useState<AiEligibility | null>(null);

  useEffect(() => {
    getVersion().then(setAppVersion);
  }, []);

  useEffect(() => {
    localStorage.setItem("appleIntelligenceDescriptions", String(aiEnabled));
    emit("ai-toggle-changed", { enabled: aiEnabled }).catch(() => {});
  }, [aiEnabled]);

  useEffect(() => {
    invoke<AiEligibility | null>("get_apple_intelligence_eligibility")
      .then((result) => {
        if (result && typeof result.eligible === "boolean" && typeof result.reason === "string") {
          setAiEligibility(result);
        } else {
          setAiEligibility({ eligible: false, reason: "Could not check system eligibility." });
        }
      })
      .catch(() => setAiEligibility({ eligible: false, reason: "Could not check system eligibility." }));
  }, []);

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

  const openAppleIntelligenceSettings = useCallback(async () => {
    try {
      await openUrl(APPLE_INTELLIGENCE_SETTINGS_URL);
    } catch {
      try {
        await openUrl(SIRI_SETTINGS_FALLBACK_URL);
      } catch {
        // Ignore — best-effort
      }
    }
  }, []);

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
            <span className="muted">Version {appVersion}</span>
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

      {/* Apple Intelligence */}
      <section className="panel-card">
        <div className="settings-section">
          <div className="settings-label ai-settings-label">
            <svg
              className="ai-icon"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              aria-hidden="true"
              focusable="false"
            >
              <defs>
                {/* Not Apple’s trademark mark; just a generic sparkle with a gradient. */}
                <linearGradient id={AI_GRADIENT_ID} x1="0" y1="0" x2="1" y2="1">
                  <stop offset="0" stopColor="#ff4fd8" />
                  <stop offset="0.55" stopColor="#7a5cff" />
                  <stop offset="1" stopColor="#00d1ff" />
                </linearGradient>
              </defs>
              <path
                fill={`url(#${AI_GRADIENT_ID})`}
                d="M12 2.2l1.55 5.05 5.05 1.55-5.05 1.55L12 15.4l-1.55-5.05L5.4 8.8l5.05-1.55L12 2.2z"
              />
              <path
                fill={`url(#${AI_GRADIENT_ID})`}
                d="M5.5 14l1.05 3.2 3.2 1.05-3.2 1.05L5.5 22.5 4.45 19.3 1.25 18.25l3.2-1.05L5.5 14z"
              />
              <path
                fill={`url(#${AI_GRADIENT_ID})`}
                d="M18.5 14l1.05 3.2 3.2 1.05-3.2 1.05-1.05 3.2-1.05-3.2-3.2-1.05 3.2-1.05L18.5 14z"
              />
            </svg>
            Apple Intelligence
          </div>
          <div
            className="settings-row settings-row-switch"
            role="button"
            tabIndex={0}
            onClick={() => setAiEnabled((v) => !v)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                setAiEnabled((v) => !v);
              }
            }}
            aria-label="Toggle Apple Intelligence step descriptions"
          >
            <span>Use for step descriptions</span>
            <AppleToggle
              checked={aiEnabled}
              onChange={setAiEnabled}
              aria-label="Apple Intelligence step descriptions"
            />
          </div>
          <div className="muted">
            {aiEligibility
              ? aiEligibility.eligible
                ? "No extra permissions. Requires Apple Intelligence enabled in System Settings. Runs on-device (offline). Step data stays on your Mac."
                : `${aiEligibility.reason} No extra permissions. StepCast will use built-in descriptions. Step data stays on your Mac.`
              : "Checking eligibility..."}
          </div>
          {aiEligibility?.details && <div className="muted">{aiEligibility.details}</div>}
          <button className="button ghost" onClick={openAppleIntelligenceSettings}>
            Open Apple Intelligence &amp; Siri Settings
          </button>
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
