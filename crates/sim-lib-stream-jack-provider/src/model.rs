//! Modeled JACK provider lane.

use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Export, Lib, LibManifest, LibTarget, Linker, LoadCx, Result, Symbol, Version,
};
use sim_lib_stream_host::{
    AudioDeviceCard, AudioSite, AudioSiteKey, FakeBackend, ModeledAudioSite,
};

/// Returns the provider library identity.
pub fn jack_provider_symbol() -> Symbol {
    Symbol::qualified("audio/provider", "jack")
}

/// Returns the backend identity used by native JACK streams.
pub fn jack_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "jack")
}

/// Returns the modeled JACK audio site export symbol.
pub fn jack_modeled_site_symbol() -> Symbol {
    Symbol::qualified("audio/site", "jack-modeled")
}

/// Builds the deterministic modeled JACK site used by default validation.
pub fn default_modeled_jack_site() -> Arc<dyn AudioSite> {
    let key = AudioSiteKey(jack_modeled_site_symbol());
    let card = AudioDeviceCard::modeled(key, "JACK Provider Modeled");
    Arc::new(ModeledAudioSite::new(card, Arc::new(FakeBackend::new())))
}

/// Enumerates JACK provider sites for the active feature set.
#[cfg(not(feature = "jack-hardware"))]
pub fn enumerate_jack_sites() -> Result<Vec<Arc<dyn AudioSite>>> {
    Ok(vec![default_modeled_jack_site()])
}

/// Enumerates JACK provider sites for the active feature set.
#[cfg(feature = "jack-hardware")]
pub fn enumerate_jack_sites() -> Result<Vec<Arc<dyn AudioSite>>> {
    crate::native::enumerate_jack_hardware_sites()
}

/// FFI-free modeled JACK provider library used by the host loader tests.
pub struct JackProviderModeled;

impl Lib for JackProviderModeled {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: jack_provider_symbol(),
            version: Version("0.1.0".to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Site {
                symbol: jack_modeled_site_symbol(),
                runtime_id: None,
            }],
        }
    }

    fn load(&self, _cx: &mut LoadCx, _linker: &mut Linker) -> Result<()> {
        Ok(())
    }
}
