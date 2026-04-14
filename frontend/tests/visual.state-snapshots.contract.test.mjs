import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const panelPath = path.join(repoRoot, "frontend", "src", "components", "panels", "ConnectorOpsPanel.vue");
const snapshotsRoot = path.join(repoRoot, "frontend", "tests", "visual_snapshots");

function normalizeTemplate(templateText) {
  return templateText
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .join("\n");
}

function extractTemplateFromVue(fileText) {
  const match = fileText.match(/<template>([\s\S]*?)<\/template>/);
  assert.ok(match, "Vue file must include a template block");
  return normalizeTemplate(match[1]);
}

const panel = fs.readFileSync(panelPath, "utf8");
const panelTemplate = extractTemplateFromVue(panel);

const snapshotFiles = [
  "connector-ops-panel.degraded-state.snapshot",
  "connector-ops-panel.timeout-state.snapshot",
  "connector-ops-panel.session-expired-state.snapshot",
];

for (const snapshotFile of snapshotFiles) {
  const snapshotPath = path.join(snapshotsRoot, snapshotFile);
  const expectedSnippet = normalizeTemplate(fs.readFileSync(snapshotPath, "utf8"));
  assert.ok(
    panelTemplate.includes(expectedSnippet),
    `visual state snapshot mismatch for ${snapshotFile}`,
  );
}
