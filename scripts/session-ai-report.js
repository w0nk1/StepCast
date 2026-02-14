import fs from "node:fs";
import os from "node:os";
import path from "node:path";

function parseArgs(argv) {
  const args = { session: null, dir: null, trace: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--session") args.session = argv[++i] ?? null;
    else if (a === "--dir") args.dir = argv[++i] ?? null;
    else if (a === "--trace") args.trace = argv[++i] ?? null;
    else if (a === "-h" || a === "--help") args.help = true;
  }
  return args;
}

function sessionsRoot() {
  return path.join(os.homedir(), "Library", "Caches", "com.w0nk1.stepcast", "sessions");
}

function listDirsSortedByMtime(root) {
  if (!fs.existsSync(root)) return [];
  const entries = fs.readdirSync(root, { withFileTypes: true });
  const dirs = entries
    .filter((e) => e.isDirectory())
    .map((e) => path.join(root, e.name))
    .map((p) => ({ p, mtimeMs: fs.statSync(p).mtimeMs }))
    .sort((a, b) => b.mtimeMs - a.mtimeMs)
    .map((x) => x.p);
  return dirs;
}

function pickSessionDir(args) {
  if (args.dir) return args.dir;
  const root = sessionsRoot();
  if (args.session) return path.join(root, args.session);
  const dirs = listDirsSortedByMtime(root);
  return dirs[0] ?? null;
}

function tryReadJson(filePath) {
  try {
    const s = fs.readFileSync(filePath, "utf8");
    return JSON.parse(s);
  } catch {
    return null;
  }
}

function listAiTraceFiles(sessionDir) {
  const files = fs.readdirSync(sessionDir);
  return files
    .filter((f) => /^ai-trace-\d+-(request|response)\.json$/.test(f))
    .map((f) => path.join(sessionDir, f))
    .map((p) => ({ p, mtimeMs: fs.statSync(p).mtimeMs }))
    .sort((a, b) => b.mtimeMs - a.mtimeMs)
    .map((x) => x.p);
}

function pickTraceTs(sessionDir, args) {
  if (args.trace) return String(args.trace);
  const files = listAiTraceFiles(sessionDir);
  const first = files[0];
  if (!first) return null;
  const m = path.basename(first).match(/^ai-trace-(\d+)-/);
  return m?.[1] ?? null;
}

function readAiTrace(sessionDir, traceTs) {
  if (!traceTs) return { request: null, response: null };
  const req = tryReadJson(path.join(sessionDir, `ai-trace-${traceTs}-request.json`));
  const res = tryReadJson(path.join(sessionDir, `ai-trace-${traceTs}-response.json`));
  return { request: req, response: res };
}

function parseRecordingLog(logPath) {
  if (!fs.existsSync(logPath)) return { byStepId: new Map(), aiDesc: new Map(), lines: [] };
  const lines = fs.readFileSync(logPath, "utf8").split("\n").filter(Boolean);
  const byStepId = new Map();
  const aiDesc = new Map();

  // Heuristic: events are sequential. We attach the nearest preceding `ax_click` + `click`
  // to the next `screenshot_path=.../step-XYZ.png` we see.
  let lastClick = null;
  let lastAx = null;

  for (const line of lines) {
    if (line.includes(" click: ")) lastClick = line;
    if (line.includes(" ax_click: ")) lastAx = line;

    const sp = line.match(/screenshot_path=([^ ]+\/(step-\d+)\.png)/);
    if (sp) {
      const stepId = sp[2];
      const entry = byStepId.get(stepId) ?? { stepId, screenshotPath: sp[1], click: null, ax: null, extra: [] };
      entry.screenshotPath = sp[1];
      entry.click = lastClick;
      entry.ax = lastAx;
      entry.extra.push(line);
      byStepId.set(stepId, entry);
      continue;
    }

    const ai = line.match(/ai_desc\s+trace=\d+\s+id=([^\s]+)\s+text=(.*)$/);
    if (ai) {
      aiDesc.set(ai[1], ai[2]);
      continue;
    }
    const fail = line.match(/ai_desc_failed\s+trace=\d+\s+id=([^\s]+)\s+error=(.*)$/);
    if (fail) {
      aiDesc.set(fail[1], `<<FAILED>> ${fail[2]}`);
      continue;
    }
  }

  return { byStepId, aiDesc, lines };
}

