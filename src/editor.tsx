import React from "react";
import ReactDOM from "react-dom/client";
import EditorWindow from "./components/EditorWindow";
import { initTheme } from "./components/SettingsSheet";
import { I18nProvider } from "./i18n";
import "./App.css";
import "./editor.css";
import "react-image-crop/dist/ReactCrop.css";

initTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider>
      <EditorWindow />
    </I18nProvider>
  </React.StrictMode>,
);
