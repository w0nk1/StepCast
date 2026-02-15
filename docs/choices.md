2026-02-02
- Default: initial open-source distribution unsigned (Gatekeeper bypass) until Apple Developer account available; reason: no dev account now, keep momentum.
- Default: stack = Tauri 2 + Vite + React + TypeScript + bun; reason: common ecosystem, fast iteration, easy to hire/help.
- Default: MVP screenshot capture via macOS `screencapture` CLI; reason: minimal native code, fastest to ship.
- Default: worktree branch name `feat/stepcast-mvp`; reason: conventional branch naming, matches plan scope.
- Default: scaffold in repo root using create-tauri-app project name from current directory; reason: plan requires root, CLI derives names from target dir.
- Default: license = MIT; reason: permissive OSS license, easy adoption.
- Default: record Task 2 spec by extending existing spec file, not adding a new doc; reason: user request to avoid extra docs while honoring spec requirement.

2026-02-03
- Default: allow negative x/y in capture region validation; reason: multi-display coordinates can be negative.
- Default: storage write_steps returns Result<(), String> with serde_json::to_string_pretty; reason: simple error surface + readable steps.json.
- Default: use tauri-plugin-macos-permissions version "2"; reason: aligns with Tauri 2 plugin major versions.
- Default: request_permissions issues request_* then check_* to return booleans; reason: request_* returns unit in plugin API.
- Default: request_permissions opens macOS Privacy panes when permissions missing; reason: macOS does not always show prompts.
- Default: PermissionStatus implements Default (false/false) for minimal unit test; reason: keep test tiny and deterministic.
- Default: use tauri-nspanel git dependency on branch "v2.1"; reason: upstream docs recommend v2.1 for Tauri 2.
- Default: panel label "main" and size 350x720; reason: keep export visible without scrolling.
- Default: panel style mask uses nonactivating_panel; reason: menu bar panel should not steal focus.
- Default: align panel settings with OpenUsage (MainMenu+1 level, can_join_all_spaces + stationary + full_screen_auxiliary, nonactivating style mask); reason: proven menu bar panel behavior.
- Default: show panel before positioning on tray click; reason: macOS quirk requires window shown to move between monitors.
- Default: tray icon uses bundled icons/icon.png with template rendering and left-click toggles panel; reason: matches macOS menu bar conventions.
- Default: tray icon disables menu-on-left-click to ensure click events fire; reason: click should toggle panel instead of a menu.
- Default: tray toggle reacts on MouseButtonState::Up; reason: avoid double toggles from Down+Up events.
- Default: panel movable by window background; reason: allow dragging even without a title bar.
- Default: tray click uses show_and_make_key before positioning; reason: ensures visibility and avoids macOS positioning quirks.
- Default: panel can become key and uses show_and_make_key on tray click; reason: avoid immediate hide when app is inactive.
- Default: start_recording returns "missing screen recording or accessibility permission" when permissions false; reason: concise error without new error types.
- Default: tray icon uses `icons/icon.png` resource; reason: existing asset in repo, no new files.
- Default: panel/tray uses window label `main`; reason: default Tauri window label.
- Default: tray icon resolve order = Resource icons/icon.png -> Resource icon.png -> App icons/icon.png -> App icon.png; reason: cover dev/prod bundle layouts with explicit NotFound error.
- Default: export templates minimal HTML/Markdown with only title placeholder; reason: task scope requires title only and avoids extra styling.
- Default: exclude .vscode recommendations from commit unless requested; reason: editor-specific and not required for build.

2026-02-08 (startup-ux)
- Default: NO auto-show panel on any launch (first or subsequent); reason: plan explicitly requires non-window hint only.
- Default: persist startup state as JSON in config_dir/com.w0nk1.stepcast; reason: reuse existing dirs + serde_json deps, no new dependency.
- Default: global shortcut Cmd+Shift+S to toggle panel; reason: fallback for menu-bar-hider apps; common macOS convention.
- Default: first-run hint = temporary Dock icon via ActivationPolicy::Regular; reason: visible, no extra permission needed, switches to Accessory on "Got it".
- Default: tray menu: "Open StepCast", "Quick Start", separator, "Quit StepCast"; reason: discoverable access to panel and tutorial.

