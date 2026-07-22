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
