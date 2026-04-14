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
  "admin page role matrix must continue to allow only admin/operator mutation surfaces",
);
assert.match(
  tenantPage,
  /const canViewOperationalPanel = computed\(\s*\(\) => props\.role === "admin" \|\| props\.role === "operator" \|\| props\.role === "viewer",\s*\);/,
  "tenant page role matrix must preserve viewer read surface",
);
assert.match(
  panel,
  /const canMutateConnectors = computed\(\(\) => props\.role === "admin" \|\| props\.role === "operator"\);/,
  "panel mutation contract must remain admin/operator only",
);
assert.match(
  panel,
  /if \(status === 401 \|\| status === 403\) \{\s*return "auth_expired";\s*\}/,
  "backend auth outcomes must map 401/403 to session-expired UX state",
);
assert.match(
  panel,
  /sessionState = ref<"active" \| "expired">\("active"\)/,
  "panel must track role/session expiry state for mutation gating",
);
assert.ok(
  panel.includes("Re-authenticate with admin/operator credentials and retry."),
  "session-expiry UX must include explicit re-auth guidance",
);
