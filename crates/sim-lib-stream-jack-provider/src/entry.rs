//! Provider entry points.

use sim_kernel::{Error, Result};
use sim_lib_stream_host::{AudioProviderRegistrar, AUDIO_PROVIDER_ABI_VERSION};

/// Registers JACK provider sites through the host-supplied registrar.
pub fn jack_provider_entry(registrar: &mut dyn AudioProviderRegistrar) -> Result<()> {
    if registrar.host_abi_version() != AUDIO_PROVIDER_ABI_VERSION {
        return Err(Error::HostError(format!(
            "unsupported audio provider ABI {}",
            registrar.host_abi_version()
        )));
    }
    for site in crate::enumerate_jack_sites()? {
        registrar.register_site(site);
    }
    Ok(())
}

/// Exported provider symbol resolved by cdylib hosts.
///
/// Returns 0 when registration succeeds and 1 when the host ABI or JACK
/// enumeration is unavailable.
#[cfg(feature = "jack-hardware")]
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn sim_audio_provider_v1(registrar: &mut dyn AudioProviderRegistrar) -> i32 {
    match jack_provider_entry(registrar) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
