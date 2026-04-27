//! Integration tests for the S14-T2 RBAC authorization module.

use dicom_auth::*;
use std::str::FromStr;
use std::sync::Arc;

// ===========================================================================
// Role and Permission enum tests
// ===========================================================================

#[test]
fn role_from_str_valid() {
    assert_eq!(Role::from_str("Radiologist").unwrap(), Role::Radiologist);
    assert_eq!(Role::from_str("Technologist").unwrap(), Role::Technologist);
    assert_eq!(
        Role::from_str("ReferringPhysician").unwrap(),
        Role::ReferringPhysician
    );
    assert_eq!(Role::from_str("Administrator").unwrap(), Role::Administrator);
    assert_eq!(Role::from_str("Researcher").unwrap(), Role::Researcher);
}

#[test]
fn role_from_str_invalid() {
    assert!(Role::from_str("UnknownRole").is_err());
    assert!(Role::from_str("").is_err());
    assert!(Role::from_str("radiologist").is_err()); // case-sensitive
}

#[test]
fn role_as_str() {
    assert_eq!(Role::Radiologist.as_str(), "Radiologist");
    assert_eq!(Role::Administrator.as_str(), "Administrator");
}

#[test]
fn permission_from_str_valid() {
    assert_eq!(
        Permission::from_str("ReadStudy").unwrap(),
        Permission::ReadStudy
    );
    assert_eq!(
        Permission::from_str("WriteReport").unwrap(),
        Permission::WriteReport
    );
    assert_eq!(
        Permission::from_str("DeleteStudy").unwrap(),
        Permission::DeleteStudy
    );
    assert_eq!(
        Permission::from_str("ExportData").unwrap(),
        Permission::ExportData
    );
    assert_eq!(
        Permission::from_str("AdminConfig").unwrap(),
        Permission::AdminConfig
    );
    assert_eq!(
        Permission::from_str("BreakGlass").unwrap(),
        Permission::BreakGlass
    );
}

#[test]
fn permission_from_str_invalid() {
    assert!(Permission::from_str("Unknown").is_err());
    assert!(Permission::from_str("").is_err());
}

// ===========================================================================
// RolePermissionMap tests
// ===========================================================================

#[test]
fn default_role_permission_map_radiologist_permissions() {
    let map = rbac::RolePermissionMap::new();
    let perms = map.permissions_for(Role::Radiologist).unwrap();
    assert!(perms.contains(&Permission::ReadStudy));
    assert!(perms.contains(&Permission::WriteReport));
    assert!(perms.contains(&Permission::ExportData));
    assert!(!perms.contains(&Permission::DeleteStudy));
    assert!(!perms.contains(&Permission::AdminConfig));
    assert!(!perms.contains(&Permission::BreakGlass));
}

#[test]
fn default_role_permission_map_technologist_permissions() {
    let map = rbac::RolePermissionMap::new();
    let perms = map.permissions_for(Role::Technologist).unwrap();
    assert!(perms.contains(&Permission::ReadStudy));
    assert!(perms.contains(&Permission::ExportData));
    assert!(!perms.contains(&Permission::WriteReport));
    assert!(!perms.contains(&Permission::DeleteStudy));
    assert!(!perms.contains(&Permission::AdminConfig));
}

#[test]
fn default_role_permission_map_referring_physician_permissions() {
    let map = rbac::RolePermissionMap::new();
    let perms = map.permissions_for(Role::ReferringPhysician).unwrap();
    assert!(perms.contains(&Permission::ReadStudy));
    assert!(!perms.contains(&Permission::WriteReport));
    assert!(!perms.contains(&Permission::DeleteStudy));
    assert!(!perms.contains(&Permission::ExportData));
    assert!(!perms.contains(&Permission::AdminConfig));
}