2026-02-08 (release-notes)
- Default: combination approach — update prompt shows release notes body + post-update "What's New" banner; reason: covers both pre-update and post-update discovery.
- Default: store last_seen_version in startup_state.json; reason: reuses existing persistence, detects version change on launch.

2026-02-08 (export-webp)
- Default: convert export screenshots to WebP with PNG fallback; reason: WebP ~30-60% smaller, image crate 0.25 supports it natively.
- Default: only use WebP if smaller than PNG; reason: for tiny images WebP overhead can exceed PNG, so keep whichever is smaller.
- Default: capture storage stays PNG; conversion only at export time; reason: PNG is lossless source of truth, WebP conversion is a one-way optimization.

2026-02-08 (pdf-optimization)
- Default: PDFKit post-processing with best-effort fallback to original bytes; reason: if PDFDocument init or dataRepresentationWithOptions fails, export still succeeds with unoptimized PDF.
- Default: skip createLinearizedPDFOption; reason: not exposed in objc2-pdf-kit bindings.
- Default: optimize_pdf_bytes is a pure helper (bytes in, bytes out); reason: easy to unit test, no file I/O coupling.

2026-02-08 (smoothness review)
- Default: package manager = npm; reason: CI already uses npm ci, wider contributor compat, one lockfile.
- Default: pin git deps (tauri-nspanel da9c9a8, tauri-plugin-aptabase e896cce) to commit SHAs; reason: reproducible builds, update quarterly.
- Default: skip ESLint/Prettier; reason: TS strict mode suffices, small team, zero code quality issues found. Revisit when team >2.
- Default: skip structured logging (log/tracing crates); reason: custom debug_log() + conditional eprintln! is sufficient for current scale.
- Default: add cargo-audit + npm audit to CI; reason: catch known vulnerabilities early.
- Default: keep stale worktrees (codex/fast-capture, codex/permission-gate-settings); reason: branches have unmerged work.

