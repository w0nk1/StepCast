import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface WelcomeBannerProps {
  onDismiss: () => void;
}

export default function WelcomeBanner({ onDismiss }: WelcomeBannerProps) {
  const handleDismiss = useCallback(async () => {
    try {
      await invoke("mark_startup_seen");
    } catch {
      // Best-effort — dismiss locally even if save fails
    }
    onDismiss();
  }, [onDismiss]);

  return (
    <section className="welcome-banner">
      <h2 className="welcome-title">Welcome to StepCast</h2>
      <ul className="welcome-tips">
        <li>Click the menu bar icon to open this panel</li>
        <li>
          Press <kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> from anywhere
        </li>
        <li>Right-click the icon for more options</li>
      </ul>
      <button className="button ghost welcome-dismiss" onClick={handleDismiss}>
        Got it
      </button>
    </section>
  );
}
