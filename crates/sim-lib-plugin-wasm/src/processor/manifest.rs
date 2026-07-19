use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{PortDecl, PortDir, PortMedia};
use sim_lib_plugin_core::{ParameterDescriptor, PluginDescriptor, PluginFormat, PluginId};

use crate::abi::WasmAudioManifest;

/// Host ceiling on the audio channel count a plugin manifest may declare.
///
/// The manifest is guest-supplied text; the host sizes per-channel buffers from
/// it in `WasmPluginProcessor::prepare`, so an unbounded count would let a
/// hostile plugin drive an arbitrary host allocation. The cap is validated when
/// the descriptor is built, before any host buffer is allocated.
pub(super) const MAX_PLUGIN_CHANNELS: u16 = 512;

pub(super) fn descriptor_from_manifest(manifest: &WasmAudioManifest) -> Result<PluginDescriptor> {
    check_plugin_channels("input", manifest.audio_in_channels)?;
    check_plugin_channels("output", manifest.audio_out_channels)?;
    let plugin_id = PluginId::new(PluginFormat::Wasm, manifest.stable_id_str().to_owned())?;
    let mut descriptor = PluginDescriptor::new(
        plugin_id,
        manifest.name_str().to_owned(),
        manifest.vendor_str().to_owned(),
        "0.1.0".to_owned(),
    )?;
    if manifest.audio_in_channels > 0 {
        descriptor.ports.push(PortDecl::new(
            "audio-in",
            PortMedia::Audio,
            PortDir::In,
            manifest.audio_in_channels,
        ));
    }
    if manifest.audio_out_channels > 0 {
        descriptor.ports.push(PortDecl::new(
            "audio-out",
            PortMedia::Audio,
            PortDir::Out,
            manifest.audio_out_channels,
        ));
    }
    for id in 0..u32::from(manifest.param_count) {
        descriptor.parameters.push(ParameterDescriptor::new(
            id,
            format!("param-{id}"),
            format!("Param {id}"),
            0.0,
            1.0,
            1.0,
        )?);
    }
    Ok(descriptor)
}

fn check_plugin_channels(which: &str, channels: u16) -> Result<()> {
    if channels > MAX_PLUGIN_CHANNELS {
        return Err(Error::Eval(format!(
            "wasm plugin declares {channels} {which} channels, exceeding the host maximum of {MAX_PLUGIN_CHANNELS}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MAX_PLUGIN_CHANNELS, descriptor_from_manifest};
    use crate::abi::WasmAudioManifest;

    fn manifest_with_channels(audio_in: u16, audio_out: u16) -> WasmAudioManifest {
        let mut name = [0u8; 64];
        name[..4].copy_from_slice(b"test");
        let mut stable_id = [0u8; 64];
        stable_id[..8].copy_from_slice(b"sim.test");
        WasmAudioManifest {
            audio_in_channels: audio_in,
            audio_out_channels: audio_out,
            param_count: 0,
            _pad: 0,
            name,
            vendor: [0u8; 32],
            stable_id,
        }
    }

    #[test]
    fn manifest_within_channel_cap_builds_descriptor() {
        let manifest = manifest_with_channels(2, MAX_PLUGIN_CHANNELS);
        let descriptor = descriptor_from_manifest(&manifest).expect("channel count within cap");
        assert_eq!(descriptor.ports.len(), 2);
    }

    #[test]
    fn manifest_over_channel_cap_is_rejected() {
        let manifest = manifest_with_channels(u16::MAX, 2);
        let err = descriptor_from_manifest(&manifest)
            .expect_err("manifest over the channel cap must be rejected");
        assert!(format!("{err}").contains("exceeding the host maximum"));
    }

    #[test]
    fn manifest_over_output_channel_cap_is_rejected() {
        let manifest = manifest_with_channels(2, MAX_PLUGIN_CHANNELS + 1);
        let err = descriptor_from_manifest(&manifest)
            .expect_err("output channel count over the cap must be rejected");
        assert!(format!("{err}").contains("output channels"));
    }
}