#[test]
fn default_role_permission_map_administrator_has_all_permissions() {
    let map = rbac::RolePermissionMap::new();
    let perms = map.permissions_for(Role::Administrator).unwrap();
    assert!(perms.contains(&Permission::ReadStudy));
    assert!(perms.contains(&Permission::WriteReport));
    assert!(perms.contains(&Permission::DeleteStudy));
    assert!(perms.contains(&Permission::ExportData));
    assert!(perms.contains(&Permission::AdminConfig));
    assert!(perms.contains(&Permission::BreakGlass));
}

#[test]
fn default_role_permission_map_researcher_permissions() {
    let map = rbac::RolePermissionMap::new();
    let perms = map.permissions_for(Role::Researcher).unwrap();
    assert!(perms.contains(&Permission::ReadStudy));
    assert!(perms.contains(&Permission::ExportData));
    assert!(!perms.contains(&Permission::WriteReport));
    assert!(!perms.contains(&Permission::DeleteStudy));
    assert!(!perms.contains(&Permission::AdminConfig));
}

#[test]
fn radiologist_can_read_write_but_not_admin() {
    // S14-T2 acceptance: radiologist can read/write but not admin
    let map = rbac::RolePermissionMap::new();
    assert!(map.has_permission(Role::Radiologist, Permission::ReadStudy));
    assert!(map.has_permission(Role::Radiologist, Permission::WriteReport));
    assert!(!map.has_permission(Role::Radiologist, Permission::AdminConfig));
    assert!(!map.has_permission(Role::Radiologist, Permission::DeleteStudy));
    assert!(!map.has_permission(Role::Radiologist, Permission::BreakGlass));
}

#[test]
fn administrator_has_all_permissions() {
    // S14-T2 acceptance: administrator has all permissions
    let map = rbac::RolePermissionMap::new();
    for perm in Permission::all() {
        assert!(
            map.has_permission(Role::Administrator, *perm),
            "Administrator should have {:?}",
            perm
        );
    }
}

#[test]
fn role_permission_map_grant_and_revoke() {
    let mut map = rbac::RolePermissionMap::empty();
    assert!(!map.has_permission(Role::Radiologist, Permission::ReadStudy));

    map.grant(Role::Radiologist, Permission::ReadStudy);
    assert!(map.has_permission(Role::Radiologist, Permission::ReadStudy));

    map.revoke(Role::Radiologist, Permission::ReadStudy);
    assert!(!map.has_permission(Role::Radiologist, Permission::ReadStudy));
}

#[test]
fn role_permission_map_role_count() {
    let map = rbac::RolePermissionMap::new();
    assert_eq!(map.role_count(), 5); // 5 default roles
}

// ===========================================================================
// StudyAccessList tests
// ===========================================================================

#[test]
fn study_access_list_no_restrictions_by_default() {
    let list = rbac::StudyAccessList::new();
    // No entry = unrestricted access
    assert!(list.can_access_study(Role::Radiologist, Some("1.2.3.4")));
    assert!(list.can_access_study(Role::Radiologist, None));
}

#[test]
fn study_access_list_grant_and_check() {
    let mut list = rbac::StudyAccessList::new();
    list.grant_study(Role::Researcher, "1.2.3.4".to_string());
    list.grant_study(Role::Researcher, "5.6.7.8".to_string());

    assert!(list.can_access_study(Role::Researcher, Some("1.2.3.4")));
    assert!(list.can_access_study(Role::Researcher, Some("5.6.7.8")));
    assert!(!list.can_access_study(Role::Researcher, Some("9.10.11.12")));
}

#[test]
fn study_access_list_other_roles_unrestricted() {
    let mut list = rbac::StudyAccessList::new();
    list.grant_study(Role::Researcher, "1.2.3.4".to_string());

    // Administrator has no explicit entry, so is unrestricted
    assert!(list.can_access_study(Role::Administrator, Some("9.10.11.12")));
}

#[test]
fn study_access_list_revoke() {
    let mut list = rbac::StudyAccessList::new();
    list.grant_study(Role::Researcher, "1.2.3.4".to_string());
    assert!(list.can_access_study(Role::Researcher, Some("1.2.3.4")));

    list.revoke_study(Role::Researcher, "1.2.3.4");
    // After revoking the only study, the set is non-empty but doesn't contain the study
    assert!(!list.can_access_study(Role::Researcher, Some("1.2.3.4")));
}

