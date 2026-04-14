# Frontend Environment Contract

Status: normative frontend runtime contract for production deployments.

## 1. API base path contract

- `VITE_WORKFLOW_API_BASE` controls frontend workflow API routing for connector operations.
- If unset, the frontend defaults to `/interop`.
- Frontend connector panel endpoints are derived as:
  - `${VITE_WORKFLOW_API_BASE}/connectors/status`
  - `${VITE_WORKFLOW_API_BASE}/connectors/features`
  - `${VITE_WORKFLOW_API_BASE}/connectors/rollout`
- Trailing slash values are normalized away at runtime.

## 2. Auth/session mode contract

- Frontend assumes backend-enforced auth and role controls (`admin`, `operator`, `viewer`).
- Connector mutation controls remain enabled only for:
  - `admin`
  - `operator`
- Session/auth expiry behavior is fail-closed:
  - backend `401` or `403` responses force mutation controls into disabled state,
  - UI must render explicit re-auth guidance,
  - no mutation retries are attempted while in expired state.

## 3. Connector admin mutation contract

- Rollout mutation requests are sent as `application/x-www-form-urlencoded`.
- Mutation UX requirements:
  - optimistic local edit experience,
  - deterministic rollback on mutation failure,
  - deterministic success/error confirmation messaging,
  - stale-state indicator when backend read APIs degrade after prior successful sync.

## 4. Downstream timeout and degraded-state contract

- Timeout-class backend failures (`408`/`504`) render explicit downstream-timeout messaging.
- When prior connector state exists, failures render degraded mode with stale-state timestamp.
- Empty-state responses must be explicit when no connector records are returned.

## 5. Telemetry and privacy controls

- Frontend runtime does not emit PHI/PII from connector panel state.
- Correlation and audit mapping remain backend responsibilities.
- Any new frontend telemetry controls must reference:
  - `docs/13-Error-Model-and-Telemetry.md`
  - `docs/09-Security-Threat-Model.md`

## 6. Verification references

- UI contract tests:
  - `frontend/tests/role-aware-rendering.contract.test.mjs`
  - `frontend/tests/role-matrix.auth-outcomes.contract.test.mjs`
  - `frontend/tests/accessibility.keyboard-nav.contract.test.mjs`
  - `frontend/tests/visual.state-snapshots.contract.test.mjs`
- Frontend implementation:
  - `frontend/src/components/panels/ConnectorOpsPanel.vue`
  - `frontend/src/components/pages/ConnectorAdminPage.vue`
  - `frontend/src/components/pages/TenantHealthDashboardPage.vue`
