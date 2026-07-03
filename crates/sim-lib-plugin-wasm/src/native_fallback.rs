//! Capability-checked native plugin fallback loading.

#[cfg(feature = "clap-host")]
use sim_kernel::Result;
#[cfg(feature = "clap-host")]
use sim_lib_plugin_clap::{ClapHostProcessor, native::ClapHostProvider};
#[cfg(all(feature = "lv2-host", target_os = "linux"))]
use sim_lib_plugin_core::{AudioPluginCapability as Lv2Capability, CapabilitySet as Lv2Caps};
#[cfg(feature = "clap-host")]
use sim_lib_plugin_core::{AudioPluginCapability, CapabilitySet, PluginFormat, PluginLoadSpec};
#[cfg(all(feature = "lv2-host", target_os = "linux"))]
use sim_lib_plugin_lv2::{Lv2HostProcessor, native::Lv2HostProvider};

/// Loads a CLAP plugin after checking the native-plugin capability.
///
/// # Errors
///
/// Returns a capability error when `caps` does not include
/// [`AudioPluginCapability::NativePlugin`]. Returns an error from the provider
/// when `spec` cannot be instantiated.
#[cfg(feature = "clap-host")]
pub fn load_clap_plugin<P: ClapHostProvider>(
    caps: &CapabilitySet,
    spec: &PluginLoadSpec,
    provider: &P,
) -> Result<ClapHostProcessor<P::Plugin>> {
    caps.require(AudioPluginCapability::NativePlugin)?;
    spec.require_format(PluginFormat::Clap)?;
    let plugin = provider.instantiate(spec)?;
    Ok(ClapHostProcessor::new(plugin))
}

/// Loads an LV2 plugin after checking the native-plugin capability.
///
/// # Errors
///
/// Returns a capability error when `caps` does not include
/// [`AudioPluginCapability::NativePlugin`].
/// Returns an error from the provider when `uri` cannot be instantiated.
#[cfg(all(feature = "lv2-host", target_os = "linux"))]
pub fn load_lv2_plugin<P: Lv2HostProvider>(
    caps: &Lv2Caps,
    uri: &str,
    provider: &P,
) -> sim_kernel::Result<Lv2HostProcessor<P::Plugin>> {
    caps.require(Lv2Capability::NativePlugin)?;
    let plugin = provider.instantiate(uri)?;
    Ok(Lv2HostProcessor::new(plugin))
}

#[cfg(all(test, feature = "clap-host"))]
mod tests {
    use sim_kernel::Error;
    use sim_lib_plugin_clap::native::FixtureClapHostProvider;
    use sim_lib_plugin_core::{
        AudioPluginCapability, CapabilitySet, PluginFormat, PluginInstance, PluginLoadSpec,
    };

    use super::load_clap_plugin;

    #[test]
    fn clap_load_denied_without_capability() {
        let spec = PluginLoadSpec::new(PluginFormat::Clap, "fixture://gain").unwrap();
        let provider = FixtureClapHostProvider::gain();
        let err = load_clap_plugin(&CapabilitySet::empty(), &spec, &provider)
            .expect_err("load must require native capability");

        assert!(matches!(
            err,
            Error::CapabilityDenied { capability }
                if capability == AudioPluginCapability::NativePlugin.as_capability_name()
        ));
    }

    #[test]
    fn clap_load_allowed_with_capability() {
        let spec = PluginLoadSpec::new(PluginFormat::Clap, "fixture://gain").unwrap();
        let provider = FixtureClapHostProvider::gain();
        let caps = CapabilitySet::with(AudioPluginCapability::NativePlugin);
        let processor = load_clap_plugin(&caps, &spec, &provider)
            .expect("native capability allows fixture CLAP load");

        assert_eq!(
            processor.instance().descriptor().id.format,
            PluginFormat::Clap
        );
        assert_eq!(
            processor.instance().descriptor().id.stable_id,
            "org.sim.gain"
        );
    }

    #[test]
    fn clap_load_rejects_non_clap_specs() {
        let spec = PluginLoadSpec::new(PluginFormat::Wasm, "fixture://gain").unwrap();
        let provider = FixtureClapHostProvider::gain();
        let caps = CapabilitySet::with(AudioPluginCapability::NativePlugin);

        assert!(load_clap_plugin(&caps, &spec, &provider).is_err());
    }
}

#[cfg(all(test, feature = "lv2-host", target_os = "linux"))]
mod lv2_tests {
    use sim_kernel::Error;
    use sim_lib_plugin_core::{AudioPluginCapability, CapabilitySet, PluginFormat, PluginInstance};
    use sim_lib_plugin_lv2::native::FixtureLv2HostProvider;

    use super::load_lv2_plugin;

    #[test]
    fn lv2_load_denied_without_capability() {
        let provider = FixtureLv2HostProvider::gain();
        let err = load_lv2_plugin(&CapabilitySet::empty(), provider.uri(), &provider)
            .expect_err("load must require native capability");

        assert!(matches!(
            err,
            Error::CapabilityDenied { capability }
                if capability == AudioPluginCapability::NativePlugin.as_capability_name()
        ));
    }

    #[test]
    fn lv2_load_allowed_with_capability() {
        let provider = FixtureLv2HostProvider::gain();
        let caps = CapabilitySet::with(AudioPluginCapability::NativePlugin);
        let processor = load_lv2_plugin(&caps, provider.uri(), &provider)
            .expect("native capability allows fixture LV2 load");

        assert_eq!(
            processor.instance().descriptor().id.format,
            PluginFormat::Lv2
        );
        assert_eq!(
            processor.instance().descriptor().id.stable_id,
            "https://sim.dev/lv2/gain"
        );
    }

    #[test]
    fn lv2_load_rejects_unavailable_uri() {
        let provider = FixtureLv2HostProvider::gain();
        let caps = CapabilitySet::with(AudioPluginCapability::NativePlugin);

        assert!(load_lv2_plugin(&caps, "urn:missing:lv2", &provider).is_err());
    }
}