2026-02-09 (apple-intel-feasibility)
- Default: treat Apple Intelligence step auto-description as optional feature gated by model availability (device/region/language/setting); reason: framework only works on Apple Intelligence-capable devices, must not block recording/export.
- Default: prefer native Swift wrapper (Tauri plugin/XPC/embedded CLI) over new Rust bindings for first integration; reason: Apple API is Swift-first; third-party Rust bindings are very new/low-adoption and add linkage/rpath risk.
- Default: avoid “CLI-only” bridges if possible; prefer in-process or GUI-host integration; reason: Apple DTS notes command-line tools can be more strictly rate limited than GUI apps.
- Default: do not ship Apple Intelligence trademark logos in-app unless Apple provides explicit UI glyph/badge licensing; use generic sparkle icon instead; reason: avoid trademark risk / implied endorsement.
- Default: use WebKit’s built-in OS-native HTML switch (`<input type="checkbox" switch>`) for the toggle UI; reason: matches macOS System Settings look and degrades gracefully to checkbox.
- Default: increase panel height to `640` to avoid Settings view scrolling; reason: Settings now includes Apple Intelligence section + longer copy.
- Default: v1 uses text-only inputs (Step fields + captured AX label/role metadata) and does NOT depend on screenshot understanding; reason: avoids OCR/Vision complexity, aligns with device-scale model strengths (summarize/extract/classify).
- Default: generate AI descriptions post-recording / at export time (not in the click pipeline); reason: avoid adding latency/jitter to recording; simpler error handling; user can export without AI if unavailable.
- Decision: support envelope for Apple Intelligence descriptions = macOS 26+ + Apple Silicon + Apple Intelligence enabled + supported region/language; otherwise deterministic fallback descriptions; reason: Foundation Models availability/hardware gating.
- Default: no new macOS TCC permissions needed for basic step description generation; reason: Foundation Models uses system on-device model; only tool-calling into Contacts/Calendar/etc would trigger those standard permissions.
- Default: do not request/depend on Foundation Models adapter entitlement; reason: adapters are out-of-scope for v1; entitlement needed only for deploying custom adapters.
- Default: StepCast toggle = global setting (default OFF) with live availability status + explanation; reason: consistent exports, avoids surprises, and feature frequently unavailable on older Macs/regions.
- Default: tutorial includes a short "Optional: Apple Intelligence descriptions" section (requirements, how to enable in macOS + StepCast, privacy note); reason: reduces support burden and sets correct expectations.
- Default: open Apple Intelligence settings via `x-apple.systempreferences:com.apple.Siri-Settings.extension` with fallback `x-apple.systempreferences:com.apple.preference.siri`; reason: best-effort deep link to the correct System Settings pane.
- Default: persist toggle in `localStorage.appleIntelligenceDescriptions`; reason: simple, cross-version, no backend migration needed.
- Decision: implement Foundation Models integration via embedded Swift sidecar helper (build.rs `swiftc` -> include_bytes -> extract to cache -> JSON stdio); reason: FoundationModels is Swift-first; lowest-risk path for v1 without Rust bindings.
- Change: toggle UI uses a custom Apple-style switch (button + CSS), not WebKit `<input switch>`; reason: consistent look/feel in Tauri WKWebView + matches requested UX.
- Decision: keep tray panel default height at 640 but auto-resize to 720 when Settings is open; reason: avoid scrolling in Settings without adding permanent empty space in the recorder UI.
- Default: AI description constraints = 1 sentence, max 110 chars, greedy sampling; reason: still short but allows location grounding ("from the Dock", "in Finder").
- Default: AI descriptions use a deterministic baseline + quality gate (verb/kind/app/label checks) to prevent vague outputs and "tab" hallucinations; reason: user trust > model creativity, stable exports.
- Default: store additional AX grounding metadata (subrole, role_description, identifier, container_role/subrole) on each step; reason: clicked element is often a child (static text) inside a list row; container hints improve specificity.
- Default: when AX label missing (or ActionType::RightClick), run on-device Vision OCR on the step screenshot (ROI near click) and use the best near-click text as grounding label; reason: reliably captures sidebar items and filenames (e.g. "Downloads", "coreauth.png") without cloud/LLM vision.
- Default: never generate AI descriptions for auth placeholder steps or ActionType::Note; reason: avoid sensitive/system dialogs; note steps already user-authored.
- Default: missing-only/all generation never overwrites manual descriptions; per-step sparkle can overwrite; reason: protect user edits by default.
- Change: keep AX grounding metadata even when the clicked element has no label; attempt to recover label by scanning the AX child tree (depth-limited); reason: Finder/sidebar/file-list rows often store the visible text on child static text elements.
- Change: make OCR ROI selection prefer semantic AX sidebar hints; geometry fallback only when AX metadata missing; reason: avoid "left side = sidebar" hacks when AX already provides a clean signal.
- Change: Vision OCR uses `.accurate` + smaller min text height (0.010); reason: improve label extraction for small UI text in screenshots.
- Change: Right-click OCR grounding uses filename-like strings only (ignore context menu OCR); reason: prevent wrong targets like "Right-click Open".
- Change: propagate Apple Intelligence toggle across Tauri webviews via `ai-toggle-changed` event (plus storage fallback); reason: storage events are unreliable in some Tauri window setups.
- Change: Settings panel auto-resizes to content using `documentElement.scrollHeight` + `ResizeObserver`; reason: no scrolling for the last "eighth" when Settings content grows (eligibility copy, etc.).
- Change: AX label extraction prefers `AXTitleUIElement` + `AXPlaceholderValue` and avoids `AXValue` for text fields; reason: semantically correct labels without leaking user-entered text.
- Change: text-field steps baseline uses `Click ... field` (not `Enter ...`); reason: StepCast does not capture global typing events (would require Input Monitoring permission), so claiming typing is inaccurate.

