//! Modeled cpal audio site registration helpers.
#![forbid(unsafe_code)]

use std::sync::Arc;

use sim_kernel::{Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    AudioDeviceCard, AudioSite, AudioSiteKey, FakeBackend, HostDirection, HostOpenStream,
    HostStreamConfigRequest, ModeledAudioSite, fake_backend_symbol,
};

/// Returns the modeled backend identity symbol for cpal validation.
pub fn cpal_modeled_backend_symbol() -> Symbol {
    fake_backend_symbol()
}

/// Deterministic cpal-named site backed by the shared fake host backend.
pub struct CpalModeledSite {
    site: ModeledAudioSite,
}

impl CpalModeledSite {
    /// Builds a deterministic modeled stereo cpal site.
    pub fn stereo() -> Arc<Self> {
        let key = AudioSiteKey::new("sim:cpal-modeled");
        let card = AudioDeviceCard::modeled(key, "cpal Modeled Stereo");
        Arc::new(Self {
            site: ModeledAudioSite::new(card, Arc::new(FakeBackend::new())),
        })
    }

    /// Builds the default deterministic modeled stereo cpal site.
    pub fn default_stereo() -> Arc<Self> {
        Self::stereo()
    }

    /// Builds a stream request for the modeled stereo playback device.
    pub fn playback_request(capacity: usize) -> Result<HostStreamConfigRequest> {
        Ok(HostStreamConfigRequest::new(
            cpal_modeled_backend_symbol(),
            Symbol::new("fake/pcm"),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(capacity)?,
        ))
    }
}

impl AudioSite for CpalModeledSite {
    fn key(&self) -> &AudioSiteKey {
        self.site.key()
    }

    fn card(&self) -> &AudioDeviceCard {
        self.site.card()
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        self.site.open(request)
    }
}

/// Builds the default modeled cpal playback site.
pub fn default_modeled_cpal_site() -> Arc<dyn AudioSite> {
    CpalModeledSite::stereo()
}
