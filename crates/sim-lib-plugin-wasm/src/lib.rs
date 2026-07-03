#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! WebAssembly audio plugin host for SIM audio graph processors.
//!
//! The default build contains the manifest data model. Enabling the
//! `wasm-plugin` feature adds a wasmtime-backed [`WasmPluginProcessor`] that
//! implements [`sim_lib_plugin_core::PluginInstance`].

pub mod abi;
pub mod limits;
pub mod loader;
pub mod native_fallback;
pub mod processor;
pub mod router;

pub use abi::WasmAudioManifest;
pub use limits::WasmResourceLimits;
pub use loader::load_wasm_plugin;
#[cfg(feature = "clap-host")]
pub use native_fallback::load_clap_plugin;
#[cfg(all(feature = "lv2-host", target_os = "linux"))]
pub use native_fallback::load_lv2_plugin;
pub use processor::WasmPluginProcessor;
pub use router::{PluginRouter, PluginRouterBuilder, RoutedPlugin};

#[cfg(test)]
mod tests;