2026-02-10 (ax-pressable-scoring)
- Change: AX element selection prefers pressable elements (`AXPress`/`AXConfirm`) with a strong minimum score (>=220) and tries to pull the visible label from child `AXStaticText` when pressable; reason: many cross-platform apps expose custom buttons as pressable `AXGroup`s; prevents vague “App in App” steps and improves close/button grounding without OCR or per-app rules.
- Change: store clicked element bounds (percent within captured screenshot) as `step.ax.element_bounds` and use it to focus OCR ROI when needed; reason: if the click lands on a button background (not on the label text), click-distance OCR can pick the wrong nearby label. Element-bounds ROI keeps OCR “clean” and generic while staying on-device.
- Change: treat AX labels that look like GUID/hex IDs as non-human and ignore them for grounding; prefer `AXIdentifier`-derived labels over OCR; reason: some apps expose implementation IDs as AX labels (unhelpful for tutorials), and `AXIdentifier` often encodes the real control purpose (e.g. emoji button).
- Change: window controls (close/minimize/zoom) always use deterministic baseline text (no OCR, no model); reason: OCR picks random chrome text (“Ch”), and the model cannot improve accuracy over the AX subrole signal.
- Change: titleless overlay windows default to window_title `Popup` (not `Menu`) unless they are near the menu bar; reason: avoids misclassifying in-app popovers/pickers as menus and improves step kind classification downstream.

2026-02-10 (ai-helper-packaging)
- Change: ship the Swift FoundationModels helper as a bundled resource (`src-tauri/bin/stepcast_ai_helper`) and execute it from the app bundle; reason: avoids running a cache-extracted unsigned binary which can break Gatekeeper/notarization expectations in release builds.

2026-02-10 (list-selection-grounding)
- Default: for clicks inside list/table/outline containers, prefer the *selected row/item* label (via AXSelectedRows/AXSelectedChildren/AXSelectedItems) when the hit-test label looks structural/metadata-heavy; reason: AppKit lists often return container/view-mode labels at the click point ("List view"), and many apps embed timestamps/status metadata in AX labels. Selected-item label is usually the actual user target (file/chat).
- Change: classify list interactions using `AX container identifier` / `AX identifier` hints (table/list/outline/collection) even when the clicked element role is `AXButton`/`AXGroup`; reason: many apps implement list rows as pressable buttons, and the verb "Select …" is clearer than "Click … button" for tutorials.
- Change: treat common structural labels like "editor area"/"scroll area"/"split view" as generic (not grounding labels); reason: avoids "sad" descriptions and keeps output novice-friendly when the UI target isn't a named control.

2026-02-11 (capture-overlay-stability)
- Change: classify titleless overlays by display-relative Y even when `window_id == main_window_id`; reason: prevents false non-overlay classification on multi-display/menu-overlay cases where frontmost window resolves to the overlay itself.
- Change: force menu-region capture when AX role is menu-related (`AXMenuItem`/`AXMenuBarItem`/`AXMenuButton`/`AXMenu`); reason: avoids tiny window-ID snippets for status/menu-bar interactions and restores consistent top-region capture.
- Change: for popup overlays, compute context union using the largest app window for the clicked PID when available; reason: prevents clipped popup-only screenshots (e.g. picker panels) without app-specific hacks.