function extractAxSummary(axLine) {
  if (!axLine) return null;
  const role = axLine.match(/role=([^ ]+)/)?.[1] ?? "";
  const label = axLine.match(/label='([^']*)'/)?.[1] ?? "";
  const parentRole = axLine.match(/parent_role=Some\\(\"([^\"]+)\"\\)/)?.[1] ?? null;
  const topRole = axLine.match(/top_role=Some\\(\"([^\"]+)\"\\)/)?.[1] ?? null;
  return { role, label, parentRole, topRole };
}

function renderReport({ sessionDir, traceTs, aiTrace, log }) {
  const steps = [];

  // Prefer request.steps order when available (this is the exact set sent to the model).
  if (aiTrace?.request?.steps && Array.isArray(aiTrace.request.steps)) {
    for (const s of aiTrace.request.steps) {
      const stepId = s?.id;
      if (!stepId) continue;
      steps.push(stepId);
    }
  } else {
    // Fallback: use screenshot files.
    const files = fs.existsSync(sessionDir) ? fs.readdirSync(sessionDir) : [];
    for (const f of files.filter((x) => /^step-\d+\.png$/.test(x)).sort()) {
      steps.push(f.replace(/\.png$/, ""));
    }
  }

  const lines = [];
  lines.push(`# Session AI Report`);
  lines.push("");
  lines.push(`- session_dir: ${sessionDir}`);
  lines.push(`- trace: ${traceTs ?? "(none found)"}`);
  lines.push("");

  for (const id of steps) {
    const entry = log.byStepId.get(id) ?? { stepId: id, screenshotPath: null, click: null, ax: null, extra: [] };
    if (!entry.screenshotPath) {
      const fallback = path.join(sessionDir, `${id}.png`);
      if (fs.existsSync(fallback)) entry.screenshotPath = fallback;
    }
    const traceResult = aiTrace?.response?.results?.find?.((r) => r?.id === id) ?? null;
    const aiText =
      log.aiDesc.get(id) ??
      traceResult?.text ??
      aiTrace?.response?.failures?.find?.((f) => f?.id === id)?.error ??
      "(no ai output found)";

    const ax = extractAxSummary(entry.ax);

    lines.push(`## ${id}`);
    lines.push("");
    lines.push(`- screenshot: ${entry.screenshotPath ?? "(missing)"}${entry.screenshotPath && fs.existsSync(entry.screenshotPath) ? "" : " (not found)"}`);
    lines.push(`- click_log: ${entry.click ?? "(missing)"}`);
    lines.push(`- ax_log: ${entry.ax ?? "(missing)"}`);
    if (ax) {
      lines.push(`- ax_summary: role=${ax.role} label=${JSON.stringify(ax.label)} parent=${ax.parentRole ?? ""} top=${ax.topRole ?? ""}`);
    }
    lines.push(`- ai_text: ${aiText}`);
    if (traceResult?.debug) {
      const dbg = traceResult.debug;
      lines.push(`- ai_debug: kind=${dbg.kind ?? ""} gate=${dbg.qualityGateReason ?? ""} label=${JSON.stringify(dbg.groundingLabel ?? "")} ocr=${JSON.stringify(dbg.groundingOcr ?? "")}`);
      lines.push(`- ai_baseline: ${dbg.baseline ?? ""}`);
      if (dbg.candidate != null) lines.push(`- ai_candidate: ${dbg.candidate}`);
    }
    lines.push("");
  }

  // Also include any screenshots that exist but were NOT sent to the model
  // (e.g. auth placeholder steps), so audits see the full timeline.
  try {
    const files = fs.readdirSync(sessionDir).filter((x) => /^step-\d+\.png$/.test(x)).sort();
    const idsInRequest = new Set(steps);
    for (const f of files) {
      const id = f.replace(/\.png$/, "");
      if (idsInRequest.has(id)) continue;
      lines.push(`## ${id}`);
      lines.push("");
      lines.push(`- screenshot: ${path.join(sessionDir, f)}`);
      lines.push(`- click_log: (not sent to model)`);
      lines.push(`- ax_log: (not sent to model)`);
      lines.push(`- ai_text: (not generated; excluded from AI input)`);
      lines.push("");
    }
  } catch {
    // ignore
  }

  return lines.join("\n");
}

function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage:",
        "  node scripts/session-ai-report.js [--dir <session_dir>] [--session <uuid>] [--trace <ts>]",
        "",
        "Defaults:",
        "  picks newest session under ~/Library/Caches/com.w0nk1.stepcast/sessions",
        "  picks newest ai-trace-* within that session",
        "",
      ].join("\n"),
    );
    process.exit(0);
  }

  const sessionDir = pickSessionDir(args);
  if (!sessionDir || !fs.existsSync(sessionDir)) {
    process.stderr.write(`No session dir found.\n`);
    process.exit(2);
  }

  const traceTs = pickTraceTs(sessionDir, args);
  const aiTrace = readAiTrace(sessionDir, traceTs);
  const log = parseRecordingLog(path.join(sessionDir, "recording.log"));

  process.stdout.write(renderReport({ sessionDir, traceTs, aiTrace, log }));
  process.stdout.write("\n");
}

main();
