#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! VST3-shaped plugin export adapters for SIM audio graph processors.
//!
//! Modeled tier, no native hosting. This crate is a pure-Rust, in-process MODEL
//! of the VST3 plugin format: the workspace forbids `unsafe` and the crate
//! carries no `-sys`/FFI dependency, so `export_*_as_vst3` produces descriptors,
//! not native `.vst3` binaries, and no plugin loading occurs. Native export and
//! hosting are deferred; `current_vst3_scope` names the blockers and SDK
//! requirements. The modeled tier is flagged by the default-on `model` feature.
//!
//! ```rust
//! use sim_lib_plugin_vst3::vst3_gain_vst3_descriptor;
//!
//! let vst3 = vst3_gain_vst3_descriptor().unwrap();
//! assert_eq!(vst3.buses.len(), 3);
//!
//! let descriptor = vst3.to_plugin_descriptor().unwrap();
//! assert_eq!(descriptor.id.format.as_str(), "vst3");
//! ```

mod adapter;
pub mod cookbook;
mod descriptor;
mod event;
mod runtime;
mod scope;

pub use adapter::{Vst3ExportedProcessor, export_gain_as_vst3, export_processor_as_vst3};
pub use cookbook::vst3_gain_demo;
pub use descriptor::{
    Vst3Bus, Vst3BusKind, Vst3ParamInfo, Vst3PluginDescriptor, vst3_gain_descriptor,
    vst3_gain_vst3_descriptor,
};
pub use event::{Vst3Event, Vst3EventBuffer, Vst3ParamMap};
pub use runtime::{Vst3PluginLib, install_vst3_plugin_lib, vst3_plugin_symbols};
pub use scope::{Vst3HostingDecision, Vst3ScopeDecision, current_vst3_scope};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