2026-02-11 (capture-overlay-stability-followup)
- Change: keep titleless `same window_id` as `NotOverlay`; reason: prevents false popup classification for legitimate titleless utility windows. Menu-bar interactions are now handled via explicit AX menu-role + top-region gating.
- Change: menu-role forced region capture is limited to top 500px of the clicked display; reason: avoids misclassifying mid-screen context menus as menu-bar captures.
- Change: popup union now prefers the clicked app’s window at the click point (`get_window_for_pid_at_click`) before falling back to largest PID window; reason: safer in multi-window apps than area-only selection.
- Change: `get_window_for_pid_at_click` now supports excluding the current popup window id; reason: prevents selecting the popup itself as "context window" and restores intended union-with-base-window behavior.
- Change: popup overlay path now always uses region capture with computed union bounds; reason: deterministic behavior and fewer tiny overlay-only window-ID captures.
- Change: `get_window_for_pid_at_click` returns the largest matching window containing the click (not first/front-most); reason: avoids selecting another small overlay and improves base-window context in stacked overlay scenarios.
- Change: clamp Dock/menu/top-region capture rectangles to clicked display bounds and adapt fixed widths/heights when display is smaller than defaults; reason: avoids off-screen/negative region math and prevents accidental clipped/empty captures on small or offset displays.
- Change: generic label filtering now rejects structural list labels (`sidebar`/`Seitenleiste`/`source list`/`list`) and metadata value labels (sizes/timestamps) as grounding targets; reason: prevents wrong outputs like "Select Seitenleiste" / "Double-click 2,4 MB".
- Change: metadata-label simplification now scores comma-separated segments and prefers recipient/entity segments (e.g. `Gesendet an ...`) over preview/status tokens; reason: improves list/chat target naming without app-specific rules.
- Change: right-click grounding now drops generic labels when OCR cannot recover a filename-like target; reason: safer generic baseline is better than wrong specific text like "Right-click List".
- Change: auth placeholder steps now carry deterministic description `Authenticate with Touch ID or enter your password to continue.`; reason: clearer guidance for end users and consistent export/editor fallback.
- Change: ignore implementation-style identifiers like `NS:8` / `AX123` as grounding labels; reason: prevents technical AX internals from leaking into user-facing step text.
- Change: filename detection now accepts truncated filenames with internal ellipsis (`foo...bar.jpeg`) but still rejects trailing-ellipsis menu items; reason: Finder list OCR often truncates long names.
- Change: selected-row AX extraction now validates candidates via `selected_label_is_probably_ui_target` before returning and keeps only semantic fallbacks; reason: avoids returning size/status cells when a file/chat target label is available in selected row descendants.

2026-02-11 (capture-owner-hardening)
- Change: `find_attached_dialog_window` now enforces owner matching (clicked PID/main PID/app-name normalization) and rejects system-UI/fremde app windows; reason: prevent foreign dialogs (Finder/1Password/CleanMyMac/etc.) from hijacking capture target selection.
- Change: attached-dialog candidates now require sensible size ratio vs main window (4%-95%); reason: reject tiny floating overlays/noise while still allowing real sheets/dialogs.
- Change: app-name comparisons in capture selection use normalized matching (strip non-alnum + lowercase); reason: avoid false mismatches from hidden Unicode/control chars (e.g. `‎WhatsApp`).
- Change: clicked-app reconciliation now switches capture window only when a concrete window for clicked PID exists at click coordinates (`get_window_for_pid_at_click`), no fallback to largest PID window; reason: prevent capturing unrelated windows that were never clicked.
- Change: do not ignore `AXMenuBarItem` clicks anymore (still ignore `AXMenuButton` menu-open noise); reason: menu-bar opener clicks are meaningful user actions and should be recorded/described.
- Change: accept foreign topmost windows as dialog candidates when they are non-system, smaller than main window, and strongly overlap/are contained in main window; reason: system-hosted sheets/open-save panels can belong to helper processes and otherwise were lost.
- Change: for `is_sheet_dialog` captures, force region union of parent window + dialog; reason: better user comprehension than dialog-only crops.
- Change: sheet fast-path now unions AX dialog region with frontmost parent window (overlap-gated) and clamps to display bounds; reason: preserve dialog context while keeping race-condition robustness.
- Change: classify `AXMenu` role as `menu item` in Swift description heuristics; reason: actions like attachment menus should use "Choose ... from the menu" instead of generic click text.
- Change: for volatile AX roles (`AXMenuItem`/`AXMenu`/`AXGroup`), prefer region capture over window-id capture in the normal window path; reason: transient overlays/pickers can disappear before slower window-id capture completes.
- Change: keep menu-bar region behavior explicit (`should_use_menu_region_capture`) and add separate transient-capture policy (`should_prefer_transient_region_capture`); reason: deterministic, app-agnostic capture selection with clearer testable rules.
- Change: OCR filename preference is now right-click-only (not all double-click list actions); reason: folders/list items without file extensions (e.g. `var 9`) are valid targets and were previously mis-grounded.
- Change: relative-day labels (`Today/Heute/Yesterday/Gestern`) and timestamp tokens are treated as non-target metadata in AI grounding; reason: avoid wrong labels like chat date separators.
- Change: when no trustworthy grounding label exists, force deterministic baseline output (no free model rewrite); reason: prevent hallucinated targets under low-confidence conditions.
- Change: filter `AXMenuButton` menu-open noise before `next_step_id()` allocation in pipeline; reason: prevents step-id reuse and screenshot overwrite when the following click is the actual menu item.
- Change: keep a short-lived `last_menu_bar_click_ms` signal and treat near-top follow-up clicks as menu interactions; reason: AX hit-testing on transient menu rows can fall back to underlying app roles (`AXGroup`), causing wrong capture branch selection.
- Change: restrict transient region capture for `AXGroup` to popup overlays only; reason: generic in-window `AXGroup` clicks should not force volatile capture behavior and can pollute screenshot/context alignment.
- Change: skip traffic-light geometry fallback for menu roles (`AXMenuBarItem`/`AXMenuItem`/`AXMenu`/`AXMenuButton`); reason: prevents false close/minimize/zoom inference on menu-bar interactions.
- Change: when menu-region capture is chosen, force step `window_title` to `Menu`; reason: downstream description classification should consistently treat those steps as menu actions.
- Change: ignore `ax.element_bounds` ROI for menu-like/group hits with weak day-like labels (`Today/Heute/...`) and fall back to click-centered OCR ROI; reason: prevents OCR grounding from drifting into unrelated background text.

