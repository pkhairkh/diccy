//! RBAC authorization module with configurable role-to-permission mapping.
//!
//! Provides [`Role`], [`Permission`], [`RolePermissionMap`], [`RbacPolicy`],
//! and study-level access control via [`StudyAccessList`]. Configuration is
//! supported via TOML and JSON policy strings.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use dicom_core::{Error, ErrorKind, Result};

// ===========================================================================
// Role enum
// ===========================================================================

/// Clinical and administrative roles for RBAC authorization.
///
/// Each role maps to a defined set of [`Permission`] values via
/// [`RolePermissionMap`]. Roles are extracted from `AuthSubject` at
/// authorization time by a configurable `role_extractor` callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    /// Radiologist — clinical interpretation and reporting.
    Radiologist,
    /// Technologist — image acquisition and quality control.
    Technologist,
    /// Referring physician — study ordering and review.
    ReferringPhysician,
    /// Administrator — system configuration and user management.
    Administrator,
    /// Researcher — de-identified data access for research.
    Researcher,
}

impl Role {
    /// Return the string name of the role.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Radiologist => "Radiologist",
            Role::Technologist => "Technologist",
            Role::ReferringPhysician => "ReferringPhysician",
            Role::Administrator => "Administrator",
            Role::Researcher => "Researcher",
        }
    }

    /// Return all defined role variants in declaration order.
    pub fn all() -> &'static [Role] {
        &[
            Role::Radiologist,
            Role::Technologist,
            Role::ReferringPhysician,
            Role::Administrator,
            Role::Researcher,
        ]
    }
}

impl FromStr for Role {
    type Err = Box<Error>;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Radiologist" => Ok(Role::Radiologist),
            "Technologist" => Ok(Role::Technologist),
            "ReferringPhysician" => Ok(Role::ReferringPhysician),
            "Administrator" => Ok(Role::Administrator),
            "Researcher" => Ok(Role::Researcher),
            _ => Err(policy_violation(
                "rbac_role",
                format!("unknown role: {s}"),
            )),
        }
    }
}

// ===========================================================================
// Permission enum
// ===========================================================================

/// Permissions for RBAC authorization.
///
/// Each permission represents a class of operations. The RBAC policy maps
/// roles to permissions; the `RbacAuthorizer` maps `AuthAction`/`AuthResourceKey`
/// pairs to the required permission before checking the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read/view study data (query, retrieve, echo, web read).
    ReadStudy,
    /// Write/create reports and clinical annotations.
    WriteReport,
    /// Delete study data.
    DeleteStudy,
    /// Export data (clipboard, removable media, download).
    ExportData,
    /// Administrative configuration changes.
    AdminConfig,
    /// Break-glass emergency access override.
    BreakGlass,
}

impl Permission {
    /// Return the string name of the permission.
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::ReadStudy => "ReadStudy",
            Permission::WriteReport => "WriteReport",
            Permission::DeleteStudy => "DeleteStudy",
            Permission::ExportData => "ExportData",
            Permission::AdminConfig => "AdminConfig",
            Permission::BreakGlass => "BreakGlass",
        }
    }

    /// Return all defined permission variants in declaration order.
    pub fn all() -> &'static [Permission] {
        &[
            Permission::ReadStudy,
            Permission::WriteReport,
            Permission::DeleteStudy,
            Permission::ExportData,
            Permission::AdminConfig,
            Permission::BreakGlass,
        ]
    }
}

impl FromStr for Permission {
    type Err = Box<Error>;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "ReadStudy" => Ok(Permission::ReadStudy),
            "WriteReport" => Ok(Permission::WriteReport),
            "DeleteStudy" => Ok(Permission::DeleteStudy),
            "ExportData" => Ok(Permission::ExportData),
            "AdminConfig" => Ok(Permission::AdminConfig),
            "BreakGlass" => Ok(Permission::BreakGlass),
            _ => Err(policy_violation(
                "rbac_permission",
                format!("unknown permission: {s}"),
            )),
        }
    }
}

// ===========================================================================
// RolePermissionMap
// ===========================================================================

/// Mapping from [`Role`] to granted [`Permission`] sets.
///
/// Provides default mappings that follow the principle of least privilege:
/// - **Radiologist**: ReadStudy + WriteReport + ExportData
/// - **Technologist**: ReadStudy + ExportData
/// - **ReferringPhysician**: ReadStudy
/// - **Administrator**: all permissions
/// - **Researcher**: ReadStudy + ExportData
#[derive(Debug, Clone)]
pub struct RolePermissionMap {
    /// Inner mapping from role to permission set.
    inner: HashMap<Role, HashSet<Permission>>,
}

