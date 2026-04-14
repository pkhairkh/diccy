import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const srcRoot = path.join(repoRoot, "frontend", "src");
const issuesPath = path.join(repoRoot, "FRONTEND_ISSUES.md");
const styleRegex = /(^|\s)(:style|style)\s*=/;

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

function parseExceptions() {
  const text = fs.existsSync(issuesPath) ? fs.readFileSync(issuesPath, "utf8") : "";
  return text
    .split(/\r?\n/)
    .filter((line) => line.startsWith("| FE-"))
    .map((line) =>
      line
        .split("|")
        .map((part) => part.trim())
        .filter(Boolean),
    )
    .map((parts) => ({
      id: parts[0] ?? "",
      file: parts[2] ?? "",
      pattern: parts[3] ?? "",
    }));
}

const exceptions = parseExceptions();
const violations = [];

for (const file of walk(srcRoot)) {
  const rel = path.relative(repoRoot, file).replaceAll("\\", "/");
  const isNonPrimitive =
    rel.includes("/components/components/") ||
    rel.includes("/components/composites/") ||
    rel.includes("/components/panels/");
  if (!isNonPrimitive) continue;
  const text = fs.readFileSync(file, "utf8");
  if (!styleRegex.test(text)) continue;
  const approved = exceptions.some((entry) => entry.file === rel && entry.pattern.includes(":style"));
  if (!approved) {
    violations.push(`${rel} contains style binding but has no approved FRONTEND_ISSUES entry`);
  }
}

assert.equal(
  violations.length,
  0,
  `Non-primitive style attributes must be explicitly approved:\n${violations.map((v) => `- ${v}`).join("\n")}`,
);