// ===========================================================================
// RbacPolicy tests
// ===========================================================================

#[test]
fn rbac_policy_default_has_all_roles() {
    let policy = RbacPolicy::new();
    assert!(policy.has_permission(Role::Radiologist, Permission::ReadStudy));
    assert!(policy.has_permission(Role::Administrator, Permission::AdminConfig));
}

#[test]
fn rbac_policy_can_access_study_unrestricted() {
    let policy = RbacPolicy::new();
    assert!(policy.can_access_study(Role::Radiologist, Some("any.study.uid")));
}

// ===========================================================================
// permission_for_action mapping tests
// ===========================================================================

#[test]
fn permission_for_action_maps_read_operations() {
    assert_eq!(
        permission_for_action(AuthAction::Query, AuthResourceKey::Study),
        Some(Permission::ReadStudy)
    );
    assert_eq!(
        permission_for_action(AuthAction::Retrieve, AuthResourceKey::Instance),
        Some(Permission::ReadStudy)
    );
    assert_eq!(
        permission_for_action(AuthAction::Echo, AuthResourceKey::Study),
        Some(Permission::ReadStudy)
    );
    assert_eq!(
        permission_for_action(AuthAction::WebRequest, AuthResourceKey::Study),
        Some(Permission::ReadStudy)
    );
}

#[test]
fn permission_for_action_maps_write_operations() {
    assert_eq!(
        permission_for_action(AuthAction::Store, AuthResourceKey::Instance),
        Some(Permission::WriteReport)
    );
    assert_eq!(
        permission_for_action(AuthAction::ViewerMeasurementWrite, AuthResourceKey::ViewerMeasurement),
        Some(Permission::WriteReport)
    );
}

#[test]
fn permission_for_action_maps_delete() {
    assert_eq!(
        permission_for_action(AuthAction::Delete, AuthResourceKey::Study),
        Some(Permission::DeleteStudy)
    );
}

#[test]
fn permission_for_action_maps_admin_operations() {
    assert_eq!(
        permission_for_action(AuthAction::Associate, AuthResourceKey::Study),
        Some(Permission::AdminConfig)
    );
    assert_eq!(
        permission_for_action(AuthAction::StorageCommitment, AuthResourceKey::StorageCommitment),
        Some(Permission::AdminConfig)
    );
}

// ===========================================================================
// RbacAuthorizer with role_extractor tests
// ===========================================================================

#[test]
fn rbac_authorizer_with_role_extractor_radiologist_can_read() {
    let authorizer = RbacAuthorizer::new()
        .with_rbac_policy(RbacPolicy::new())
        .with_role_extractor(Arc::new(|_subject| Some(Role::Radiologist)));

    let request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Query,
        subject: AuthSubject {
            principal: Some("dr.smith"),
            peer: None,
        },
        resource: AuthResource {
            study_uid: Some("1.2.3.4"),
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
    };

    let decision = authorizer.authorize(&request).expect("decision");
    assert!(decision.is_allowed());
}

#[test]
fn rbac_authorizer_with_role_extractor_radiologist_cannot_admin() {
    let authorizer = RbacAuthorizer::new()
        .with_rbac_policy(RbacPolicy::new())
        .with_role_extractor(Arc::new(|_subject| Some(Role::Radiologist)));

    let request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Associate,
        subject: AuthSubject {
            principal: Some("dr.smith"),
            peer: None,
        },
        resource: AuthResource::none(),
    };

    let decision = authorizer.authorize(&request).expect("decision");
    assert!(!decision.is_allowed());
    assert!(matches!(
        decision,
        AuthDecision::Deny(AuthDenyReason::Unauthorized)
    ));
}

