import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("codex feedback workflow review lookup", () => {
  const workflowPath = resolve(process.cwd(), ".github/workflows/codex-feedback-loop.yml");
  const workflow = readFileSync(workflowPath, "utf8");

  it("uses --slurp for paginated review lookup to avoid multiline review_id outputs", () => {
    expect(workflow).toContain('pulls/$PR/reviews" --paginate --slurp | jq -r');
    expect(workflow).not.toContain('pulls/$PR/reviews" --paginate --jq');
  });
});
