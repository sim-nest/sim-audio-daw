#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Modeled macOS CoreAudio stream-host adapter (in-process simulation).
//!
//! Modeled tier, no native I/O. This crate is a pure-Rust, in-process MODEL of
//! a CoreAudio backend, not a binding to Apple's AudioToolbox/CoreAudio
//! frameworks. The workspace forbids `unsafe` and the crate carries no
//! `-sys`/FFI dependency, so it performs no real Apple framework I/O -- it
//! serves deterministic fake PCM devices. The modeled tier is flagged by the
//! default-on `model` feature; a native provider would live behind a separate
//! FFI binding outside this repo.
//!
//! The simple macOS path uses PortAudio or RtAudio over CoreAudio. This crate
//! exists for native CoreAudio coverage when that portable path is insufficient,
//! while keeping workspace validation independent of Apple frameworks and
//! hardware. MIDI remains separate: RtMidi is the macOS MIDI path, and this
//! crate intentionally models only PCM devices.

mod backend;
mod bridge;
mod model;
mod runtime;

pub use backend::{
    CoreAudioBackend, coreaudio_audio_backend_candidate, coreaudio_backend_symbol,
    coreaudio_clock_symbol, coreaudio_transport_symbol,
};
pub use bridge::CoreAudioRenderBridge;
pub use model::{
    CoreAudioDevice, CoreAudioTiming, macos_audio_backend_priority, macos_midi_backend_priority,
};
pub use runtime::{CoreAudioLib, install_stream_coreaudio_lib};

#[cfg(test)]
mod tests;
