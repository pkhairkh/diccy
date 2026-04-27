//! Tests for the dicom-regulatory crate verifying documentation completeness.

use dicom_regulatory::*;

// ===========================================================================
// SRS tests
// ===========================================================================

#[test]
fn srs_builds_from_manifests() {
    let srs = srs::build_srs_from_manifests();
    assert!(!srs.requirements.is_empty(), "SRS must contain requirements");
    assert_eq!(srs.version, "0.1.0");
    assert_eq!(srs.safety_class, srs::SafetyClass::ClassC);
}

#[test]
fn srs_every_requirement_has_id() {
    let srs = srs::build_srs_from_manifests();
    for req in &srs.requirements {
        assert!(!req.id.is_empty(), "requirement must have an ID");
        assert!(
            req.id.starts_with("REQ-"),
            "requirement ID must follow REQ-<DOMAIN>-<NNN> format: {}",
            req.id
        );
    }
}

#[test]
fn srs_every_requirement_has_description() {
    let srs = srs::build_srs_from_manifests();
    for req in &srs.requirements {
        assert!(!req.description.is_empty(), "requirement {} must have a description", req.id);
    }
}

#[test]
fn srs_requirement_ids_are_unique() {
    let srs = srs::build_srs_from_manifests();
    let mut ids = std::collections::HashSet::new();
    for req in &srs.requirements {
        assert!(ids.insert(&req.id), "duplicate requirement ID: {}", req.id);
    }
}

#[test]
fn srs_covers_all_categories() {
    let srs = srs::build_srs_from_manifests();
    let categories: std::collections::HashSet<srs::ReqCategory> =
        srs.requirements.iter().map(|r| r.category).collect();
    assert!(categories.contains(&srs::ReqCategory::Functional), "must have Functional requirements");
    assert!(categories.contains(&srs::ReqCategory::Safety), "must have Safety requirements");
    assert!(categories.contains(&srs::ReqCategory::Security), "must have Security requirements");
    assert!(categories.contains(&srs::ReqCategory::Interoperability), "must have Interoperability requirements");
}

#[test]
fn srs_safety_class_is_class_c() {
    let srs = srs::build_srs_from_manifests();
    // DiCCY is a medical device where death or serious injury is possible
    // from incorrect rendering, so Class C is required per IEC 62304.
    assert_eq!(srs.safety_class, srs::SafetyClass::ClassC);
}

// ===========================================================================
// SDD tests
// ===========================================================================

#[test]
fn sdd_builds_from_workspace() {
    let sdd = sdd::build_sdd_from_workspace();
    assert!(!sdd.architecture.crates.is_empty(), "SDD must describe workspace crates");
    assert!(!sdd.architecture.dependencies.is_empty(), "SDD must describe crate dependencies");
    assert!(!sdd.design_elements.is_empty(), "SDD must have design elements");
}

#[test]
fn sdd_every_crate_has_name_and_purpose() {
    let sdd = sdd::build_sdd_from_workspace();
    for krate in &sdd.architecture.crates {
        assert!(!krate.name.is_empty(), "crate must have a name");
        assert!(!krate.purpose.is_empty(), "crate {} must have a purpose", krate.name);
        assert!(!krate.bounded_context.is_empty(), "crate {} must have a bounded context", krate.name);
        assert!(!krate.public_api.is_empty(), "crate {} must list public API", krate.name);
    }
}

#[test]
fn sdd_core_crate_exists() {
    let sdd = sdd::build_sdd_from_workspace();
    let names: Vec<&str> = sdd.architecture.crates.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"dicom-core"), "SDD must include dicom-core");
    assert!(names.contains(&"dicom-audit"), "SDD must include dicom-audit");
    assert!(names.contains(&"dicom-auth"), "SDD must include dicom-auth");
}

#[test]
fn sdd_dependency_edges_reference_existing_crates() {
    let sdd = sdd::build_sdd_from_workspace();
    let names: std::collections::HashSet<&str> =
        sdd.architecture.crates.iter().map(|c| c.name.as_str()).collect();
    for edge in &sdd.architecture.dependencies {
        assert!(names.contains(edge.from.as_str()), "dependency source '{}' must be a known crate", edge.from);
        assert!(names.contains(edge.to.as_str()), "dependency target '{}' must be a known crate", edge.to);
    }
}

