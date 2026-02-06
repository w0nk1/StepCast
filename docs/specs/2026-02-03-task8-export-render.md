Task 8: HTML/Markdown exporter

Scope
- Add renderHtml/renderMarkdown in src/export/render.ts using templates loaded via ?raw and replace {{title}}.
- Add templates: src/export/templates/guide.html and src/export/templates/guide.md.
- Add test: src/export/render.test.ts ensures renderHtml output includes title.

Non-goals
- No additional placeholders or styling beyond minimal template structure.
- No integration with UI, storage, or export pipeline.
