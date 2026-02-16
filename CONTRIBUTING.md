# Contributing

Thanks for your interest in contributing!

## Getting Started

1. Fork the repo and clone it
2. `npm install`
3. `npx tauri dev`
4. Create a branch: `git checkout -b feat/my-feature`

## Pull Requests

- One feature or fix per PR
- `cargo clippy -- -D warnings` and `npm run build` should pass
- `npm run i18n:check` should pass for locale changes
- Use [conventional commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `chore:`)

## Review Automation

- CI (`.github/workflows/ci.yml`) is the required merge gate for PRs
- Codex review follow-up is automated via `.github/workflows/codex-feedback-loop.yml`
- If you want to trigger codex fixes manually on a PR, comment `/codex-fix` on that PR

## Translations

- Source locale is `src/i18n/locales/en.json`
- Add/update translated keys in `src/i18n/locales/<locale>.json`
- New locale files are auto-discovered in-app (including Settings language dropdown); no code changes needed
- Keep placeholders aligned with English (`{count}`, `{name}`, plural vars)
- Validate with `npm run i18n:check`

## Issues

Found a bug? [Open an issue](https://github.com/w0nk1/StepCast/issues) with your macOS version and steps to reproduce.
