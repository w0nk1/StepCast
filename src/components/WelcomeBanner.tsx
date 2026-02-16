import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";

interface WelcomeBannerProps {
  onDismiss: () => void;
}

export default function WelcomeBanner({ onDismiss }: WelcomeBannerProps) {
  const { t } = useI18n();
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
      <h2 className="welcome-title">{t("welcome.title")}</h2>
      <ul className="welcome-tips">
        <li>{t("welcome.tip.menu_bar")}</li>
        <li>
          <span dangerouslySetInnerHTML={{ __html: t("welcome.tip.shortcut") }} />
        </li>
        <li>{t("welcome.tip.right_click")}</li>
        <li>
          {t("welcome.tip.ai")}
        </li>
      </ul>
      <button className="button ghost welcome-dismiss" onClick={handleDismiss}>
        {t("welcome.dismiss")}
      </button>
    </section>
  );
}
