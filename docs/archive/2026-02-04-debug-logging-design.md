# Debug Logging for Tray/Panel Positioning

## Summary
Add targeted debug-only logs to the tray click, panel positioning, and click filtering paths so we can diagnose sporadic panel jumps and unexpected step captures without shipping noisy logs in release builds. All logs are guarded by `cfg!(debug_assertions)` and focus on the minimum data needed to reconstruct coordinate/scale issues.

## Architecture
We keep logging localized to the existing flow: `tray.rs` (tray click handling and panel show/hide), `panel.rs` (panel bounds), `lib.rs` (auto-show after stop/discard), and `pipeline.rs` (tray/panel click filtering). A small helper `rect_debug` formats `tauri::Rect` consistently so we can compare event rects against the `tray.rect()` API rect. Panel bounds are recorded after `show`/`position` and used to suppress clicks inside the panel; debug logs emit those bounds to confirm they match the actual window geometry.

## Data Flow
On tray click, we compute `event_rect`, fetch `api_rect` from `tray.rect()`, select the effective rect, and log all three in debug builds. After positioning the panel, we log the resulting panel bounds. When recording stops or is discarded, we re-position using `tray.rect()` and log the panel bounds again. In the capture pipeline, tray clicks are filtered using a short time window and a tray-rect containment check; panel clicks are filtered when the panel is visible and the click is inside its bounds. Both filters emit debug logs with click coords and the rect used for the decision.

## Error Handling
All logging is best-effort: failures to fetch `tray.rect()` or panel bounds are already handled in the main flow, and we only emit logs when those values are available. Logging uses `eprintln!` and is gated behind debug assertions.

## Testing
We add unit tests for `rect_debug` to lock the formatting and for the tray/panel click filtering logic (already present). Tests run in debug builds and validate behavior without requiring any OS APIs.
