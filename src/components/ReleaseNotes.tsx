import { useMemo } from "react";

/** Render release-notes markdown as clean, user-friendly HTML.
 *  Supports: bullet lists (`- ` / `* `), **bold**, headings (stripped to bold),
 *  and plain paragraphs. No external dependency needed. */
export default function ReleaseNotes({ body }: { body: string }) {
  const elements = useMemo(() => parseNotes(body), [body]);

  if (elements.length === 0) return null;

  return <div className="release-notes">{elements}</div>;
}

type NoteElement =
  | { type: "paragraph"; text: string }
  | { type: "list"; items: string[] };

function parseNotes(raw: string): JSX.Element[] {
  const lines = raw.split("\n");
  const blocks: NoteElement[] = [];
  let currentList: string[] | null = null;

  for (const line of lines) {
    const trimmed = line.trim();

    // Skip empty lines (flush current list)
    if (!trimmed) {
      if (currentList) {
        blocks.push({ type: "list", items: currentList });
        currentList = null;
      }
      continue;
    }

    // Skip heading markers like "## What's Changed" — GitHub auto-generated noise
    if (/^#{1,4}\s/.test(trimmed)) {
      if (currentList) {
        blocks.push({ type: "list", items: currentList });
        currentList = null;
      }
      continue;
    }

    // Bullet: `- text` or `* text`
    const bulletMatch = trimmed.match(/^[-*]\s+(.+)/);
    if (bulletMatch) {
      const text = cleanLine(bulletMatch[1]);
      if (!currentList) currentList = [];
      currentList.push(text);
      continue;
    }

    // Plain text → paragraph
    if (currentList) {
      blocks.push({ type: "list", items: currentList });
      currentList = null;
    }
    const text = cleanLine(trimmed);
    if (text) {
      blocks.push({ type: "paragraph", text });
    }
  }

  if (currentList) {
    blocks.push({ type: "list", items: currentList });
  }

  return blocks.map((block, i) => {
    if (block.type === "list") {
      return (
        <ul key={i}>
          {block.items.map((item, j) => (
            <li key={j} dangerouslySetInnerHTML={{ __html: inlineMd(item) }} />
          ))}
        </ul>
      );
    }
    return <p key={i} dangerouslySetInnerHTML={{ __html: inlineMd(block.text) }} />;
  });
}

/** Strip PR links, contributor mentions, and other GitHub noise from a line. */
function cleanLine(line: string): string {
  return line
    // Remove PR references like "by @user in https://github.com/..."
    .replace(/\s*by\s+@[\w-]+\s+in\s+https?:\/\/\S+/gi, "")
    // Remove standalone PR links like "https://github.com/.../pull/123"
    .replace(/\s*https?:\/\/github\.com\/\S+\/pull\/\d+/g, "")
    // Remove inline PR refs like "(#123)"
    .replace(/\s*\(#\d+\)/g, "")
    .trim();
}

/** Convert **bold** to <strong> tags. */
function inlineMd(text: string): string {
  return text.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
}
