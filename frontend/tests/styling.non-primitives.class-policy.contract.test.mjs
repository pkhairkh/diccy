import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const srcRoot = path.join(repoRoot, "frontend", "src");
const issuesPath = path.join(repoRoot, "FRONTEND_ISSUES.md");
const nonPrimitiveMarkers = ["/components/components/", "/components/composites/", "/components/panels/"];
const isoDateRegex = /^\d{4}-\d{2}-\d{2}$/;
const todayIsoDate = new Date().toISOString().slice(0, 10);

function walk(dir) {
  if (!fs.existsSync(dir)) return [];
  const files = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const next = path.join(dir, entry.name);
    if (entry.isDirectory()) files.push(...walk(next));
    if (entry.isFile() && next.endsWith(".vue")) files.push(next);
  }
  return files;
}

function isNonPrimitive(filePath) {
  const normalized = filePath.replaceAll("\\", "/");
  return nonPrimitiveMarkers.some((marker) => normalized.includes(marker));
}

function parseExceptions() {
  const text = fs.existsSync(issuesPath) ? fs.readFileSync(issuesPath, "utf8") : "";
  const lines = text.split(/\r?\n/).filter((line) => line.startsWith("| FE-"));
  const entries = lines.map((line) => {
    const parts = line
      .split("|")
      .map((part) => part.trim())
      .filter(Boolean);
    return {
      id: parts[0] ?? "",
      layer: parts[1] ?? "",
      file: parts[2] ?? "",
      pattern: parts[3] ?? "",
      expiry: parts[7] ?? "",
    };
  });
  const ids = new Set();
  for (const entry of entries) {
    assert.ok(entry.id, "exception id is required");
    assert.ok(!ids.has(entry.id), `duplicate exception id: ${entry.id}`);
    ids.add(entry.id);
    assert.ok(fs.existsSync(path.join(repoRoot, entry.file)), `exception file missing: ${entry.file}`);
    assert.match(entry.expiry, isoDateRegex, `exception expiry must use YYYY-MM-DD: ${entry.id}`);
    assert.ok(
      entry.expiry >= todayIsoDate,
      `exception expired and must be removed or renewed: ${entry.id} (expiry ${entry.expiry}, today ${todayIsoDate})`,
    );
  }
  return entries;
}

const exceptions = parseExceptions();
const classRegex = /class\s*=\s*"([^"]+)"/g;
const styleRegex = /(^|\s)(:style|style)\s*=/;

const rawClassOccurrences = new Map();
const violations = [];

for (const file of walk(srcRoot)) {
  if (!isNonPrimitive(file)) continue;
  const text = fs.readFileSync(file, "utf8");
  const rel = path.relative(repoRoot, file).replaceAll("\\", "/");
  for (const line of text.split(/\r?\n/)) {
    let match = null;
    while ((match = classRegex.exec(line)) !== null) {
      const classValue = match[1].trim();
      const tokens = classValue.split(/\s+/).filter(Boolean);
      const tokenized = classValue.startsWith("classTokens.");
      if (tokens.length > 1 && !tokenized) {
        violations.push(`raw_multi_token_class ${rel}: ${classValue}`);
        rawClassOccurrences.set(classValue, [...(rawClassOccurrences.get(classValue) ?? []), rel]);
      }
    }
    if (styleRegex.test(line)) {
      const approved = exceptions.some((entry) => entry.file === rel && entry.pattern.includes(":style"));
      if (!approved) {
        violations.push(`style_attribute_non_primitive ${rel}: ${line.trim()}`);
      }
    }
  }
}

for (const [classValue, files] of rawClassOccurrences.entries()) {
  const uniqueFiles = [...new Set(files)];
  if (uniqueFiles.length > 1) {
    violations.push(
      `repeated_raw_multi_token_class "${classValue}" appears in ${uniqueFiles.length} files: ${uniqueFiles.join(", ")}`,
    );
  }
}

assert.equal(
  violations.length,
  0,
  `Class-policy violations:\n${violations.map((v) => `- ${v}`).join("\n")}`,
);
