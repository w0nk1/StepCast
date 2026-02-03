# Panel UI Redesign

## Problem

1. Panel überlappt mit der macOS Menu-Bar (Bug in `position_panel_at_tray_icon`)
2. Panel ist zu groß (720px) für ein Menu-Bar-Tool
3. UI nutzt Platz ineffizient (Permissions immer sichtbar, redundante Buttons)
4. Keine visuelle Verbindung zum Tray-Icon
5. Kein Dunkelmodus-Support

## Design-Entscheidungen

| Aspekt | Entscheidung |
|--------|--------------|
| Position | Bündig unter Tray-Icon, zentriert |
| Höhe | ~520px |
| Header | Minimal: App-Name + Status-Chip |
| Permissions | Ausblenden wenn alle erteilt |
| Buttons | Kontextabhängig (nur relevante zeigen) |
| Steps | Mini-Thumbnails (40x30px) + Text |
| Notch | Subtiler Notch oben mittig |
| Border-Radius | 10-12px |
| Farben | macOS-nativ (grau) + Dunkelmodus |
| Guide-Titel | Im Export-Bereich |

---

## 1. Positionierungs-Fix (Rust)

**Datei:** `src-tauri/src/panel.rs`

**Problem:** `nudge_up_points = 6.0` schiebt Panel nach oben in die Menu-Bar.

**Lösung:**
```rust
// Vorher:
let nudge_up_points = 6.0;
let panel_y_phys = icon_phys_y + icon_height_phys + padding_phys - nudge_up_phys;

// Nachher:
let gap_points = 4.0; // 4pt Abstand unter Menu-Bar
let gap_phys = (gap_points * scale_factor).round() as i32;
let panel_y_phys = icon_phys_y + icon_height_phys + gap_phys;
```

---

## 2. Panel-Größe anpassen

**Datei:** `src-tauri/tauri.conf.json`

```json
{
  "app": {
    "windows": [{
      "width": 320,
      "height": 520
    }]
  }
}
```

Breite von 350 auf 320 reduzieren (kompakter).

---

## 3. CSS-Variablen für Farbschema

**Datei:** `src/App.css`

Neue Struktur mit Light/Dark Mode:

```css
:root {
  /* Light Mode (Standard) */
  --bg-primary: #ffffff;
  --bg-secondary: #f5f5f7;
  --bg-tertiary: #e8e8ed;
  --text-primary: #1d1d1f;
  --text-secondary: #86868b;
  --border: rgba(0, 0, 0, 0.1);
  --accent: #007aff;
  --accent-hover: #0066d6;
  --success: #34c759;
  --warning: #ff9500;
  --danger: #ff3b30;
  --shadow: 0 10px 40px rgba(0, 0, 0, 0.15);
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg-primary: #1c1c1e;
    --bg-secondary: #2c2c2e;
    --bg-tertiary: #3a3a3c;
    --text-primary: #f5f5f7;
    --text-secondary: #98989d;
    --border: rgba(255, 255, 255, 0.1);
    --accent: #0a84ff;
    --accent-hover: #409cff;
    --success: #30d158;
    --warning: #ff9f0a;
    --danger: #ff453a;
    --shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  }
}
```

---

## 4. Panel-Container mit Notch

**Datei:** `src/App.css`

```css
.panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  padding-top: 20px; /* Platz für Notch */
  background: var(--bg-primary);
  border-radius: 12px;
  box-shadow: var(--shadow);
  position: relative;
}

.panel::before {
  content: "";
  position: absolute;
  top: -6px;
  left: 50%;
  transform: translateX(-50%);
  width: 16px;
  height: 8px;
  background: var(--bg-primary);
  clip-path: polygon(50% 0%, 0% 100%, 100% 100%);
  box-shadow: var(--shadow);
}
```

---

## 5. Minimaler Header

**Datei:** `src/components/RecorderPanel.tsx`

Vorher:
```tsx
<header className="panel-header">
  <div>
    <p className="eyebrow">StepCast</p>
    <h1 className="panel-title">Capture a clean how-to.</h1>
  </div>
  <div className="status-chip">...</div>
</header>
```

Nachher:
```tsx
<header className="panel-header">
  <h1 className="panel-title">StepCast</h1>
  <div className="status-chip" data-tone={STATUS_TONES[status]}>
    {STATUS_LABELS[status]}
  </div>
</header>
```

CSS:
```css
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.panel-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}
```

---

## 6. Permissions-Sektion (konditionell)

