import { describe, expect, it } from "vitest";
import workflow from "../../.github/workflows/codex-feedback-loop.yml?raw";

describe("codex feedback workflow review lookup", () => {
  it("uses --slurp for paginated review lookup to avoid multiline review_id outputs", () => {
    expect(workflow).toContain('pulls/$PR/reviews" --paginate --slurp | jq -r');
    expect(workflow).not.toContain('pulls/$PR/reviews" --paginate --jq');
  });
});
