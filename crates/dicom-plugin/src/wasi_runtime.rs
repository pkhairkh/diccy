//! WebAssembly (WASM) plugin execution runtime.
//!
//! This module provides a sandboxed runtime for plugins compiled to
//! WASM. In production, this would use an engine like `wasmtime` or
//! `wasmer`; for now, the implementation validates the WASM binary
//! header and provides the trait interface as a stub that can be
//! filled in when a WASM runtime dependency is added.

use crate::manifest::PluginManifest;
use crate::r#trait::PluginError;

// ---------------------------------------------------------------------------
// WASM magic bytes and version
// ---------------------------------------------------------------------------

/// The WASM binary magic number: `\0asm`.
const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// The WASM binary format version (1).
const WASM_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

// ---------------------------------------------------------------------------
// WasiPluginRuntime
// ---------------------------------------------------------------------------

/// WASM-based plugin execution runtime (sandboxed).
///
/// This runtime loads and validates WASM modules that implement the
/// DiCCY plugin interface. The actual execution engine is stubbed;
/// when a production WASM runtime is integrated, the stubs will be
/// replaced with real WASI calls.
pub struct WasiPluginRuntime {
    /// The manifest describing the plugin.
    manifest: Option<PluginManifest>,
    /// The validated WASM binary (stored for future execution).
    wasm_bytes: Option<Vec<u8>>,
    /// Whether the runtime has been initialised (on_load called).
    loaded: bool,
}

impl Default for WasiPluginRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WasiPluginRuntime {
    /// Create a new, empty WASM runtime.
    pub fn new() -> Self {
        Self {
            manifest: None,
            wasm_bytes: None,
            loaded: false,
        }
    }

    /// Load and validate a WASM binary.
    ///
    /// The binary is checked for the correct WASM magic header and
    /// version number. If validation passes, the bytes are stored for
    /// later execution.
    pub fn load_wasm(&mut self, wasm_bytes: &[u8]) -> Result<(), PluginError> {
        validate_wasm_binary(wasm_bytes)?;
        self.wasm_bytes = Some(wasm_bytes.to_vec());
        Ok(())
    }

    /// Attach a manifest to this runtime.
    ///
    /// The manifest must be set before calling [`call_on_load`].
    pub fn set_manifest(&mut self, manifest: PluginManifest) {
        self.manifest = Some(manifest);
    }

    /// Return a reference to the attached manifest, if any.
    pub fn manifest(&self) -> Option<&PluginManifest> {
        self.manifest.as_ref()
    }

    /// Return whether a WASM binary has been loaded.
    pub fn has_wasm(&self) -> bool {
        self.wasm_bytes.is_some()
    }

    /// Return whether the plugin has been initialised.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Call the plugin's `on_load` entry point.
    ///
    /// In production, this would invoke the WASM module's `_diccy_on_load`
    /// exported function. For now, it marks the runtime as loaded.
    pub fn call_on_load(&mut self) -> Result<(), PluginError> {
        if self.wasm_bytes.is_none() {
            return Err(PluginError::LoadFailed {
                plugin_name: self
                    .manifest
                    .as_ref()
                    .map(|m| m.plugin.name.clone())
                    .unwrap_or_default(),
                reason: "no WASM binary loaded".to_string(),
            });
        }
        self.loaded = true;
        Ok(())
    }

    /// Call the plugin's `on_unload` entry point.
    ///
    /// In production, this would invoke the WASM module's
    /// `_diccy_on_unload` exported function.
    pub fn call_on_unload(&mut self) -> Result<(), PluginError> {
        if !self.loaded {
            return Err(PluginError::UnloadFailed {
                plugin_name: self
                    .manifest
                    .as_ref()
                    .map(|m| m.plugin.name.clone())
                    .unwrap_or_default(),
                reason: "plugin is not loaded".to_string(),
            });
        }
        self.loaded = false;
        Ok(())
    }

    /// Return the size of the loaded WASM binary in bytes.
    pub fn wasm_size(&self) -> usize {
        self.wasm_bytes.as_ref().map_or(0, |b| b.len())
    }
}

// ---------------------------------------------------------------------------
// WASM binary validation
// ---------------------------------------------------------------------------

/// Validate that a byte slice starts with a valid WASM header.
///
/// A valid WASM binary begins with the 4-byte magic number `\0asm`
/// followed by the 4-byte version number `0x01 0x00 0x00 0x00`.
pub fn validate_wasm_binary(bytes: &[u8]) -> Result<(), PluginError> {
    if bytes.len() < 8 {
        return Err(PluginError::InvalidWasmBinary {
            reason: format!(
                "binary too short ({} bytes, need at least 8 for WASM header)",
                bytes.len()
            ),
        });
    }

    let magic: [u8; 4] = bytes[0..4].try_into().unwrap();
    if magic != WASM_MAGIC {
        return Err(PluginError::InvalidWasmBinary {
            reason: format!(
                "invalid magic bytes: expected {:02X?}, got {:02X?}",
                WASM_MAGIC, magic
            ),
        });
    }

    let version: [u8; 4] = bytes[4..8].try_into().unwrap();
    if version != WASM_VERSION {
        return Err(PluginError::InvalidWasmBinary {
            reason: format!(
                "unsupported WASM version: expected {:02X?}, got {:02X?}",
                WASM_VERSION, version
            ),
        });
    }

    Ok(())
}

/// Return `true` if the given bytes start with the WASM magic header.
pub fn is_wasm_binary(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0..4] == WASM_MAGIC
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    #[test]
    fn validate_correct_wasm_header() {
        let mut bytes = vec![0x00, 0x61, 0x73, 0x6D]; // magic
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // version
        assert!(validate_wasm_binary(&bytes).is_ok());
    }

    #[test]
    fn reject_too_short() {
        assert!(validate_wasm_binary(&[0x00, 0x61]).is_err());
    }

    #[test]
    fn reject_bad_magic() {
        let mut bytes = vec![0x00, 0x62, 0x73, 0x6D]; // wrong magic
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        assert!(validate_wasm_binary(&bytes).is_err());
    }

    #[test]
    fn reject_bad_version() {
        let mut bytes = vec![0x00, 0x61, 0x73, 0x6D]; // correct magic
        bytes.extend_from_slice(&[0x02, 0x00, 0x00, 0x00]); // wrong version
        assert!(validate_wasm_binary(&bytes).is_err());
    }

    #[test]
    fn is_wasm_binary_check() {
        let good: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        assert!(is_wasm_binary(&good));
        let bad: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic
        assert!(!is_wasm_binary(&bad));
    }

    #[test]
    fn runtime_load_unload_cycle() {
        let mut rt = WasiPluginRuntime::new();
        let wasm: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        rt.load_wasm(&wasm).unwrap();
        assert!(rt.has_wasm());
        assert!(!rt.is_loaded());

        rt.call_on_load().unwrap();
        assert!(rt.is_loaded());

        rt.call_on_unload().unwrap();
        assert!(!rt.is_loaded());
    }

    #[test]
    fn runtime_rejects_load_without_wasm() {
        let mut rt = WasiPluginRuntime::new();
        assert!(rt.call_on_load().is_err());
    }

    #[test]
    fn runtime_rejects_unload_without_load() {
        let mut rt = WasiPluginRuntime::new();
        let wasm: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        rt.load_wasm(&wasm).unwrap();
        assert!(rt.call_on_unload().is_err());
    }
}
