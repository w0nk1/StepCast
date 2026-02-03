# Task 8 Export Render Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan.

**Goal:** Add HTML/Markdown rendering using raw templates and a basic test.

**Architecture:** Small render module loads two raw templates and replaces a {{title}} token. A single test verifies HTML output contains the provided title.

**Tech Stack:** TypeScript, Vite raw imports, Bun test runner.

---

### Task 1: Add failing renderHtml test

**Files:**
- Create: `src/export/render.test.ts`

**Step 1: Write the failing test**

```typescript
import { test, expect } from "bun:test";
import { renderHtml } from "./render";

test("renderHtml includes title", () => {
  const output = renderHtml("My Guide");
  expect(output).toContain("My Guide");
});
```

**Step 2: Run test to verify it fails**

Run: `bun test src/export/render.test.ts`
Expected: FAIL because module `./render` or `renderHtml` does not exist yet.

**Step 3: Commit**

Skip: user requested no commits.

### Task 2: Implement templates + renderers

**Files:**
- Create: `src/export/templates/guide.html`
- Create: `src/export/templates/guide.md`
- Create: `src/export/render.ts`

**Step 1: Add template files**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>{{title}}</title>
  </head>
  <body>
    <h1>{{title}}</h1>
  </body>
</html>
```

```markdown
# {{title}}
```

**Step 2: Implement render module**

```typescript
import guideHtml from "./templates/guide.html?raw";
import guideMarkdown from "./templates/guide.md?raw";

const TITLE_TOKEN = "{{title}}";

export function renderHtml(title: string): string {
  return guideHtml.replaceAll(TITLE_TOKEN, title);
}

export function renderMarkdown(title: string): string {
  return guideMarkdown.replaceAll(TITLE_TOKEN, title);
}
```

**Step 3: Run test to verify it passes**

Run: `bun test src/export/render.test.ts`
Expected: PASS.

**Step 4: Commit**

Skip: user requested no commits.

### Task 3: Code quality fixes

**Files:**
- Update: `src/export/render.ts`
- Update: `src/export/render.test.ts`

**Step 1: Escape HTML title**
- Add `escapeHtml` helper to replace `&`, `<`, `>`.
- Apply `escapeHtml` to title in `renderHtml` only.
- Keep `renderMarkdown` unchanged.

**Step 2: Expand tests**
- Add test that `renderMarkdown` includes the title.
- Add test that `renderHtml` escapes `<`, `>`, `&`.

**Step 3: Run test to verify it passes**

Run: `bun test src/export/render.test.ts`
Expected: PASS.

**Step 4: Commit**

Skip: user requested no commits.