impl Default for RolePermissionMap {
    fn default() -> Self {
        Self::new()
    }
}

impl RolePermissionMap {
    /// Create a new role-permission map with default mappings.
    pub fn new() -> Self {
        let mut inner = HashMap::new();

        // Radiologist: read studies, write reports, export data
        let radiologist_perms: HashSet<Permission> = [
            Permission::ReadStudy,
            Permission::WriteReport,
            Permission::ExportData,
        ]
        .into_iter()
        .collect();
        inner.insert(Role::Radiologist, radiologist_perms);

        // Technologist: read studies, export data
        let technologist_perms: HashSet<Permission> =
            [Permission::ReadStudy, Permission::ExportData]
                .into_iter()
                .collect();
        inner.insert(Role::Technologist, technologist_perms);

        // ReferringPhysician: read studies only
        let referring_perms: HashSet<Permission> = [Permission::ReadStudy].into_iter().collect();
        inner.insert(Role::ReferringPhysician, referring_perms);

        // Administrator: all permissions
        let admin_perms: HashSet<Permission> = Permission::all().iter().copied().collect();
        inner.insert(Role::Administrator, admin_perms);

        // Researcher: read studies, export data
        let researcher_perms: HashSet<Permission> =
            [Permission::ReadStudy, Permission::ExportData]
                .into_iter()
                .collect();
        inner.insert(Role::Researcher, researcher_perms);

        Self { inner }
    }

    /// Create an empty role-permission map with no mappings.
    pub fn empty() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Get the permissions for a role.
    pub fn permissions_for(&self, role: Role) -> Option<&HashSet<Permission>> {
        self.inner.get(&role)
    }

    /// Check if a role has a specific permission.
    pub fn has_permission(&self, role: Role, permission: Permission) -> bool {
        self.inner
            .get(&role)
            .map_or(false, |perms| perms.contains(&permission))
    }

    /// Grant a permission to a role.
    pub fn grant(&mut self, role: Role, permission: Permission) {
        self.inner.entry(role).or_default().insert(permission);
    }

    /// Revoke a permission from a role.
    pub fn revoke(&mut self, role: Role, permission: Permission) {
        if let Some(perms) = self.inner.get_mut(&role) {
            perms.remove(&permission);
        }
    }

    /// Return the number of roles in the map.
    pub fn role_count(&self) -> usize {
        self.inner.len()
    }
}

// ===========================================================================
// StudyAccessList
// ===========================================================================

/// Study-level access control list.
///
/// Determines which study UIDs a role can access. If no entry exists for a
/// role (or the entry's set is empty), the role can access all studies
/// (no restriction). When a non-empty set is present, only the listed
/// study UIDs are accessible.
#[derive(Debug, Clone, Default)]
pub struct StudyAccessList {
    /// Mapping from role to allowed study UIDs. Empty or absent = all studies.
    inner: HashMap<Role, HashSet<String>>,
}

impl StudyAccessList {
    /// Create a new empty study access list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Grant access to a study for a role.
    pub fn grant_study(&mut self, role: Role, study_uid: String) {
        self.inner.entry(role).or_default().insert(study_uid);
    }

    /// Revoke access to a study for a role.
    pub fn revoke_study(&mut self, role: Role, study_uid: &str) {
        if let Some(studies) = self.inner.get_mut(&role) {
            studies.remove(study_uid);
        }
    }

    /// Check if a role can access a specific study.
    ///
    /// Returns `true` if:
    /// - The role has no entry in the access list (unrestricted), or
    /// - The study UID is in the role's allowed set.
    ///
    /// Returns `false` if:
    /// - The role has an entry but the set is empty (no access — fail-closed), or
    /// - The study UID is not in the role's allowed set.
    pub fn can_access_study(&self, role: Role, study_uid: Option<&str>) -> bool {
        match (self.inner.get(&role), study_uid) {
            (None, _) => true,
            (Some(_studies), None) => false,
            (Some(studies), Some(uid)) => studies.contains(uid),
        }
    }
}

// ===========================================================================
// RbacPolicy
// ===========================================================================

