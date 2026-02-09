import React from "react";
import ReactDOM from "react-dom/client";
import EditorWindow from "./components/EditorWindow";
import { initTheme } from "./components/SettingsSheet";
import "./App.css";
import "./editor.css";

initTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <EditorWindow />
  </React.StrictMode>,
);