#[test]
fn sdd_design_elements_reference_known_requirements() {
    let sdd = sdd::build_sdd_from_workspace();
    let srs = srs::build_srs_from_manifests();
    let req_ids: std::collections::HashSet<&str> =
        srs.requirements.iter().map(|r| r.id.as_str()).collect();
    for de in &sdd.design_elements {
        assert!(
            req_ids.contains(de.parent_requirement.as_str()),
            "design element {} references unknown requirement {}",
            de.id,
            de.parent_requirement
        );
    }
}

// ===========================================================================
// STP tests
// ===========================================================================

#[test]
fn stp_builds_from_tests() {
    let stp = stp::build_stp_from_tests();
    assert!(!stp.test_suites.is_empty(), "STP must contain test suites");
    assert!(!stp.traceability_matrix.is_empty(), "STP must have a traceability matrix");
}

#[test]
fn stp_traceability_covers_all_requirements() {
    let srs = srs::build_srs_from_manifests();
    let stp = stp::build_stp_from_tests();
    let traced_ids: std::collections::HashSet<&str> =
        stp.traceability_matrix.iter().map(|e| e.requirement_id.as_str()).collect();
    for req in &srs.requirements {
        assert!(
            traced_ids.contains(req.id.as_str()),
            "requirement {} not found in traceability matrix",
            req.id
        );
    }
}

#[test]
fn stp_every_test_case_references_valid_requirement() {
    let srs = srs::build_srs_from_manifests();
    let req_ids: std::collections::HashSet<&str> =
        srs.requirements.iter().map(|r| r.id.as_str()).collect();
    let stp = stp::build_stp_from_tests();
    for suite in &stp.test_suites {
        for tc in &suite.test_cases {
            for req_id in &tc.requirement_ids {
                assert!(
                    req_ids.contains(req_id.as_str()),
                    "test case {} references unknown requirement {}",
                    tc.id,
                    req_id
                );
            }
        }
    }
}

#[test]
fn stp_test_case_ids_are_unique() {
    let stp = stp::build_stp_from_tests();
    let mut ids = std::collections::HashSet::new();
    for suite in &stp.test_suites {
        for tc in &suite.test_cases {
            assert!(ids.insert(tc.id.clone()), "duplicate test case ID: {}", tc.id);
        }
    }
}

// ===========================================================================
// RMF tests
// ===========================================================================

#[test]
fn rmf_builds_from_issues() {
    let rmf = rmf::build_rmf_from_issues();
    assert!(!rmf.hazards.is_empty(), "RMF must contain hazard entries");
}

#[test]
fn rmf_hazard_ids_are_unique() {
    let rmf = rmf::build_rmf_from_issues();
    let mut ids = std::collections::HashSet::new();
    for hazard in &rmf.hazards {
        assert!(ids.insert(&hazard.id), "duplicate hazard ID: {}", hazard.id);
    }
}

#[test]
fn rmf_risk_matrix_is_5x5() {
    let rmf = rmf::build_rmf_from_issues();
    assert_eq!(rmf.risk_matrix.cells.len(), 5, "risk matrix must have 5 probability rows");
    for row in &rmf.risk_matrix.cells {
        assert_eq!(row.len(), 5, "each risk matrix row must have 5 severity columns");
    }
}

#[test]
fn rmf_risk_matrix_classifies_correctly() {
    // Verify some known classifications
    assert_eq!(
        rmf::RiskLevel::from_severity_probability(rmf::Severity::Negligible, rmf::Probability::Improbable),
        rmf::RiskLevel::Acceptable,
        "1×1 = 1 should be Acceptable"
    );
    assert_eq!(
        rmf::RiskLevel::from_severity_probability(rmf::Severity::Catastrophic, rmf::Probability::Frequent),
        rmf::RiskLevel::Unacceptable,
        "5×5 = 25 should be Unacceptable"
    );
    assert_eq!(
        rmf::RiskLevel::from_severity_probability(rmf::Severity::Serious, rmf::Probability::Occasional),
        rmf::RiskLevel::ALARP,
        "3×3 = 9 should be ALARP"
    );
}