2026-02-12 (ai-description-hardening)
- Change: checkbox verbs now use toggle semantics from current AX state (`checked=true` => `Disable`, `checked=false` => `Enable`); reason: previously steps could be described inverted.
- Change: menu/menu-bar/checkbox/window-control kinds now use deterministic baseline text (no model call), and model errors fall back to baseline; reason: removes red AI failures for otherwise well-grounded steps.
- Change: OCR list grounding now penalizes file-kind/status/date metadata labels and prefers row-aligned primary labels; reason: improves filename/folder target selection in Finder-like lists.
- Change: AX list clicks now attempt selected-row label replacement more aggressively (including one short retry for delayed selection updates); reason: reduce OCR dependence and improve first-click row accuracy.

2026-02-12 (ai-description-hardening-followup)
- Default: hide generic window titles like `Ohne Titel`/`Untitled` from description context; reason: reduces noise and avoids confusing suffixes.
- Default: for list-item grounding, reject weak labels that look like file-kind/status/date/sidebar location when click is in content area; reason: better generic fallback than wrong specific target (`Filme`, `hreibtisch`).
- Default: for popup/group clicks, treat picker category labels (`Emoji/GIF/Stickers/Today`) as non-target context unless role is explicit tab/menu; reason: avoids mislabeling sticker-item clicks as category clicks.
- Default: if list OCR confidence is low or semantically weak, prefer deterministic generic phrasing (`Select the item...`) over speculative names; reason: stability > false precision.

2026-02-12 (preclick-screencapturekit-buffer)
- Default: use ScreenCaptureKit pre-click ring buffer only for volatile click paths (`prefer_transient_region_capture`); reason: highest benefit where menus/popovers close fastest, minimal pipeline risk elsewhere.
- Default: keep existing CG/window/region capture as hard fallback when pre-click frame is unavailable or fails; reason: no regression in current capture baseline.
- Default: ring buffer depth `4` at `12` FPS with BGRA stream; reason: enough temporal history for menu clicks while keeping CPU/memory bounded.
- Default: maintain a single active ScreenCaptureKit display stream and switch on display change (first click after switch falls back); reason: avoid multi-stream handler ambiguity in current crate integration.
- Default: add linker rpath `/usr/lib/swift` in `src-tauri/build.rs`; reason: ensure test/dev binaries can resolve Swift runtime dylibs required by ScreenCaptureKit bridge.

