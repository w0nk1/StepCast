import guideHtml from "./templates/guide.html?raw";
import guideMarkdown from "./templates/guide.md?raw";

const TITLE_TOKEN = "{{title}}";

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

export function renderHtml(title: string): string {
  const escapedTitle = escapeHtml(title);
  return guideHtml.replaceAll(TITLE_TOKEN, escapedTitle);
}

export function renderMarkdown(title: string): string {
  const escapedTitle = escapeHtml(title);
  return guideMarkdown.replaceAll(TITLE_TOKEN, escapedTitle);
}
