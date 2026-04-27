//! Plugin sandboxing and capability enforcement.
//!
//! The sandbox ensures that plugins can only exercise the extension points
//! and permissions they declared in their manifest. Any attempt to use an
//! undeclared capability is rejected at runtime (fail-closed).

use std::collections::HashSet;

use crate::manifest::PluginPermissions;
use crate::r#trait::PluginError;

// ---------------------------------------------------------------------------
// Sandbox
// ---------------------------------------------------------------------------

/// Enforces plugin capability restrictions.
///
/// A `PluginSandbox` is created for each loaded plugin from the
/// permissions declared in the plugin's manifest. Every call from the
/// plugin into the runtime must pass through the sandbox, which checks
/// that the operation is within the declared capability set.
pub struct PluginSandbox {
    /// Extension points the plugin is allowed to register.
    allowed_extension_points: HashSet<String>,
    /// Resource limits for the plugin.
    resource_limits: ResourceLimits,
    /// Whether the plugin may access the network.
    network_access: bool,
    /// Whether the plugin may read from the filesystem.
    filesystem_read: bool,
    /// Whether the plugin may write to the filesystem.
    filesystem_write: bool,
    /// Name of the plugin this sandbox guards (for error messages).
    plugin_name: String,
}

impl PluginSandbox {
    /// Create a sandbox from declared permissions.
    pub fn from_permissions(plugin_name: &str, permissions: &PluginPermissions) -> Self {
        Self {
            allowed_extension_points: permissions
                .extension_points
                .iter()
                .cloned()
                .collect::<HashSet<_>>(),
            resource_limits: ResourceLimits {
                max_memory_bytes: permissions.max_memory_mb * 1024 * 1024,
                max_cpu_time_ms: permissions.max_cpu_time_ms,
                allowed_network_hosts: Vec::new(),
            },
            network_access: permissions.network_access,
            filesystem_read: permissions.filesystem_read,
            filesystem_write: permissions.filesystem_write,
            plugin_name: plugin_name.to_string(),
        }
    }

    /// Create a permissive sandbox that allows everything (for testing).
    pub fn permissive(plugin_name: &str) -> Self {
        Self {
            allowed_extension_points: [
                "ViewerTool".to_string(),
                "ImageProcessor".to_string(),
                "WorkflowHook".to_string(),
                "StorageBackend".to_string(),
            ]
            .into_iter()
            .collect(),
            resource_limits: ResourceLimits {
                max_memory_bytes: 1024 * 1024 * 1024, // 1 GiB
                max_cpu_time_ms: 60_000,              // 60 s
                allowed_network_hosts: vec!["*".to_string()],
            },
            network_access: true,
            filesystem_read: true,
            filesystem_write: true,
            plugin_name: plugin_name.to_string(),
        }
    }

    /// Check whether the plugin may register the given extension point.
    ///
    /// Returns `Ok(())` if access is allowed, or a [`PluginError::PermissionDenied`]
    /// error if the extension point was not declared in the manifest.
    pub fn check_extension_point(&self, extension_point: &str) -> Result<(), PluginError> {
        if self.allowed_extension_points.contains(extension_point) {
            Ok(())
        } else {
            Err(PluginError::PermissionDenied {
                required: format!("extension_point:{extension_point}"),
                detail: format!(
                    "plugin '{}' did not declare extension point '{extension_point}' in its manifest",
                    self.plugin_name
                ),
            })
        }
    }

    /// Check whether the plugin may access the network.
    pub fn check_network_access(&self) -> Result<(), PluginError> {
        if self.network_access {
            Ok(())
        } else {
            Err(PluginError::PermissionDenied {
                required: "network_access".to_string(),
                detail: format!(
                    "plugin '{}' did not declare network_access in its manifest",
                    self.plugin_name
                ),
            })
        }
    }

    /// Check whether the plugin may read the filesystem.
    pub fn check_filesystem_read(&self) -> Result<(), PluginError> {
        if self.filesystem_read {
            Ok(())
        } else {
            Err(PluginError::PermissionDenied {
                required: "filesystem_read".to_string(),
                detail: format!(
                    "plugin '{}' did not declare filesystem_read in its manifest",
                    self.plugin_name
                ),
            })
        }
    }

    /// Check whether the plugin may write the filesystem.
    pub fn check_filesystem_write(&self) -> Result<(), PluginError> {
        if self.filesystem_write {
            Ok(())
        } else {
            Err(PluginError::PermissionDenied {
                required: "filesystem_write".to_string(),
                detail: format!(
                    "plugin '{}' did not declare filesystem_write in its manifest",
                    self.plugin_name
                ),
            })
        }
    }

    /// Return a reference to the resource limits for this sandbox.
    pub fn resource_limits(&self) -> &ResourceLimits {
        &self.resource_limits
    }

    /// Return the set of allowed extension point names.
    pub fn allowed_extension_points(&self) -> &HashSet<String> {
        &self.allowed_extension_points
    }

    /// Return the plugin name this sandbox guards.
    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }
}

// ---------------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------------

/// Resource consumption limits for a sandboxed plugin.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory the plugin may allocate (bytes).
    pub max_memory_bytes: u64,
    /// Maximum CPU time per operation (ms).
    pub max_cpu_time_ms: u64,
    /// Network hosts the plugin is allowed to connect to.
    ///
    /// An empty list means no hosts are allowed (unless `network_access`
    /// is `false`, in which case this field is irrelevant).
    pub allowed_network_hosts: Vec<String>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64 MiB
            max_cpu_time_ms: 5000,
            allowed_network_hosts: Vec::new(),
        }
    }
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    #[test]
    fn sandbox_enforces_extension_point() {
        let permissions = PluginPermissions {
            extension_points: vec!["ViewerTool".to_string()],
            ..Default::default()
        };
        let sandbox = PluginSandbox::from_permissions("test.plugin", &permissions);
        assert!(sandbox.check_extension_point("ViewerTool").is_ok());
        assert!(sandbox.check_extension_point("ImageProcessor").is_err());
    }

    #[test]
    fn sandbox_enforces_network() {
        let permissions = PluginPermissions {
            network_access: false,
            ..Default::default()
        };
        let sandbox = PluginSandbox::from_permissions("test.plugin", &permissions);
        assert!(sandbox.check_network_access().is_err());

        let permissions = PluginPermissions {
            network_access: true,
            ..Default::default()
        };
        let sandbox = PluginSandbox::from_permissions("test.plugin", &permissions);
        assert!(sandbox.check_network_access().is_ok());
    }

    #[test]
    fn permissive_sandbox_allows_all() {
        let sandbox = PluginSandbox::permissive("test.plugin");
        assert!(sandbox.check_extension_point("ViewerTool").is_ok());
        assert!(sandbox.check_extension_point("ImageProcessor").is_ok());
        assert!(sandbox.check_extension_point("WorkflowHook").is_ok());
        assert!(sandbox.check_extension_point("StorageBackend").is_ok());
        assert!(sandbox.check_network_access().is_ok());
        assert!(sandbox.check_filesystem_read().is_ok());
        assert!(sandbox.check_filesystem_write().is_ok());
    }
}
