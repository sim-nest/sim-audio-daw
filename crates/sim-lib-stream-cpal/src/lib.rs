#![cfg_attr(not(feature = "cpal-hardware"), forbid(unsafe_code))]
#![deny(missing_docs)]
//! cpal stream-host adapter.
//!
//! The default `model` feature exposes a deterministic modeled audio site that
//! opens through the shared fake host backend, so validation stays independent
//! of local sound hardware. The `cpal-hardware` feature adds the native cpal
//! boundary in `native.rs`; that module owns the crate's unsafe callback slice
//! conversion and documents every unsafe block.

mod backend_resolution;
mod model;
mod provider_modeled;

#[cfg(feature = "cpal-hardware")]
mod native;

pub use backend_resolution::{
    BackendResolution, BackendResolutionRow, audio_backend_resolution_rows,
};
pub use model::{CpalModeledSite, cpal_modeled_backend_symbol, default_modeled_cpal_site};
pub use provider_modeled::{
    CpalProviderModeled, cpal_modeled_provider_entry, cpal_modeled_provider_symbol,
};

#[cfg(feature = "cpal-hardware")]
pub use native::{
    CpalDriver, CpalHardwareSite, config_from_cpal, cpal_hardware_backend_symbol,
    enumerate_cpal_hardware_sites, enumerate_cpal_sites,
};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
