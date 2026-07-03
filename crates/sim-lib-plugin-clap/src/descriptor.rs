use sim_kernel::Result;
use sim_lib_audio_graph_core::{PortDecl, PortDir, PortMedia};
use sim_lib_plugin_core::{
    ParameterDescriptor, ParameterKind, PluginDescriptor, PluginFormat, PluginId,
};

/// Builds a CLAP-format audio-effect [`PluginDescriptor`].
///
/// Delegates to `PluginDescriptor::audio_effect` with `PluginFormat::Clap`,
/// producing a symmetric `channels`-wide audio in/out effect identified by
/// `stable_id` and presented as `name`.
pub fn clap_audio_effect_descriptor(
    stable_id: impl Into<String>,
    name: impl Into<String>,
    channels: u16,
) -> Result<PluginDescriptor> {
    PluginDescriptor::audio_effect(PluginFormat::Clap, stable_id, name, channels)
}

/// Builds the CLAP gain fixture descriptor.
///
/// A stereo audio effect (`org.sim.gain`, "SIM Gain") carrying a single
/// floating-point "gain" parameter ranging 0.0 to 2.0 with a default of 1.0.
pub fn clap_gain_descriptor() -> Result<PluginDescriptor> {
    Ok(
        clap_audio_effect_descriptor("org.sim.gain", "SIM Gain", 2)?.with_parameter(
            ParameterDescriptor::new(0, "gain", "Gain", 0.0, 2.0, 1.0)?
                .with_kind(ParameterKind::Float),
        ),
    )
}

/// Builds the CLAP subtractive-synth fixture descriptor.
///
/// An instrument (`org.sim.subtractive-synth`) with one event input port, a
/// stereo audio output port, and an "output-gain" parameter ranging 0.0 to 1.0
/// with a default of 0.25.
pub fn clap_synth_descriptor() -> Result<PluginDescriptor> {
    let mut descriptor = PluginDescriptor::new(
        PluginId::new(PluginFormat::Clap, "org.sim.subtractive-synth")?,
        "SIM Subtractive Synth",
        "sim",
        env!("CARGO_PKG_VERSION"),
    )?;
    descriptor
        .ports
        .push(PortDecl::new("events-in", PortMedia::Event, PortDir::In, 1));
    descriptor.ports.push(PortDecl::new(
        "audio-out",
        PortMedia::Audio,
        PortDir::Out,
        2,
    ));
    descriptor.parameters.push(ParameterDescriptor::new(
        0,
        "output-gain",
        "Output Gain",
        0.0,
        1.0,
        0.25,
    )?);
    Ok(descriptor)
}
