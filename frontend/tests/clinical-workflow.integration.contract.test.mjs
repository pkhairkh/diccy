import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const pagePath = path.join(repoRoot, "frontend", "src", "components", "pages", "ClinicalWorkflowPage.vue");
const measurementPanelPath = path.join(
  repoRoot,
  "frontend",
  "src",
  "components",
  "panels",
  "ClinicalMeasurementPanel.vue",
);
const segmentationPanelPath = path.join(
  repoRoot,
  "frontend",
  "src",
  "components",
  "panels",
  "ClinicalSegmentationPanel.vue",
);
const mprPanelPath = path.join(repoRoot, "frontend", "src", "components", "panels", "ClinicalMprPanel.vue");
const fusionPanelPath = path.join(
  repoRoot,
  "frontend",
  "src",
  "components",
  "panels",
  "ClinicalFusionPanel.vue",
);
const rtPanelPath = path.join(repoRoot, "frontend", "src", "components", "panels", "ClinicalRtPanel.vue");

const page = fs.readFileSync(pagePath, "utf8");
const measurementPanel = fs.readFileSync(measurementPanelPath, "utf8");
const segmentationPanel = fs.readFileSync(segmentationPanelPath, "utf8");
const mprPanel = fs.readFileSync(mprPanelPath, "utf8");
const fusionPanel = fs.readFileSync(fusionPanelPath, "utf8");
const rtPanel = fs.readFileSync(rtPanelPath, "utf8");

assert.match(page, /import ClinicalMeasurementPanel/);
assert.match(page, /import ClinicalSegmentationPanel/);
assert.match(page, /import ClinicalMprPanel/);
assert.match(page, /import ClinicalFusionPanel/);
assert.match(page, /import ClinicalRtPanel/);

assert.match(page, /orderedMeasurements[\s\S]*localeCompare/);
assert.match(page, /orderedSegmentations[\s\S]*localeCompare/);
assert.match(page, /commitMutation\(/);
assert.match(page, /function undo\(/);
assert.match(page, /function redo\(/);
assert.match(page, /function jumpToMeasurement\(/);
assert.match(page, /function exportMeasurements\(/);

assert.match(measurementPanel, />\s*Create\s*<\/button>/);
assert.match(measurementPanel, />\s*Rename\s*<\/button>/);
assert.match(measurementPanel, />\s*Remove\s*<\/button>/);
assert.match(measurementPanel, /Jump to viewport/);
assert.match(measurementPanel, /Export CSV/);
assert.match(measurementPanel, /Export JSON/);
assert.match(measurementPanel, />\s*Undo\s*<\/button>/);
assert.match(measurementPanel, />\s*Redo\s*<\/button>/);

assert.match(segmentationPanel, /Create, import, style, lock, and remove overlays/);
assert.match(segmentationPanel, />\s*Create\s*<\/button>/);
assert.match(segmentationPanel, />\s*Import\s*<\/button>/);
assert.match(segmentationPanel, />\s*Style\s*<\/button>/);
assert.match(segmentationPanel, /\{\{ seg\.locked \? "Unlock" : "Lock" \}\}/);
assert.match(segmentationPanel, />\s*Remove\s*<\/button>/);

assert.match(mprPanel, /MPR and Viewport Linkage/);
assert.match(mprPanel, /mipEnabled \? "Disable" : "Enable"/);
assert.match(mprPanel, /volume3dEnabled \? "Disable" : "Enable"/);
assert.match(mprPanel, /Capability-gated controls are disabled/);

assert.match(fusionPanel, /Alpha \(%\)/);
assert.match(fusionPanel, /Colormap/);
assert.match(fusionPanel, /Registration status/);
assert.match(fusionPanel, /RMSE/);

assert.match(rtPanel, /Dose opacity \(%\)/);
assert.match(rtPanel, /Iso-dose threshold \(%\)/);
assert.match(page, /Dose opacity must be within 0\.\.100/);
assert.match(page, /Iso-dose threshold must be within 1\.\.100/);
