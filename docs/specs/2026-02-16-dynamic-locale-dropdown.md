# Dynamic Locale Selector (Contributor-Scale)

## Scope
- Replace hardcoded language pills with a scalable dropdown.
- Ensure language sync accepts any supported locale, not only `en`/`de`.
- Keep contributor workflow simple: add locale JSON, run checks, open PR.

## Design
- Use i18n runtime `availableLocales` + `getLanguageLabel` in `SettingsSheet`.
- Render language selector as `<select>` with options: `system` + discovered locales.
- Validate event payloads with `isSupportedAppLanguage` in `EditorWindow` listener.
- Keep English fallback behavior unchanged in i18n core.

## Verification
- `npm test`
- `npm run i18n:check`
- `npm run build`