/// Combined RBAC policy with role-permission mapping and study-level access control.
///
/// Use [`RbacPolicy::from_toml`] or [`RbacPolicy::from_json`] to load
/// configuration from a policy file string.
#[derive(Debug, Clone)]
pub struct RbacPolicy {
    /// Role-to-permission mapping.
    pub permission_map: RolePermissionMap,
    /// Study-level access control.
    pub study_access: StudyAccessList,
}

impl Default for RbacPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl RbacPolicy {
    /// Create a new RBAC policy with default mappings and no study restrictions.
    pub fn new() -> Self {
        Self {
            permission_map: RolePermissionMap::new(),
            study_access: StudyAccessList::new(),
        }
    }

    /// Check if a role has a specific permission.
    pub fn has_permission(&self, role: Role, permission: Permission) -> bool {
        self.permission_map.has_permission(role, permission)
    }

    /// Check if a role can access a specific study.
    pub fn can_access_study(&self, role: Role, study_uid: Option<&str>) -> bool {
        self.study_access.can_access_study(role, study_uid)
    }

    /// Parse an RBAC policy from a TOML configuration string.
    ///
    /// Expected format:
    /// ```toml
    /// [roles.Radiologist]
    /// permissions = ["ReadStudy", "WriteReport", "ExportData"]
    ///
    /// [study_access.Radiologist]
    /// studies = ["1.2.3.4", "5.6.7.8"]
    /// ```
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        let mut policy = Self {
            permission_map: RolePermissionMap::empty(),
            study_access: StudyAccessList::new(),
        };
        let mut current_section: Option<String> = None;