#[test]
fn rbac_authorizer_with_role_extractor_administrator_can_admin() {
    let authorizer = RbacAuthorizer::new()
        .with_rbac_policy(RbacPolicy::new())
        .with_role_extractor(Arc::new(|_subject| Some(Role::Administrator)));

    let request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Associate,
        subject: AuthSubject {
            principal: Some("admin"),
            peer: None,
        },
        resource: AuthResource::none(),
    };

    let decision = authorizer.authorize(&request).expect("decision");
    assert!(decision.is_allowed());
}

#[test]
fn rbac_authorizer_with_role_extractor_no_role_denies() {
    let authorizer = RbacAuthorizer::new()
        .with_rbac_policy(RbacPolicy::new())
        .with_role_extractor(Arc::new(|_subject| None));

    let request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Query,
        subject: AuthSubject::anonymous(),
        resource: AuthResource::none(),
    };

    let decision = authorizer.authorize(&request).expect("decision");
    assert!(!decision.is_allowed());
    assert!(matches!(
        decision,
        AuthDecision::Deny(AuthDenyReason::Unauthenticated)
    ));
}

#[test]
fn rbac_authorizer_with_role_extractor_based_on_principal() {
    let authorizer = RbacAuthorizer::new()
        .with_rbac_policy(RbacPolicy::new())
        .with_role_extractor(Arc::new(|subject| {
            match subject.principal {
                Some("admin") => Some(Role::Administrator),
                Some("dr.smith") => Some(Role::Radiologist),
                Some("tech.jones") => Some(Role::Technologist),
                _ => None,
            }
        }));

    // Radiologist can read
    let request = AuthRequest {
        scope: AuthScope::Viewer,
        action: AuthAction::Query,
        subject: AuthSubject {
            principal: Some("dr.smith"),
            peer: None,
        },
        resource: AuthResource::none(),
    };
    assert!(authorizer.authorize(&request).expect("decision").is_allowed());

    // Radiologist cannot delete
    let request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Delete,
        subject: AuthSubject {
            principal: Some("dr.smith"),
            peer: None,
        },
        resource: AuthResource::none(),
    };
    assert!(!authorizer.authorize(&request).expect("decision").is_allowed());

    // Administrator can delete
    let request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Delete,
        subject: AuthSubject {
            principal: Some("admin"),
            peer: None,
        },
        resource: AuthResource::none(),
    };
    assert!(authorizer.authorize(&request).expect("decision").is_allowed());

    // Unknown principal denied
    let request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Query,
        subject: AuthSubject {
            principal: Some("unknown"),
            peer: None,
        },
        resource: AuthResource::none(),
    };
    assert!(!authorizer.authorize(&request).expect("decision").is_allowed());
}

