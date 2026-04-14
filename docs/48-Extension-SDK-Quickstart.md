# Extension SDK Quickstart

Status: **Implemented quickstart baseline** (As of 2026-02-22)
Reference: `docs/41-Extension-Plugin-Contract.md`

## 1. Scope

This quickstart shows how to register an `rdvf` workflow extension and handle deterministic events.

## 2. API surface

Key types (from `rdvf::extensions`):
- `WorkflowExtension`
- `ExtensionRegistry`
- `ExtensionEvent`
- `ExtensionResult`
- `ExtensionEventKind`

## 3. Minimal sample implementation

```rust
use rdvf::extensions::{
    ExtensionEvent, ExtensionEventKind, ExtensionProfile, ExtensionRegistry, ExtensionResult,
    ExtensionStatus, WorkflowExtension,
};
use std::collections::BTreeMap;

struct ExampleExtension;

impl WorkflowExtension for ExampleExtension {
    fn id(&self) -> &'static str { "example.sdk" }
    fn profile(&self) -> ExtensionProfile { ExtensionProfile::Workstation }
    fn handle(&self, event: &ExtensionEvent) -> rdvf::Result<ExtensionResult> {
        if event.kind != ExtensionEventKind::MeasurementCaptured {
            return Ok(ExtensionResult::no_op());
        }
        let mut outputs = BTreeMap::new();
        outputs.insert("handled".to_string(), "true".to_string());
        Ok(ExtensionResult { status: ExtensionStatus::Accepted, outputs, error_code: None })
    }
}
```

## 4. Verification

- Duplicate extension IDs are rejected fail-closed.
- Registration order is deterministic and execution order follows registration order.
- Reference tests: `crates/rdvf/src/extensions.rs` unit tests.
