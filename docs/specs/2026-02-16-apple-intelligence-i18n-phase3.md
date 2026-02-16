# Phase 3: Apple Intelligence i18n (Swift)

## Scope
- Localize Apple Intelligence generated step descriptions for `en`/`de`.
- Pipe frontend app language into `generate_step_descriptions` backend command.
- Keep deterministic baseline behavior and quality-gate safety.

## Design
- Extend Rust command `generate_step_descriptions` with optional `app_language`.
- Forward locale into `apple_intelligence::generate_descriptions` and helper JSON request.
- Extend Swift `GenerateRequest` with `appLanguage`.
- Add lightweight locale helpers in Swift (`en|de`) and localize:
  - action verbs
  - baseline descriptions
  - prompt rules/examples
  - quality-gate verb/context checks
- Keep fallback behavior deterministic; unknown locale -> English.

## Verification
- `swiftc -typecheck` helper files
- `cd src-tauri && cargo test --quiet`
- `npm test`
- `npm run build`