2026-02-12 (pixel-first-capture-default)
- Default: prefer `preclick_fullframe_capture` for all non-right-click, non-auth interactions; reason: pixel-level full-display frame is the most robust cross-app/web source of truth for transient overlays/menus.
- Default: keep right-click on existing context-menu path; reason: right-click menus appear after click and would be missed by strict pre-click capture.
- Default: keep auth dialogs on dedicated secure capture/placeholder path; reason: secure UI may not be capturable via regular full-frame semantics.
- Default: reject stale pre-click frames older than `500ms`; reason: avoid mismatched screenshots when stream/frame timing drifts.
- Default: reject pre-click frames captured after the click (`age_ms < 0`) and return no pre-click frame if all ring frames are newer than click timestamp; reason: prevent time-inverted frame selection under queue lag.

2026-02-12 (ai-grounding-debug-telemetry)
- Default: keep list-item labels equal to `window_title` when AX container hints indicate real list/sidebar targets; reason: avoids false "generic label" rejection that pushed OCR drift (`Pepper` -> `vat`).
- Default: strip context-menu artifacts from OCR labels (`@ open/öffnen/...`) and broaden context-menu phrase detection; reason: prevent contamination like `"mixuino_errors @ öffnen"` becoming the target label.
- Default: remove Private Use Area glyphs from labels (e.g. ``); reason: icon-font artifacts are not user-facing text and pollute tutorial steps.
- Default: quality gate now returns explicit decision reasons (`accepted`, `quoted_label_lost`, etc.); reason: deterministic traceability for why baseline/candidate was chosen.
- Default: include per-step AI debug payload (`kind`, `groundingLabel`, `groundingOcr`, `baseline`, `candidate`, `qualityGateReason`) in generation response; reason: richer post-mortem data for fragile edge cases.

2026-02-12 (baseline-first-no-typo-list)
- Default: prefer deterministic baseline text for fragile kinds (`list item`, `item`, `button`, `text field`, `tab`) when model rewrite differs; reason: stability over stylistic rewrites.
- Default: no language-specific typo correction list in grounding pipeline; reason: keep behavior multilingual and avoid locale-bound rules.

2026-02-12 (result-hardening-round-2)
- Default: do not classify by `window_title=Menu` alone when AX role is not menu-like; reason: prevents false "Choose ... from the menu" wording on normal web/content clicks.
- Default: treat infrastructure labels like `Popover Dismiss Region` as generic UI labels; reason: avoid technical/internal identifiers in end-user instructions.
- Default: for sidebar-side list clicks (`x < 30%`), reject OCR-only labels that are not strong filename tokens; reason: wrong specific names are worse than stable generic steps.
- Default: for picker-like interactions (emoji/gif/sticker/picker signals in AX role/identifier), reject OCR/identifier labels for list items; reason: grid content OCR is volatile and produces non-actionable fragments.

2026-02-13 (step-focus-crop-system)
- Default: persist non-destructive per-step crop metadata as `crop_region` percentages (`x/y/width/height`) instead of rewriting screenshots; reason: stable across resolutions, editable, and safe for rollback.
- Default: auto-focus crop only when capture is display-sized (>= ~screen bounds tolerance); reason: avoid over-cropping already tight window/dialog captures.
- Default: auto-focus center uses AX element bounds when available, else click position; reason: better first-pass focus without app-specific rules.
- Default: crop exports by transforming image bytes during export (HTML/Markdown/PDF) while keeping source files untouched; reason: output readability + consistent behavior across formats.

