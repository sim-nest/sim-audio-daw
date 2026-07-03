#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Modeled PortAudio stream-host adapter (in-process simulation).
//!
//! Modeled tier, no native I/O. This crate is a pure-Rust, in-process MODEL of
//! a PortAudio backend, not a binding to `libportaudio`. The workspace forbids
//! `unsafe` and the crate carries no `-sys`/FFI dependency, so it performs no
//! real PortAudio I/O -- it serves a deterministic fake default output. The
//! modeled tier is flagged by the default-on `model` feature; a native provider
//! would live behind a separate FFI binding outside this repo.
//!
//! This crate keeps validation independent of a PortAudio installation. It
//! models PortAudio devices as `sim-lib-stream-host` devices, provides a
//! deterministic fake default output, bridges host callbacks into
//! `ProcessBlock`, and documents the backend priority used by the bootstrap.
//! PortAudio is the portable fallback back-end behind native PipeWire support.

mod backend;
mod callback;
mod model;
mod runtime;
mod tone;

pub use backend::{PortAudioBackend, portaudio_backend_symbol, portaudio_transport_symbol};
pub use callback::{PortAudioCallbackBridge, PortAudioHostBuffer};
pub use model::{PortAudioDevice, portaudio_backend_priority};
pub use runtime::{PortAudioLib, install_stream_portaudio_lib};
pub use tone::{PortAudioTestTonePlan, test_tone_buffer};

#[cfg(test)]
mod tests;
