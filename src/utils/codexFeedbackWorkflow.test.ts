import { describe, expect, it } from "vitest";
import workflow from "../../.github/workflows/codex-feedback-loop.yml?raw";

describe("codex feedback workflow review lookup", () => {
  it("uses --slurp for paginated review lookup to avoid multiline review_id outputs", () => {
    expect(workflow).toContain('pulls/$PR/reviews" --paginate --slurp | jq -r');
    expect(workflow).not.toContain('pulls/$PR/reviews" --paginate --jq');
  });

  it("keeps closed-pr no-op log message yaml-safe (no inline # truncation)", () => {
    expect(workflow).toContain('run: echo "PR ${{ steps.ctx.outputs.pr_number }} is not open; skipping."');
    expect(workflow).not.toContain('run: echo "PR #${{ steps.ctx.outputs.pr_number }} is not open; skipping."');
  });
});
