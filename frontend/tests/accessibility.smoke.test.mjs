import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const adminPagePath = path.join(
  repoRoot,
  "frontend",
  "src",
  "components",
  "pages",
  "ConnectorAdminPage.vue",
);
const tenantPagePath = path.join(
  repoRoot,
  "frontend",
  "src",
  "components",
  "pages",
  "TenantHealthDashboardPage.vue",
);
const panelPath = path.join(
  repoRoot,
  "frontend",
  "src",
  "components",
  "panels",
  "ConnectorOpsPanel.vue",
);

const adminPage = fs.readFileSync(adminPagePath, "utf8");
const tenantPage = fs.readFileSync(tenantPagePath, "utf8");
const panel = fs.readFileSync(panelPath, "utf8");

assert.match(adminPage, /<h1[^>]*>Connector Configuration<\/h1>/);
assert.match(tenantPage, /<h1[^>]*>Tenant Integration Health<\/h1>/);
assert.match(panel, /<h2>Connector Health<\/h2>/);
assert.match(panel, /<h2>Connector Configuration<\/h2>/);
assert.match(panel, /<h2>Operational Metrics<\/h2>/);

assert.ok(
  adminPage.includes("Role does not permit connector configuration access."),
  "admin page must keep role-gated fallback copy",
);
assert.ok(
  tenantPage.includes("Read-only mode active for this role."),
  "tenant page must declare read-only fallback for viewer role",
);
