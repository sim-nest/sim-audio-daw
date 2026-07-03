//! Capability-checked WebAssembly plugin loading.

use sim_kernel::Result;
use sim_lib_plugin_core::{AudioPluginCapability, CapabilitySet};

use crate::{WasmPluginProcessor, WasmResourceLimits};

/// Loads a WebAssembly audio plugin after checking the wasm-plugin capability.
///
/// # Errors
///
/// Returns a capability error when `caps` does not include
/// [`AudioPluginCapability::WasmPlugin`]. Returns an eval error when the wasm
/// module is invalid or misses required exports.
pub fn load_wasm_plugin(
    caps: &CapabilitySet,
    wasm_bytes: &[u8],
    limits: WasmResourceLimits,
) -> Result<WasmPluginProcessor> {
    caps.require(AudioPluginCapability::WasmPlugin)?;
    WasmPluginProcessor::from_bytes_with_limits(wasm_bytes, limits)
}
