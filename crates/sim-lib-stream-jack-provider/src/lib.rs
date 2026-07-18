#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Loadable JACK audio placement provider.
//!
//! The default modeled lane registers a deterministic JACK-shaped audio site
//! without linking to JACK or opening host hardware. The `jack-hardware` feature
//! enables the native JACK module for Rust host-registered provider entries.

pub mod cookbook;
mod entry;
mod model;

#[cfg(feature = "jack-hardware")]
mod native;

pub use cookbook::{
    jack_hardware_smoke_demo, jack_loadable_modeled_provider_demo, jack_provider_load_demo,
};
pub use entry::jack_provider_entry;
pub use model::{
    default_modeled_jack_site, enumerate_jack_sites, jack_backend_symbol, jack_modeled_site_symbol,
    jack_provider_symbol, JackProviderModeled,
};

#[cfg(feature = "jack-hardware")]
pub use native::{enumerate_jack_hardware_sites, JackDriver, JackHardwareSite};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
