//! Modeled cpal provider for the loadable audio-provider contract.

use sim_kernel::{
    AbiVersion, Error, Lib, LibManifest, LibTarget, Linker, LoadCx, Result, Symbol, Version,
};
use sim_lib_stream_host::{AUDIO_PROVIDER_ABI_VERSION, AudioProviderRegistrar};

use crate::default_modeled_cpal_site;

/// Returns the modeled cpal provider identity.
pub fn cpal_modeled_provider_symbol() -> Symbol {
    Symbol::qualified("audio/provider", "cpal-modeled")
}

/// Registers the modeled cpal site through a host-supplied registrar.
pub fn cpal_modeled_provider_entry(registrar: &mut dyn AudioProviderRegistrar) -> Result<()> {
    if registrar.host_abi_version() != AUDIO_PROVIDER_ABI_VERSION {
        return Err(Error::HostError(format!(
            "unsupported audio provider ABI {}",
            registrar.host_abi_version()
        )));
    }
    registrar.register_site(default_modeled_cpal_site());
    Ok(())
}

/// FFI-free modeled cpal provider library.
pub struct CpalProviderModeled;

impl Lib for CpalProviderModeled {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: cpal_modeled_provider_symbol(),
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sim_kernel::{
        Cx, DefaultFactory, EagerPolicy, GrantSeat, Lib, LibLoader, LibSource, LoaderRegistry,
    };
    use sim_lib_stream_host::{
        AudioProviderHost, AudioRouter, AudioSiteKey, DeviceCatalog, RouterAudioProviderRegistrar,
        native_audio_provider_capability,
    };

    use super::*;

    #[derive(Clone, Copy)]
    struct CpalModeledProviderLoader;

    impl LibLoader for CpalModeledProviderLoader {
        fn can_load(&self, source: &LibSource) -> bool {
            matches!(source, LibSource::Symbol(symbol) if symbol == &cpal_modeled_provider_symbol())
        }

        fn load(&self, _cx: &mut Cx, source: LibSource) -> Result<Box<dyn Lib>> {
            assert!(self.can_load(&source));
            Ok(Box::new(CpalProviderModeled))
        }
    }

    fn test_cx() -> (Cx, GrantSeat) {
        Cx::new_seated(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
    }

    #[test]
    fn provider_modeled_entry_registers_cpal_site() {
        let mut router = AudioRouter::new();
        let mut registrar = RouterAudioProviderRegistrar::new(&mut router);

        cpal_modeled_provider_entry(&mut registrar).unwrap();

        let key = AudioSiteKey::new("sim:cpal-modeled");
        assert!(router.site(&key).is_some());
        assert_eq!(router.sites_by_capability(2, &[48_000]), vec![key]);
    }

    #[test]
    fn provider_modeled_loads_through_audio_provider_host() {
        let loaders = LoaderRegistry::new().with_loader(CpalModeledProviderLoader);
        let (mut cx, seat) = test_cx();
        seat.grant(&mut cx, native_audio_provider_capability());
        let mut router = AudioRouter::new();
        let mut host = AudioProviderHost::new(&mut cx, &loaders)
            .with_entry(cpal_modeled_provider_symbol(), cpal_modeled_provider_entry);

        host.load_into(
            LibSource::Symbol(cpal_modeled_provider_symbol()),
            &mut router,
        )
        .unwrap();

        let key = AudioSiteKey::new("sim:cpal-modeled");
        assert!(router.site(&key).is_some());

        let mut catalog = DeviceCatalog::default_modeled();
        catalog.register_provider_sites(&router);
        let records = catalog.enumerate_audio().unwrap();
        assert!(records.iter().any(|record| record.id == key.0));
    }

    #[test]
    fn provider_modeled_manifest_matches_entry_symbol() {
        assert_eq!(
            CpalProviderModeled.manifest().id,
            cpal_modeled_provider_symbol()
        );
    }
}
