#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Modeled JACK stream-host adapter (in-process simulation).
//!
//! Modeled tier, no native I/O. This crate is a pure-Rust, in-process MODEL of
//! a JACK backend, not a binding to `libjack`. The workspace forbids `unsafe`
//! and the crate carries no `-sys`/FFI dependency, so it performs no real JACK
//! server I/O -- it serves a deterministic fake client and ports. The modeled
//! tier is flagged by the default-on `model` feature; a native provider would
//! live behind a separate FFI binding outside this repo.
//!
//! The crate keeps validation independent of a running JACK server. It models
//! a JACK client, routable audio and MIDI ports, sample-frame transport, and
//! the callback bridge used to drive an audio graph. A future provider can
//! populate the same model from native JACK client and port registration.

mod backend;
mod bridge;
mod model;
mod runtime;
#[cfg(test)]
mod spike;

pub use backend::{JackBackend, jack_backend_symbol, jack_transport_symbol};
pub use bridge::{JackGraphBridge, JackMidiEvent};
pub use model::{JackClient, JackPort, JackTiming, JackTransportState, jack_clock_symbol};
pub use runtime::{JackLib, install_stream_jack_lib};

#[cfg(test)]
mod tests;