2026-02-13 (step-focus-crop-followup)
- Default: apply auto-focus crop for very large captures based on display coverage (`area_ratio` / width+height ratios), not only exact fullscreen matches; reason: large sheet/dialog union captures were still too zoomed out for tutorial readability.
- Default: apply auto-focus crop in fast-path step creation (`sheet_fast_path`, `window_control_fast_path`) as well; reason: these early-return branches bypassed the normal crop decision and produced oversized screenshots.
- Default: tighten pre-click frame freshness window from `500ms` to `250ms`; reason: reduce stale-frame risk for transient overlays while keeping enough tolerance at current capture FPS.
- Default: increase pre-click buffer stream rate from `12` to `16` FPS; reason: improve probability of capturing transient UI state close to click-time with acceptable overhead.
2026-02-13 (screenshot-event-stability)
- Default: on frontend `step-updated` merges, preserve an existing `screenshot_path` when incoming payload omits it or clears it unexpectedly without `capture_status=Failed`; reason: prevents transient/partial update events from blanking already-captured thumbnails.
- Default: retry local screenshot image loads up to 2 times with short delay + cache-busting query; reason: reduce timing-related blank thumbnails when filesystem write and webview image decode race.
2026-02-13 (editor-crop-visibility-fix)
- Default: cropped editor rendering is activated whenever `crop_region` is valid (not gated on loaded natural image size); reason: avoids post-apply "image disappeared" state caused by sizing race.
- Default: cropped frame uses a deterministic fallback `aspect-ratio` (`16/9`) until natural image dimensions are known; reason: prevents zero-size layout collapse in absolute-position crop mode.
- Default: editor screenshot wrapper is block-level and width-constrained (`width: min(100%, 760px)`); reason: stable layout for both full and cropped frames.
2026-02-13 (editor-crop-drag-reposition)
- Default: existing crops can be repositioned directly in step preview via pointer drag (left mouse), commit on pointer release only; reason: fast UX without command spam during drag.
- Default: crop drag uses direct manipulation semantics (content follows cursor), implemented by translating crop origin against pointer delta scaled by crop size; reason: intuitive repositioning of visible area.
- Default: no-op click (down/up without movement) does not persist crop updates; reason: avoid unnecessary writes/events.
2026-02-13 (editor-crop-drag-runtime-hardening)
- Default: disable native browser image drag in editor screenshot surfaces (`draggable=false`, prevent default dragstart, `-webkit-user-drag: none`); reason: avoids ghost-image/plus-cursor interference during crop reposition.
- Default: crop drag preview updates are batched via `requestAnimationFrame`; reason: smoother drag with lower React render pressure under frequent pointermove events.
- Default: commit crop changes only when movement exceeds a minimal epsilon (`0.02` percent units); reason: ignore jitter/no-op clicks and reduce unnecessary state writes/events.
- Default: editor and timeline images use `loading="lazy"` + `decoding="async"`; reason: lower initial decode cost for long recordings.
2026-02-14 (pre-push-coverage-gate)
- Default: keep strict per-file thresholds (`80/80/80/80`) and fix via targeted tests instead of lowering config; reason: preserves quality gate and prevents regressions on critical editor/crop flows.
- Default: cover branch gaps through UI-level tests (RTL/Vitest) before considering instrumentation ignores; reason: executable behavior checks are more trustworthy than coverage-only annotations.

2026-02-14 (swift-foundationmodels-ci-compat)
- Default: compile AI helper with conditional `FoundationModels` import and feature guards (`#if canImport(FoundationModels)`); reason: keep CI/macOS runners without Apple Intelligence SDK buildable.
- Default: when `FoundationModels` is unavailable, return deterministic baseline output with explicit reason (`model_unavailable_fallback`) instead of throwing/aborting; reason: preserve functional captures and editor flow without hard failure.

2026-02-14 (release-versioning-default)
- Default: bump to `0.3.0` (minor) for this merge set; reason: includes substantial new user-facing features (AI-assisted steps, crop editor, capture hardening), not a patch-only change.

2026-02-14 (publish-runner-target-alignment)
- Default: pin Publish matrix to native-arch runners (`aarch64 -> macos-15`, `x86_64 -> macos-13`) instead of `macos-latest` for both; reason: prevent cross-arch linker failures in `screencapturekit` artifacts.

2026-02-15 (publish-runner-label-correction)
- Default: use `macos-15-intel` for x86_64 publish jobs; reason: `macos-13` label is unsupported on current GitHub-hosted runner pool for this repo.

2026-02-15 (release-provenance-clean-cut)
- Default: cut `v0.3.1` from latest `main` instead of publishing `v0.3.0` draft produced via workflow_dispatch commit mismatch; reason: keep tag, source, and artifacts strictly aligned.
