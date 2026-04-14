import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const adminPagePath = path.join(repoRoot, "frontend", "src", "components", "pages", "ConnectorAdminPage.vue");
const tenantPagePath = path.join(
  repoRoot,
  "frontend",
  "src",
  "components",
  "pages",
  "TenantHealthDashboardPage.vue",
);
const panelPath = path.join(repoRoot, "frontend", "src", "components", "panels", "ConnectorOpsPanel.vue");

const adminPage = fs.readFileSync(adminPagePath, "utf8");
const tenantPage = fs.readFileSync(tenantPagePath, "utf8");
const panel = fs.readFileSync(panelPath, "utf8");
const combined = `${adminPage}\n${tenantPage}\n${panel}`;

assert.match(
  panel,
  /<button[\s\S]*type="button"[\s\S]*Save connector policy[\s\S]*<\/button>/,
  "workflow mutation action must use a keyboard-focusable button element",
);
assert.ok(!/tabindex\s*=\s*"-1"/.test(combined), "admin workflow views must not hide controls from keyboard navigation");
assert.ok(!/(<div|<section|<article|<p|<span)[^>]*@click=/.test(combined), "click handlers on non-interactive tags are disallowed");
assert.match(
  panel,
  /:aria-label="`Rollout percent for \$\{connector\.alias\}`"/,
  "rollout mutation input must expose deterministic screen-reader labeling",
);
assert.match(
  panel,
  /:disabled="mutationsDisabled \|\| !hasDirtyConnectorConfig"/,
  "save mutation action must be disabled when session/role state is not mutation-eligible",
);
