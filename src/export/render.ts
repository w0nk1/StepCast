import guideHtml from "./templates/guide.html?raw";
import guideMarkdown from "./templates/guide.md?raw";
import type { Step } from "../types/step";

const TITLE_TOKEN = "{{title}}";
const STEPS_TOKEN = "{{steps}}";
const STEP_COUNT_TOKEN = "{{stepCount}}";

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function renderStepHtml(step: Step, index: number, imageBase64: string | null): string {
  const stepNumber = index + 1;
  const imageHtml = imageBase64
    ? `<img src="data:image/png;base64,${imageBase64}" alt="Step ${stepNumber}">`
    : "";
  const clickMarker = imageBase64
    ? `<div class="click-marker" style="left: ${step.click_x_percent}%; top: ${step.click_y_percent}%;"></div>`
    : "";

  return `
    <article class="step">
      <div class="step-header">
        <span class="step-number">Step ${stepNumber}</span>
        <span class="step-app">${escapeHtml(step.app)} - "${escapeHtml(step.window_title)}"</span>
      </div>
      <div class="step-image">
        ${imageHtml}
        ${clickMarker}
      </div>
    </article>
  `;
}

function renderStepMarkdown(step: Step, index: number, imageBase64: string | null): string {
  const stepNumber = index + 1;
  const imageMarkdown = imageBase64
    ? `![Step ${stepNumber}](data:image/png;base64,${imageBase64})`
    : "";

  return `
## Step ${stepNumber}

${imageMarkdown}

**Action:** Clicked in ${step.app} - "${step.window_title}"

---
`;
}

export type StepWithImage = {
  step: Step;
  imageBase64: string | null;
};

export function renderHtml(title: string, stepsWithImages: StepWithImage[] = []): string {
  const escapedTitle = escapeHtml(title);
  const stepsHtml = stepsWithImages
    .map(({ step, imageBase64 }, index) => renderStepHtml(step, index, imageBase64))
    .join("\n");

  return guideHtml
    .replaceAll(TITLE_TOKEN, escapedTitle)
    .replaceAll(STEP_COUNT_TOKEN, String(stepsWithImages.length))
    .replaceAll(STEPS_TOKEN, stepsHtml);
}

export function renderMarkdown(title: string, stepsWithImages: StepWithImage[] = []): string {
  const escapedTitle = escapeHtml(title);
  const stepsMarkdown = stepsWithImages
    .map(({ step, imageBase64 }, index) => renderStepMarkdown(step, index, imageBase64))
    .join("\n");

  return guideMarkdown
    .replaceAll(TITLE_TOKEN, escapedTitle)
    .replaceAll(STEP_COUNT_TOKEN, String(stepsWithImages.length))
    .replaceAll(STEPS_TOKEN, stepsMarkdown);
}
