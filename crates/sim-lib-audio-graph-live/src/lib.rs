#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Preallocated live audio graph runner for host callback integration.
//!
//! This crate connects the audio graph processor protocol to host callback
//! queues. It provides bounded control/audio queues, stream-clock-backed
//! transport snapshots, and a small allocation-free steady-state process path
//! for mono and stereo graphs.

mod event;
mod profile;
mod queue;
mod runner;
mod runtime;
mod site;
mod transport;

pub use event::{LiveAudioEvent, LiveControlEvent, LiveQueuePush};
pub use profile::{
    LiveStreamLane, buffered_pcm_preview_profile, lan_buffered_audio_preview_profile,
    lan_render_return_profile, realtime_local_audio_profile,
    refuse_unbuffered_audio_callback_tunnel, validate_realtime_local_audio_profile,
};
pub use queue::{AudioToControlQueue, BoundedLiveQueue, ControlToAudioQueue};
pub use runner::{LiveGraphConfig, LiveGraphRunner, LiveProcessReport, LiveSteadyStateSnapshot};
pub use runtime::{AudioGraphLiveLib, install_audio_graph_live_lib};
pub use site::{LivePlacedNode, LivePlacementSite, LivePlacementSnapshot};
pub use transport::{
    LanBufferedPreviewWindow, LiveTransportClock, lan_buffered_preview_drop_diagnostic_kind,
    lan_buffered_preview_jitter_diagnostic_kind, lan_buffered_preview_late_packet_diagnostic_kind,
    lan_buffered_preview_reorder_diagnostic_kind, live_clock_symbol,
    validate_lan_buffered_preview_envelope,
};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod site_tests;
#[cfg(test)]
mod tests;
