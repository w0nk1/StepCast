# Backend i18n Phase 2 (Export + Tray)

## Scope
- Localize Rust export text output (HTML/Markdown/PDF text templates and fallback action descriptions).
- Localize tray menu labels/tooltips in Rust.
- Use frontend-selected `appLanguage` for exports (`system|en|de`).
- Use system locale for tray labels/tooltips.

## Non-Goals
- No Swift Apple Intelligence prompt localization yet (Phase 3).
- No runtime tray relabeling on language change (applies at app start).

## Design
- Add `src-tauri/src/i18n.rs` with `Locale` and locale-resolution helpers.
- Add pure translation helpers for export/tray text pieces.
- Extend export pipeline to receive locale and render localized strings.
- Extend `export_guide` command to accept optional `app_language` from frontend.
- Keep English defaults for compatibility and fallback.

## Verification
- Rust unit tests for locale resolution + export/tray strings.
- Existing export tests remain green (English default).
- Frontend tests + build remain green.