#[test]
fn rbac_authorizer_study_level_access_control() {
    let mut policy = RbacPolicy::new();
    policy.study_access.grant_study(Role::Researcher, "1.2.3.4".to_string());

    let authorizer = RbacAuthorizer::new()
        .with_rbac_policy(policy)
        .with_role_extractor(Arc::new(|_subject| Some(Role::Researcher)));

    // Can access allowed study
    let request = AuthRequest {
        scope: AuthScope::Viewer,
        action: AuthAction::Query,
        subject: AuthSubject {
            principal: Some("researcher"),
            peer: None,
        },
        resource: AuthResource {
            study_uid: Some("1.2.3.4"),
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
    };
    assert!(authorizer.authorize(&request).expect("decision").is_allowed());

    // Cannot access non-allowed study
    let request = AuthRequest {
        scope: AuthScope::Viewer,
        action: AuthAction::Query,
        subject: AuthSubject {
            principal: Some("researcher"),
            peer: None,
        },
        resource: AuthResource {
            study_uid: Some("9.9.9.9"),
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
    };
    assert!(!authorizer.authorize(&request).expect("decision").is_allowed());
}

// ===========================================================================
// TOML policy configuration tests
// ===========================================================================

#[test]
fn rbac_policy_from_toml_basic() {
    let toml = r#"
[roles.Radiologist]
permissions = ["ReadStudy", "WriteReport", "ExportData"]

[roles.ReferringPhysician]
permissions = ["ReadStudy"]
"#;

    let policy = RbacPolicy::from_toml(toml).expect("parse TOML");
    assert!(policy.has_permission(Role::Radiologist, Permission::ReadStudy));
    assert!(policy.has_permission(Role::Radiologist, Permission::WriteReport));
    assert!(policy.has_permission(Role::Radiologist, Permission::ExportData));
    assert!(policy.has_permission(Role::ReferringPhysician, Permission::ReadStudy));
    assert!(!policy.has_permission(Role::ReferringPhysician, Permission::WriteReport));
}

#[test]
fn rbac_policy_from_toml_with_study_access() {
    let toml = r#"
[roles.Radiologist]
permissions = ["ReadStudy"]

[study_access.Radiologist]
studies = ["1.2.3.4", "5.6.7.8"]
"#;

    let policy = RbacPolicy::from_toml(toml).expect("parse TOML");
    assert!(policy.can_access_study(Role::Radiologist, Some("1.2.3.4")));
    assert!(policy.can_access_study(Role::Radiologist, Some("5.6.7.8")));
    assert!(!policy.can_access_study(Role::Radiologist, Some("9.10.11.12")));
}

#[test]
fn rbac_policy_from_toml_empty() {
    let policy = RbacPolicy::from_toml("").expect("parse empty TOML");
    // No roles configured, so no permissions
    assert!(!policy.has_permission(Role::Radiologist, Permission::ReadStudy));
}

#[test]
fn rbac_policy_from_toml_comments_ignored() {
    let toml = r#"
# This is a comment
[roles.Administrator]
permissions = ["ReadStudy", "AdminConfig"]
"#;

    let policy = RbacPolicy::from_toml(toml).expect("parse TOML with comments");
    assert!(policy.has_permission(Role::Administrator, Permission::ReadStudy));
    assert!(policy.has_permission(Role::Administrator, Permission::AdminConfig));
}

// ===========================================================================
// JSON policy configuration tests
// ===========================================================================

#[test]
fn rbac_policy_from_json_basic() {
    let json = r#"{
  "roles": {
    "Radiologist": ["ReadStudy", "WriteReport", "ExportData"],
    "ReferringPhysician": ["ReadStudy"]
  }
}"#;

    let policy = RbacPolicy::from_json(json).expect("parse JSON");
    assert!(policy.has_permission(Role::Radiologist, Permission::ReadStudy));
    assert!(policy.has_permission(Role::Radiologist, Permission::WriteReport));
    assert!(policy.has_permission(Role::ReferringPhysician, Permission::ReadStudy));
    assert!(!policy.has_permission(Role::ReferringPhysician, Permission::WriteReport));
}

#[test]
fn rbac_policy_from_json_with_study_access() {
    let json = r#"{
  "roles": {
    "Researcher": ["ReadStudy"]
  },
  "study_access": {
    "Researcher": ["1.2.3.4"]
  }
}"#;

    let policy = RbacPolicy::from_json(json).expect("parse JSON");
    assert!(policy.has_permission(Role::Researcher, Permission::ReadStudy));
    assert!(policy.can_access_study(Role::Researcher, Some("1.2.3.4")));
    assert!(!policy.can_access_study(Role::Researcher, Some("9.9.9.9")));
}

#[test]
fn rbac_policy_from_json_empty() {
    let policy = RbacPolicy::from_json("{}").expect("parse empty JSON");
    assert!(!policy.has_permission(Role::Radiologist, Permission::ReadStudy));
}

// ===========================================================================
// Legacy RbacAuthorizer backward compatibility
// ===========================================================================

#[test]
fn rbac_authorizer_legacy_still_works() {
    let rbac = RbacAuthorizer::new();
    let request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Echo,
        subject: AuthSubject::anonymous(),
        resource: AuthResource {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
    };
    let decision = rbac.authorize(&request).expect("decision");
    assert!(decision.is_allowed());
}
