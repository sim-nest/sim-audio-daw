use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use sim_kernel::{
    AbiVersion, Cx, DefaultFactory, EagerPolicy, Lib, LibLoader, LibManifest, LibSource, LibTarget,
    Linker, LoadCx, LoaderRegistry, Result, Symbol, Version,
};
use sim_lib_stream_host::{
    AudioDeviceCard, AudioProviderRegistrar, AudioRouter, AudioSite, AudioSiteKey, FakeBackend,
    ModeledAudioSite, RouterAudioProviderRegistrar,
};

use crate::jack_backend_symbol;

fn provider_symbol() -> Symbol {
    Symbol::qualified("audio/provider", "jack-spike")
}

fn jack_spike_site() -> Arc<dyn AudioSite> {
    let key = AudioSiteKey::new("audio/native/jack-spike");
    let card = AudioDeviceCard::modeled(key, "JACK Provider Spike");
    Arc::new(ModeledAudioSite::new(card, Arc::new(FakeBackend::new())))
}

#[derive(Clone)]
struct FixtureProviderLoader {
    loaded: Arc<AtomicBool>,
}

impl FixtureProviderLoader {
    fn new(loaded: Arc<AtomicBool>) -> Self {
        Self { loaded }
    }
}

impl LibLoader for FixtureProviderLoader {
    fn can_load(&self, source: &LibSource) -> bool {
        matches!(source, LibSource::Symbol(symbol) if symbol == &provider_symbol())
    }

    fn load(&self, _cx: &mut Cx, source: LibSource) -> Result<Box<dyn Lib>> {
        assert!(self.can_load(&source));
        self.loaded.store(true, Ordering::SeqCst);
        Ok(Box::new(FixtureProviderLib))
    }
}

struct FixtureProviderLib;

impl Lib for FixtureProviderLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: provider_symbol(),
            version: Version("0.1.0".to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: Vec::new(),
        }
    }

    fn load(&self, _cx: &mut LoadCx, _linker: &mut Linker) -> Result<()> {
        Ok(())
    }
}

#[test]
fn spike_provider_loads_through_loader() {
    let loaded = Arc::new(AtomicBool::new(false));
    let loaders =
        LoaderRegistry::new().with_loader(FixtureProviderLoader::new(Arc::clone(&loaded)));
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));

    let provider = loaders
        .load_lib(&mut cx, LibSource::Symbol(provider_symbol()))
        .expect("loader accepts JACK provider symbol");

    assert!(loaded.load(Ordering::SeqCst));
    assert_eq!(provider.manifest().id, provider_symbol());

    let mut router = AudioRouter::new();
    let mut registrar = RouterAudioProviderRegistrar::new(&mut router);
    assert_eq!(registrar.host_abi_version(), 1);
    registrar.register_site(jack_spike_site());

    let key = AudioSiteKey::new("audio/native/jack-spike");
    let site = router.site(&key).expect("JACK spike site registered");
    assert_eq!(site.card().display_name, "JACK Provider Spike");
    assert!(!site.card().hardware_required);
    assert_eq!(
        jack_backend_symbol(),
        Symbol::qualified("stream/host", "jack")
    );
}
