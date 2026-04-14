# Tailwind Plus UI Blocks Provenance (Panels)

## Scope

This note records Tailwind Plus UI Blocks provenance for non-primitive layout patterns
used by `frontend/src/components/panels`.

## Covered panel files

- `frontend/src/components/panels/ConnectorOpsPanel.vue`

## Pattern provenance map

| Pattern in panel | Local token source | Tailwind Plus UI Blocks baseline |
|---|---|---|
| Panel shell container (rounded border card shell) | `classTokens.panelShell` | Application shell + card container block patterns |
| Panel stack/section composition | `classTokens.panelBody`, `classTokens.panelSection`, `classTokens.panelTopOffset` | Dashboard section spacing and content stack patterns |
| Header split layout | `classTokens.panelHeader`, `classTokens.panelSubtleText` | List header and card header split-row patterns |
| Admin form grid and field grouping | `classTokens.formGrid`, `classTokens.fieldStack`, `classTokens.fieldInput`, `classTokens.fieldLabel` | Form layout blocks (two-column responsive forms) |
| Metric card and status bars | `classTokens.statCard`, `classTokens.metricBarTrack`, `classTokens.metricBarFill*`, `classTokens.metricBarWidth*` | Stats/metric blocks with progress tracks |
| Mutation warning and action CTA | `classTokens.alertBanner`, `classTokens.actionButton` | Alert + CTA utility block patterns |

## Governance

- Non-primitive panel layouts must continue using tokenized classes.
- New panel layout patterns must add provenance mapping rows in this file.
- Raw utility strings and inline style attributes in panels are not allowed without an approved exception entry.
