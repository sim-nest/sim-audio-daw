#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Modeled PipeWire stream-host adapter (in-process simulation).
//!
//! Modeled tier, no native I/O. This crate is a pure-Rust, in-process MODEL of
//! a PipeWire backend, not a binding to `libpipewire`. The workspace forbids
//! `unsafe` and the crate carries no `-sys`/FFI dependency, so it performs no
//! real PipeWire daemon I/O -- it serves deterministic, provider-reported fake
//! nodes and ports. The modeled tier is flagged by the default-on `model`
//! feature; a native provider would live behind a separate FFI binding outside
//! this repo.
//!
//! This crate keeps CI independent of a running PipeWire daemon. It models
//! provider-reported PipeWire nodes and visible SIM client ports, maps quantum,
//! sample-rate, and latency metadata into `HostStreamConfig`, and bridges fake
//! process callbacks into `ProcessBlock` and PCM callback queues. Native
//! provider crates populate this model from PipeWire registry events.

mod backend;
mod bridge;
mod model;
mod runtime;

pub use backend::{
    PipeWireBackend, pipewire_audio_backend_candidate, pipewire_backend_symbol,
    pipewire_transport_symbol,
};
pub use bridge::{PipeWireCaptureBridge, PipeWireGraphBridge};
pub use model::{PipeWireNode, PipeWirePort, PipeWireTiming, linux_audio_backend_priority};
pub use runtime::{PipeWireLib, install_stream_pipewire_lib};

#[cfg(test)]
mod tests;
