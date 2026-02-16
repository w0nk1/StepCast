#!/usr/bin/env node

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const localesDir = path.resolve(__dirname, "../src/i18n/locales");
const sourceFile = path.join(localesDir, "en.json");

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, "utf8"));
}

function extractPlaceholders(input) {
  const text = String(input ?? "");
  const names = new Set();
  const pluralPattern =
    /\{(\w+),\s*plural,\s*one\s*\{[^{}]*\}\s*other\s*\{[^{}]*\}\s*\}/g;

  for (const match of text.matchAll(pluralPattern)) {
    names.add(match[1]);
  }
  const withoutPlurals = text.replace(pluralPattern, "");
  for (const match of withoutPlurals.matchAll(/\{(\w+)\}/g)) {
    names.add(match[1]);
  }

  return [...names].sort();
}

const source = readJson(sourceFile);
const sourceKeys = Object.keys(source).sort();
const localeFiles = readdirSync(localesDir)
  .filter((name) => name.endsWith(".json") && name !== "en.json")
  .sort();

let hasIssues = false;

for (const localeFile of localeFiles) {
  const localePath = path.join(localesDir, localeFile);
  const localeCode = localeFile.replace(/\.json$/, "");
  const locale = readJson(localePath);
  const localeKeys = Object.keys(locale).sort();

  const missing = sourceKeys.filter((k) => !(k in locale));
  const extra = localeKeys.filter((k) => !(k in source));

  if (missing.length > 0) {
    hasIssues = true;
    console.error(`[i18n] ${localeCode}: missing keys (${missing.length})`);
    for (const key of missing) console.error(`  - ${key}`);
  }
  if (extra.length > 0) {
    hasIssues = true;
    console.error(`[i18n] ${localeCode}: extra keys (${extra.length})`);
    for (const key of extra) console.error(`  - ${key}`);
  }

  for (const key of sourceKeys) {
    if (!(key in locale)) continue;
    const sourceVars = extractPlaceholders(source[key]);
    const localeVars = extractPlaceholders(locale[key]);
    if (sourceVars.join("|") !== localeVars.join("|")) {
      hasIssues = true;
      console.error(`[i18n] ${localeCode}: placeholder mismatch for key "${key}"`);
      console.error(`  source: [${sourceVars.join(", ")}]`);
      console.error(`  locale: [${localeVars.join(", ")}]`);
    }
  }
}

if (hasIssues) {
  process.exit(1);
}

console.log(
  `[i18n] OK: ${sourceKeys.length} keys checked across ${localeFiles.length} locale(s).`,
);
