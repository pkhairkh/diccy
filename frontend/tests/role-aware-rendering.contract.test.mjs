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

assert.match(
  adminPage,
  /const canViewAdminControls = computed\(\(\) => props\.role === "admin" \|\| props\.role === "operator"\);/,
  "admin page must restrict configuration visibility to admin/operator",
);
assert.match(
  adminPage,
  /<ConnectorOpsPanel v-if="canViewAdminControls" :role="role" \/>/,
  "admin page must gate connector panel by role contract",
);

assert.match(
  tenantPage,
  /const canViewOperationalPanel = computed\(\s*\(\) => props\.role === "admin" \|\| props\.role === "operator" \|\| props\.role === "viewer",\s*\);/,
  "tenant page must allow viewer read visibility",
);
assert.match(
  tenantPage,
  /const canMutateOperationalPanel = computed\(\(\) => props\.role === "admin" \|\| props\.role === "operator"\);/,
  "tenant page must restrict mutation state to admin/operator",
);
assert.ok(
  tenantPage.includes("Read-only mode active for this role."),
  "tenant page must declare read-only mode for non-mutation roles",
);

assert.match(
  panel,
  /const canMutateConnectors = computed\(\(\) => props\.role === "admin" \|\| props\.role === "operator"\);/,
  "panel must preserve mutation contract for admin/operator roles",
);
assert.match(
  panel,
  /<p v-if="!canMutateConnectors" :class="classTokens\.alertBanner">/,
  "panel must render non-mutation warning for read-only roles",
);
