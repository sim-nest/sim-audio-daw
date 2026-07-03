#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Modeled Windows ASIO stream-host adapter (in-process simulation).
//!
//! Modeled tier, no native I/O. This crate is a pure-Rust, in-process MODEL of
//! an ASIO backend, not a binding to the Steinberg ASIO SDK. The workspace
//! forbids `unsafe` and the crate carries no `-sys`/FFI dependency, so it
//! performs no real ASIO driver I/O -- it serves deterministic, fake driver
//! enumeration. The modeled tier is flagged by the default-on `model` feature;
//! a native provider would live behind a separate FFI binding outside this repo.
//!
//! This crate keeps validation independent of the Steinberg ASIO SDK and local
//! audio drivers. It models provider-reported ASIO drivers, exposes
//! stream-host inventory and open plans, and bridges ASIO-style process
//! callbacks into `ProcessBlock` so the same graph processor code can run
//! under fake Linux CI, Windows ASIO, and other host adapters.
//!
//! Native ASIO providers are intentionally optional. A downstream provider must
//! target Windows, arrange SDK headers/import libraries outside this
//! repository, and populate the stable model types from driver enumeration.

mod backend;
mod bridge;
mod model;
mod runtime;

pub use backend::{AsioBackend, asio_backend_symbol, asio_clock_symbol, asio_transport_symbol};
pub use bridge::AsioBufferSwitchBridge;
pub use model::{AsioDriver, AsioPort, AsioTiming, asio_sdk_build_requirements};
pub use runtime::{AsioLib, install_stream_asio_lib};

#[cfg(test)]
mod tests;
