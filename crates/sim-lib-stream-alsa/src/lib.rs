#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Modeled ALSA stream-host adapter (in-process simulation).
//!
//! Modeled tier, no native I/O. This crate is a pure-Rust, in-process MODEL of
//! an ALSA backend, not a binding to `libasound`. The workspace forbids
//! `unsafe` and the crate carries no `-sys`/FFI dependency, so it performs no
//! real Linux PCM I/O -- it serves deterministic, provider-reported fake
//! devices. The modeled tier is flagged by the default-on `model` feature; a
//! native provider would live behind a separate FFI binding outside this repo.
//!
//! This crate keeps validation independent of an ALSA development package or
//! local sound hardware. It models provider-reported ALSA PCM devices, supports
//! `default`, `hw:*`, and `plughw:*` names, exposes host inventory/open plans,
//! bridges playback callbacks into `ProcessBlock`, and records capture buffers
//! as PCM stream packets. Native provider crates populate the same model from
//! `snd_pcm_*` enumeration. ALSA sequencer MIDI belongs in a MIDI-specific
//! adapter so this crate remains focused on PCM.

mod backend;
mod bridge;
mod model;
mod runtime;
mod site;

pub use backend::{
    AlsaBackend, alsa_audio_backend_candidate, alsa_backend_symbol, alsa_transport_symbol,
};
pub use bridge::{AlsaCaptureBridge, AlsaPlaybackBridge};
pub use model::{AlsaPcmDevice, AlsaPcmName, AlsaPcmNameKind};
pub use runtime::{AlsaLib, install_stream_alsa_lib};
pub use site::default_modeled_alsa_site;

#[cfg(test)]
mod tests;