**Datei:** `src/components/RecorderPanel.tsx`

```tsx
{/* Nur zeigen wenn Permissions fehlen */}
{missingPermissions.length > 0 && (
  <section className="panel-card permissions-card">
    <div className="permission-banner warn">
      Missing: {missingPermissions.join(", ")}
    </div>
    <button className="button ghost" onClick={handleRequestPermissions}>
      Grant Permissions
    </button>
  </section>
)}
```

---

## 7. Kontextabhängige Buttons

**Datei:** `src/components/RecorderPanel.tsx`

```tsx
<div className="controls">
  {(status === "idle" || status === "stopped") && (
    <button
      className="button primary full-width"
      onClick={() => handleCommand("start", "recording")}
      disabled={!permissionsReady}
    >
      Start Recording
    </button>
  )}

  {status === "recording" && (
    <>
      <button className="button" onClick={() => handleCommand("pause", "paused")}>
        Pause
      </button>
      <button className="button danger" onClick={() => handleCommand("stop", "stopped")}>
        Stop
      </button>
    </>
  )}

  {status === "paused" && (
    <>
      <button className="button primary" onClick={() => handleCommand("resume", "recording")}>
        Resume
      </button>
      <button className="button danger" onClick={() => handleCommand("stop", "stopped")}>
        Stop
      </button>
    </>
  )}
</div>
```

CSS:
```css
.controls {
  display: flex;
  gap: 8px;
}

.controls .button {
  flex: 1;
}

.button.full-width {
  width: 100%;
}
```

---

## 8. Steps-Liste mit Mini-Thumbnails

**Datei:** `src/components/RecorderPanel.tsx`

Neue Komponente für Step-Items:

```tsx
type Step = {
  id: number;
  description: string;
  thumbnail?: string; // Base64 oder URL
};

function StepItem({ step, index }: { step: Step; index: number }) {
  return (
    <div className="step-item">
      <div className="step-thumb">
        {step.thumbnail ? (
          <img src={step.thumbnail} alt="" />
        ) : (
          <div className="step-thumb-placeholder" />
        )}
      </div>
      <div className="step-content">
        <span className="step-number">Step {index + 1}</span>
        <span className="step-desc">{step.description}</span>
      </div>
    </div>
  );
}
```

CSS:
```css
.steps-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 240px;
  overflow-y: auto;
}

.step-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px;
  background: var(--bg-secondary);
  border-radius: 8px;
}

.step-thumb {
  width: 40px;
  height: 30px;
  border-radius: 4px;
  overflow: hidden;
  flex-shrink: 0;
  background: var(--bg-tertiary);
}

.step-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.step-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.step-number {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
}

.step-desc {
  font-size: 12px;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
```

---

## 9. Export-Bereich mit Titel-Input

**Datei:** `src/components/RecorderPanel.tsx`

```tsx
<section className="panel-card export-card">
  <h2>Export</h2>
  <input
    className="title-input"
    value={title}
    onChange={(e) => setTitle(e.target.value)}
    placeholder="Guide title..."
  />
  <div className="export-actions">
    <button className="button" onClick={handleExportHtml}>HTML</button>
    <button className="button" onClick={handleExportMarkdown}>MD</button>
    <button className="button primary" onClick={handleExportPdf}>PDF</button>
  </div>
</section>
```

---

## 10. Finale Komponenten-Struktur

```
RecorderPanel
├── Header (App-Name + Status-Chip)
├── PermissionsCard (nur wenn nötig)
├── ControlsCard
│   ├── Kontextabhängige Buttons
│   └── StepsList
│       └── StepItem (Thumb + Text)
└── ExportCard
    ├── Titel-Input
    └── Export-Buttons
```

---

## Implementierungs-Reihenfolge

1. **Positionierungs-Fix** – Bug beheben (panel.rs)
2. **Panel-Größe** – tauri.conf.json anpassen
3. **CSS-Variablen** – Farbschema + Dark Mode
4. **Panel-Container** – Border-radius + Notch
5. **Header** – Minimieren
6. **Permissions** – Konditionell ausblenden
7. **Buttons** – Kontextabhängig
8. **Steps-Liste** – Mit Thumbnails (Dummy-Daten)
9. **Export-Bereich** – Titel-Input verschieben
10. **Feinschliff** – Abstände, Transitions

---

## Offene Punkte (außerhalb dieses Designs)

- Steps tatsächlich vom Rust-Backend empfangen
- Thumbnails generieren und übertragen
- Persistenz der aufgenommenen Steps
