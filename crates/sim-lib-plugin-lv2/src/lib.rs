#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! LV2-shaped plugin adapters for SIM audio graph processors.
//!
//! The default `model` feature is a pure-Rust, in-process model of the LV2
//! plugin format. The crate carries no `-sys` or FFI dependency:
//! [`Lv2HostProcessor`] runs SIM audio-graph processors shaped to the LV2
//! descriptor, and `export_*_as_lv2` produces descriptors. On Linux, the
//! optional `lv2-host` feature exposes a provider trait and fixture host
//! provider for capability-gated fallback loading.
//!
//! ```rust
//! use sim_lib_plugin_lv2::lv2_gain_lv2_descriptor;
//!
//! let lv2 = lv2_gain_lv2_descriptor().unwrap();
//! assert_eq!(lv2.ports.len(), 3);
//!
//! let descriptor = lv2.to_plugin_descriptor().unwrap();
//! assert_eq!(descriptor.id.format.as_str(), "lv2");
//! ```

mod adapter;
mod descriptor;
#[cfg(all(feature = "lv2-host", target_os = "linux"))]
pub mod native;
mod runtime;
mod state;

pub use adapter::{
    Lv2ExportedProcessor, Lv2HostProcessor, export_gain_as_lv2, export_processor_as_lv2,
};
pub use descriptor::{
    Lv2PluginDescriptor, Lv2Port, Lv2PortKind, lv2_gain_descriptor, lv2_gain_lv2_descriptor,
};
pub use runtime::{install_lv2_plugin_lib, lv2_plugin_symbols};
pub use state::Lv2StatePatch;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
