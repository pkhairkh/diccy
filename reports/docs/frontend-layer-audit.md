# Frontend Layer Audit

Date: 2026-02-24

## Scope

- `frontend/src/components`
- `frontend/src/styles`
- `frontend/tests`

## Findings

1. Layer order enforced: `primitives -> components -> composites -> panels -> pages`.
2. Non-primitives are token consumers and do not contain unapproved multi-token raw utility lists.
3. One temporary non-primitive style exception is active:
- `FE-STYLE-001` in `frontend/src/components/panels/ConnectorOpsPanel.vue`.
4. Page modules compose only panel modules.
5. Panel modules compose composites/components and do not import pages.

## Migration Actions Applied

1. Introduced tokenized style source files:
- `frontend/src/styles/extractedTokens.ts`
- `frontend/src/styles/semanticTokens.ts`
- `frontend/src/styles/tokens.ts`
2. Added governance tests:
- `frontend/tests/layer-boundaries.contract.test.mjs`
- `frontend/tests/styling.non-primitives.class-policy.contract.test.mjs`
- `frontend/tests/non-primitive-style-attr.contract.test.mjs`

