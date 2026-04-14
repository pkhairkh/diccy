#![cfg(feature = "workflow-main-tests")]

#[test]
fn workflow_main_feature_is_enabled_for_integration_target() {
    assert!(cfg!(feature = "workflow-main-tests"));
}