#[test]
fn rmf_every_hazard_has_mitigation() {
    let rmf = rmf::build_rmf_from_issues();
    for hazard in &rmf.hazards {
        assert!(!hazard.mitigation.is_empty(), "hazard {} must have mitigation", hazard.id);
    }
}

#[test]
fn rmf_residual_risk_is_not_higher_than_initial() {
    let rmf = rmf::build_rmf_from_issues();
    let level_value = |l: rmf::RiskLevel| -> u8 {
        match l {
            rmf::RiskLevel::Acceptable => 1,
            rmf::RiskLevel::ALARP => 2,
            rmf::RiskLevel::Unacceptable => 3,
        }
    };
    for hazard in &rmf.hazards {
        assert!(
            level_value(hazard.residual_risk) <= level_value(hazard.risk_level),
            "hazard {} residual risk ({:?}) must not exceed initial risk ({:?})",
            hazard.id,
            hazard.residual_risk,
            hazard.risk_level
        );
    }
}

// ===========================================================================
// Determinism guarantees tests
// ===========================================================================

#[test]
fn determinism_guarantees_exist() {
    let det = determinism::build_determinism_guarantees();
    assert!(!det.guarantees.is_empty(), "must have determinism guarantees");
}

#[test]
fn determinism_guarantee_ids_are_unique() {
    let det = determinism::build_determinism_guarantees();
    let mut ids = std::collections::HashSet::new();
    for g in &det.guarantees {
        assert!(ids.insert(&g.id), "duplicate guarantee ID: {}", g.id);
    }
}

#[test]
fn determinism_guarantees_cover_rendering_pipeline() {
    let det = determinism::build_determinism_guarantees();
    let components: Vec<&str> = det.guarantees.iter().map(|g| g.component.as_str()).collect();
    // Must cover codec, window/level, and measurement determinism
    let has_codec = components.iter().any(|c| c.contains("codec"));
    let has_window = components.iter().any(|c| c.contains("window/level") || c.contains("Window"));
    let has_measurement = components.iter().any(|c| c.contains("measurement"));
    assert!(has_codec, "must have codec determinism guarantee");
    assert!(has_window, "must have window/level determinism guarantee");
    assert!(has_measurement, "must have measurement determinism guarantee");
}

#[test]
fn determinism_every_guarantee_has_validation_method() {
    let det = determinism::build_determinism_guarantees();
    for g in &det.guarantees {
        assert!(!g.validation_method.is_empty(), "guarantee {} must have a validation method", g.id);
        assert!(!g.test_coverage.is_empty(), "guarantee {} must describe test coverage", g.id);
    }
}

// ===========================================================================
// Cross-document consistency tests
// ===========================================================================

#[test]
fn srs_design_elements_are_consistent_with_sdd() {
    let srs = srs::build_srs_from_manifests();
    let sdd = sdd::build_sdd_from_workspace();
    // Every design element's parent requirement must exist in the SRS
    let req_ids: std::collections::HashSet<&str> =
        srs.requirements.iter().map(|r| r.id.as_str()).collect();
    for de in &sdd.design_elements {
        assert!(
            req_ids.contains(de.parent_requirement.as_str()),
            "SDD design element {} references requirement {} not in SRS",
            de.id,
            de.parent_requirement
        );
    }
}

#[test]
fn complete_iec_62304_bundle_is_available() {
    // Verify all four IEC 62304 documents can be built
    let srs = srs::build_srs_from_manifests();
    let sdd = sdd::build_sdd_from_workspace();
    let stp = stp::build_stp_from_tests();
    let rmf = rmf::build_rmf_from_issues();
    let det = determinism::build_determinism_guarantees();

    // Bundle must be non-empty and self-consistent
    assert!(!srs.requirements.is_empty());
    assert!(!sdd.architecture.crates.is_empty());
    assert!(!stp.test_suites.is_empty());
    assert!(!rmf.hazards.is_empty());
    assert!(!det.guarantees.is_empty());

    // Version consistency
    assert_eq!(srs.version, "0.1.0");
    assert_eq!(sdd.version, "0.1.0");
    assert_eq!(stp.version, "0.1.0");
}
