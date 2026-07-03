use sim_kernel::Result;
use sim_lib_audio_dsp::Gain;
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};
use sim_lib_plugin_core::{PluginDescriptor, PluginInstance, PluginState, ProcessorPlugin};

use crate::{Vst3ParamMap, vst3_gain_descriptor};

/// A graph [`Processor`] wrapped as a VST3-shaped plugin instance.
///
/// Pairs a `sim-lib-plugin-core` `ProcessorPlugin` with a [`Vst3ParamMap`] that
/// translates host-facing VST3 parameter ids into the SIM parameter ids the
/// inner processor understands. The wrapper implements `PluginInstance`, so it
/// can be driven through the shared plugin-host contract.
#[derive(Clone, Debug)]
pub struct Vst3ExportedProcessor<P> {
    inner: ProcessorPlugin<P>,
    param_map: Vst3ParamMap,
}

impl<P> Vst3ExportedProcessor<P> {
    /// Wraps `processor` under `descriptor`, using `param_map` for VST3-to-SIM
    /// parameter id translation.
    pub fn new(descriptor: PluginDescriptor, processor: P, param_map: Vst3ParamMap) -> Self {
        Self {
            inner: ProcessorPlugin::new(descriptor, processor),
            param_map,
        }
    }

    /// Returns the VST3-to-SIM parameter id map for this exported processor.
    pub fn param_map(&self) -> &Vst3ParamMap {
        &self.param_map
    }
}

sim_lib_plugin_core::forward_plugin_instance!(Vst3ExportedProcessor);

/// Exports any graph [`Processor`] as a VST3-shaped plugin instance.
///
/// Builds an identity [`Vst3ParamMap`] from `descriptor`'s parameter ids (each
/// VST3 id maps to the matching SIM id) and wraps `processor` in a
/// [`Vst3ExportedProcessor`].
pub fn export_processor_as_vst3<P: Processor>(
    descriptor: PluginDescriptor,
    processor: P,
) -> Vst3ExportedProcessor<P> {
    let param_map = Vst3ParamMap::identity(descriptor.parameters.iter().map(|param| param.id));
    Vst3ExportedProcessor::new(descriptor, processor, param_map)
}

/// Exports the built-in gain processor as a VST3-shaped plugin instance.
///
/// Uses the [`vst3_gain_descriptor`](crate::vst3_gain_descriptor) fixture and a
/// `Gain` processor initialized to `gain`. Returns an error if the descriptor
/// fails to build.
pub fn export_gain_as_vst3(gain: f32) -> Result<Vst3ExportedProcessor<Gain>> {
    Ok(export_processor_as_vst3(
        vst3_gain_descriptor()?,
        Gain::new(gain),
    ))
}
