# Recorder Own-App Click Filter Hardening

## Scope
- Prevent recorder steps from being created when the click targets StepCast itself.
- Cover menu bar edge cases where AX click metadata can resolve to system hosts.

## Design
- Add normalized own-app matcher (`is_own_app_name`) for StepCast name variants.
- Keep early PID/app-name filter for normal own-app clicks.
- Add a second guard after capture target resolution:
  - filter when resolved app or capture window app is StepCast,
  - even if AX `clicked_info` reported a different process.
- Return `PipelineError::OwnAppClick` consistently for own-app drops.
- Add unit tests for own-app name matching.

## Verification
- `cd src-tauri && cargo test --quiet own_app_name_matches_stepcast_variants`