        for line in toml_str.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Section header [roles.Radiologist] or [study_access.Radiologist]
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let section = trimmed[1..trimmed.len() - 1].trim();
                current_section = Some(section.to_string());
                continue;
            }

            // Key = value pair
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();

                if let Some(section) = &current_section {
                    if let Some(role_name) = section.strip_prefix("roles.") {
                        if let Ok(role) = Role::from_str(role_name) {
                            if key == "permissions" {
                                for perm in parse_permission_list(value) {
                                    policy.permission_map.grant(role, perm);
                                }
                            }
                        }
                    } else if let Some(role_name) = section.strip_prefix("study_access.") {
                        if let Ok(role) = Role::from_str(role_name) {
                            if key == "studies" {
                                for study in parse_string_list(value) {
                                    policy.study_access.grant_study(role, study);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(policy)
    }

    /// Parse an RBAC policy from a JSON configuration string.
    ///
    /// Expected format:
    /// ```json
    /// {
    ///   "roles": {
    ///     "Radiologist": ["ReadStudy", "WriteReport", "ExportData"]
    ///   },
    ///   "study_access": {
    ///     "Radiologist": ["1.2.3.4"]
    ///   }
    /// }
    /// ```
    pub fn from_json(json_str: &str) -> Result<Self> {
        let mut policy = Self {
            permission_map: RolePermissionMap::empty(),
            study_access: StudyAccessList::new(),
        };
        let json = json_str.trim();

        // Parse "roles" section
        if let Some(roles_start) = json.find("\"roles\"") {
            if let Some(brace_offset) = json[roles_start..].find('{') {
                let brace_start = roles_start + brace_offset;
                if let Some(brace_end) = find_matching_brace(json, brace_start) {
                    let roles_section = &json[brace_start + 1..brace_end];
                    parse_json_role_mappings(roles_section, &mut policy);
                }
            }
        }

        // Parse "study_access" section
        if let Some(access_start) = json.find("\"study_access\"") {
            if let Some(brace_offset) = json[access_start..].find('{') {
                let brace_start = access_start + brace_offset;
                if let Some(brace_end) = find_matching_brace(json, brace_start) {
                    let access_section = &json[brace_start + 1..brace_end];
                    parse_json_study_access(access_section, &mut policy);
                }
            }
        }

        Ok(policy)
    }
}

// ===========================================================================
// Permission mapping from AuthAction/AuthResourceKey
// ===========================================================================

use crate::{AuthAction, AuthResourceKey};

/// Determine the [`Permission`] required for a given action/resource pair.
///
/// This mapping bridges the protocol-level `AuthAction`/`AuthResourceKey`
/// types with the policy-level `Permission` enum used by `RbacPolicy`.
///
/// Returns `None` for unrecognized action/resource combinations, which
/// will result in a deny decision (fail-closed).
pub fn permission_for_action(action: AuthAction, _resource: AuthResourceKey) -> Option<Permission> {
    match action {
        // Read operations → ReadStudy
        AuthAction::Query | AuthAction::Retrieve | AuthAction::Echo | AuthAction::WebRequest => {
            Some(Permission::ReadStudy)
        }

        // Write/report operations → WriteReport
        AuthAction::Store
        | AuthAction::ViewerMeasurementWrite
        | AuthAction::ViewerSegmentationWrite
        | AuthAction::ViewerOverlayWrite
        | AuthAction::ViewerAnnotationWrite => Some(Permission::WriteReport),

        // Delete operations → DeleteStudy
        AuthAction::Delete => Some(Permission::DeleteStudy),

        // Protocol/admin operations → AdminConfig
        AuthAction::Associate
        | AuthAction::Command
        | AuthAction::StorageCommitment
        | AuthAction::Ups
        | AuthAction::Ian => Some(Permission::AdminConfig),
    }
}

// ===========================================================================
// Internal helpers
// ===========================================================================

fn parse_permission_list(value: &str) -> Vec<Permission> {
    let inner = value.trim().trim_start_matches('[').trim_end_matches(']');
    let mut perms = Vec::new();
    for item in inner.split(',') {
        let trimmed = item.trim().trim_matches('"').trim();
        if !trimmed.is_empty() {
            if let Ok(p) = Permission::from_str(trimmed) {
                perms.push(p);
            }
        }
    }
    perms
}

fn parse_string_list(value: &str) -> Vec<String> {
    let inner = value.trim().trim_start_matches('[').trim_end_matches(']');
    let mut items = Vec::new();
    for item in inner.split(',') {
        let trimmed = item.trim().trim_matches('"').trim();
        if !trimmed.is_empty() {
            items.push(trimmed.to_string());
        }
    }
    items
}

fn find_matching_brace(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if start >= bytes.len() || bytes[start] != b'{' {
        return None;
    }
    let mut depth = 0i32;
    for (i, &b) in bytes[start..].iter().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_json_role_mappings(section: &str, policy: &mut RbacPolicy) {
    let mut pos = 0;
    let bytes = section.as_bytes();
    while pos < bytes.len() {
        if bytes[pos] == b'"' {
            let role_start = pos + 1;
            if let Some(role_end_offset) = section[role_start..].find('"') {
                let role_name = &section[role_start..role_start + role_end_offset];
                if let Ok(role) = Role::from_str(role_name) {
                    let after_role = role_start + role_end_offset + 1;
                    if let Some(colon_offset) = section[after_role..].find(':') {
                        let after_colon = after_role + colon_offset + 1;
                        if let Some(bracket_offset) = section[after_colon..].find('[') {
                            let abs_bracket = after_colon + bracket_offset;
                            if let Some(bracket_end_offset) = section[abs_bracket..].find(']') {
                                let array_content =
                                    &section[abs_bracket + 1..abs_bracket + bracket_end_offset];
                                for perm in parse_permission_list(array_content) {
                                    policy.permission_map.grant(role, perm);
                                }
                            }
                        }
                    }
                    pos = role_start + role_end_offset;
                    continue;
                }
            }
        }
        pos += 1;
    }
}

fn parse_json_study_access(section: &str, policy: &mut RbacPolicy) {
    let mut pos = 0;
    let bytes = section.as_bytes();
    while pos < bytes.len() {
        if bytes[pos] == b'"' {
            let role_start = pos + 1;
            if let Some(role_end_offset) = section[role_start..].find('"') {
                let role_name = &section[role_start..role_start + role_end_offset];
                if let Ok(role) = Role::from_str(role_name) {
                    let after_role = role_start + role_end_offset + 1;
                    if let Some(colon_offset) = section[after_role..].find(':') {
                        let after_colon = after_role + colon_offset + 1;
                        if let Some(bracket_offset) = section[after_colon..].find('[') {
                            let abs_bracket = after_colon + bracket_offset;
                            if let Some(bracket_end_offset) = section[abs_bracket..].find(']') {
                                let array_content =
                                    &section[abs_bracket + 1..abs_bracket + bracket_end_offset];
                                for study in parse_string_list(array_content) {
                                    policy.study_access.grant_study(role, study);
                                }
                            }
                        }
                    }
                    pos = role_start + role_end_offset;
                    continue;
                }
            }
        }
        pos += 1;
    }
}

fn policy_violation(policy: impl Into<String>, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::PolicyViolation {
            policy: policy.into(),
            detail: detail.into(),
        },
        "policy violation",
    )
    .into()
}
