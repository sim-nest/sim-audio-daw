//! Modeled ALSA audio site registration helpers.

use std::sync::Arc;

use sim_lib_stream_host::{AudioDeviceCard, AudioSite, AudioSiteKey, ModeledAudioSite};

use crate::AlsaBackend;

/// Builds the default modeled ALSA playback site.
pub fn default_modeled_alsa_site() -> Arc<dyn AudioSite> {
    let key = AudioSiteKey::new("sim:alsa-modeled");
    let card = AudioDeviceCard::modeled(key, "ALSA Modeled Stereo");
    Arc::new(ModeledAudioSite::new(card, Arc::new(AlsaBackend::fake())))
}
