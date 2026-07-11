#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Common plugin descriptors, state, and graph adapters.
//!
//! ```rust
//! use sim_lib_plugin_core::{
//!     ParameterDescriptor, PluginDescriptor, PluginFormat, PluginState,
//! };
//!
//! let descriptor = PluginDescriptor::audio_effect(
//!     PluginFormat::Sim,
//!     "org.sim.doc-gain",
//!     "Doc Gain",
//!     2,
//! )
//! .unwrap()
//! .with_parameter(ParameterDescriptor::new(0, "gain", "Gain", 0.0, 2.0, 1.0).unwrap());
//!
//! assert_eq!(descriptor.parameter(0).unwrap().plain_to_normalized(1.0), 0.5);
//!
//! let mut state = PluginState::new();
//! state.set_param(0, 1.0);
//! let rebuilt = PluginState::from_expr(&state.to_expr()).unwrap();
//! assert_eq!(rebuilt.param(0), Some(1.0));
//! ```

mod adapter;
mod capability;
mod citizen;
mod descriptor;
mod runtime;
mod state;

pub use adapter::{HostedPluginProcessor, PluginInstance, ProcessorPlugin};
pub use capability::{AudioPluginCapability, CapabilitySet};
pub use citizen::{PluginDescriptorRecord, plugin_descriptor_class_symbol};
pub use descriptor::{
    ParameterDescriptor, ParameterKind, PluginDescriptor, PluginFormat, PluginId, PluginLoadSpec,
};
pub use runtime::{install_plugin_core_lib, plugin_core_symbols};
pub use state::PluginState;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
