#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Modeled CLAP-shaped plugin adapters for SIM audio graph processors.
//!
//! The default `model` feature is a pure-Rust, in-process model of the CLAP
//! plugin format. The crate carries no `-sys` or FFI dependency:
//! [`ClapHostProcessor`] runs SIM audio-graph processors shaped to the CLAP
//! descriptor, and `export_*_as_clap` produces descriptors, not native binaries.
//! The optional `clap-host` feature exposes a provider trait and fixture host
//! provider for capability-gated fallback loading without adding an SDK binding.
//!
//! ```rust
//! use sim_lib_plugin_clap::clap_gain_descriptor;
//!
//! let descriptor = clap_gain_descriptor().unwrap();
//! assert_eq!(descriptor.id.format.as_str(), "clap");
//! assert_eq!(descriptor.parameter(0).unwrap().stable_id.as_str(), "gain");
//! ```

mod adapter;
mod descriptor;
mod event;
#[cfg(feature = "clap-host")]
pub mod native;
mod runtime;

pub use adapter::{
    ClapExportedProcessor, ClapHostProcessor, export_gain_as_clap, export_processor_as_clap,
};
pub use descriptor::{clap_audio_effect_descriptor, clap_gain_descriptor, clap_synth_descriptor};
pub use event::{ClapEvent, ClapEventBuffer, ClapParamMap};
pub use runtime::{ClapPluginLib, clap_plugin_symbols, install_clap_plugin_lib};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
