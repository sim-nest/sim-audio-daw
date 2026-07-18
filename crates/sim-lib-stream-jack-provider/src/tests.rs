use std::sync::Arc;

use sim_kernel::{
    Cx, DefaultFactory, EagerPolicy, Lib, LibLoader, LibSource, LoaderRegistry, Result,
};
use sim_lib_stream_host::{
    native_audio_provider_capability, AudioProviderHost, AudioRouter, AudioSiteKey, DeviceCatalog,
    RouterAudioProviderRegistrar,
};

use crate::{
    jack_modeled_site_symbol, jack_provider_entry, jack_provider_symbol, JackProviderModeled,
};

#[derive(Clone, Copy)]
struct JackModeledProviderLoader;

impl LibLoader for JackModeledProviderLoader {
    fn can_load(&self, source: &LibSource) -> bool {
        matches!(source, LibSource::Symbol(symbol) if symbol == &jack_provider_symbol())
    }

    fn load(&self, _cx: &mut Cx, source: LibSource) -> Result<Box<dyn Lib>> {
        assert!(self.can_load(&source));
        Ok(Box::new(JackProviderModeled))
    }
}

fn test_cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}

#[cfg(not(feature = "jack-hardware"))]
#[test]
fn modeled_entry_registers_jack_site() {
    let mut router = AudioRouter::new();
    let mut registrar = RouterAudioProviderRegistrar::new(&mut router);

    jack_provider_entry(&mut registrar).unwrap();

    let key = AudioSiteKey(jack_modeled_site_symbol());
    let site = router.site(&key).expect("modeled JACK site registered");
    assert_eq!(site.card().display_name, "JACK Provider Modeled");
    assert!(!site.card().hardware_required);
}

#[cfg(not(feature = "jack-hardware"))]
#[test]
fn modeled_provider_loads_through_audio_provider_host() {
    let loaders = LoaderRegistry::new().with_loader(JackModeledProviderLoader);
    let mut cx = test_cx();
    cx.grant(native_audio_provider_capability());
    let mut router = AudioRouter::new();
    let mut host = AudioProviderHost::new(&mut cx, &loaders)
        .with_entry(jack_provider_symbol(), jack_provider_entry);

    host.load_into(LibSource::Symbol(jack_provider_symbol()), &mut router)
        .unwrap();

    let key = AudioSiteKey(jack_modeled_site_symbol());
    assert!(router.site(&key).is_some());

    let mut catalog = DeviceCatalog::default_modeled();
    catalog.register_provider_sites(&router);
    let records = catalog.enumerate_audio().unwrap();
    assert!(records.iter().any(|record| record.id == key.0));
}

#[test]
fn provider_manifest_matches_symbol() {
    assert_eq!(JackProviderModeled.manifest().id, jack_provider_symbol());
}

#[cfg(feature = "jack-hardware")]
#[test]
fn jack_hardware_smoke() {
    if std::env::var("SIM_JACK_HARDWARE_SMOKE").as_deref() != Ok("1") {
        eprintln!("set SIM_JACK_HARDWARE_SMOKE=1 to open a real JACK client");
        return;
    }

    let sites = crate::enumerate_jack_sites().expect("JACK server is available");
    assert!(!sites.is_empty(), "no JACK provider sites found");
}
