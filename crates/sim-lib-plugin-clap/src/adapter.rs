use sim_kernel::Result;
use sim_lib_audio_dsp::Gain;
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};
use sim_lib_plugin_core::{
    HostedPluginProcessor, PluginDescriptor, PluginInstance, PluginState, ProcessorPlugin,
};

use crate::{ClapParamMap, clap_gain_descriptor};

/// Audio-graph [`Processor`] that hosts a CLAP plugin instance.
///
/// Wraps the shared `HostedPluginProcessor` from `sim-lib-plugin-core` and
/// pairs it with a [`ClapParamMap`] so CLAP parameter ids can be translated to
/// SIM parameter ids. The [`Processor`] implementation forwards every call
/// straight to the hosted instance.
#[derive(Clone, Debug)]
pub struct ClapHostProcessor<I> {
    hosted: HostedPluginProcessor<I>,
    param_map: ClapParamMap,
}

impl<I> ClapHostProcessor<I> {
    /// Hosts `instance` with an empty (identity) parameter map.
    pub fn new(instance: I) -> Self {
        Self {
            hosted: HostedPluginProcessor::new(instance),
            param_map: ClapParamMap::new(),
        }
    }

    /// Returns this processor with `param_map` installed as the CLAP-to-SIM
    /// parameter id translation table.
    pub fn with_param_map(mut self, param_map: ClapParamMap) -> Self {
        self.param_map = param_map;
        self
    }

    /// Returns the CLAP-to-SIM parameter id map in effect.
    pub fn param_map(&self) -> &ClapParamMap {
        &self.param_map
    }

    /// Returns a reference to the hosted plugin instance.
    pub fn instance(&self) -> &I {
        self.hosted.instance()
    }
}

impl<I: PluginInstance> Processor for ClapHostProcessor<I> {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.hosted.prepare(cfg);
    }

    fn reset(&mut self) {
        self.hosted.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        self.hosted.process(block);
    }

    fn tail_frames(&self) -> u64 {
        self.hosted.tail_frames()
    }
}

impl<I: PluginInstance> PluginInstance for ClapHostProcessor<I> {
    fn descriptor(&self) -> &PluginDescriptor {
        self.hosted.instance().descriptor()
    }

    fn state(&self) -> PluginState {
        self.hosted.instance().state()
    }

    fn set_state(&mut self, state: PluginState) {
        self.hosted.instance_mut().set_state(state);
    }

    fn prepare(&mut self, cfg: PrepareConfig) {
        self.hosted.prepare(cfg);
    }

    fn reset(&mut self) {
        self.hosted.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        self.hosted.process(block);
    }
}

/// A native SIM [`Processor`] presented to the host as a CLAP plugin instance.
///
/// Wraps the shared `ProcessorPlugin` from `sim-lib-plugin-core` together with
/// a [`ClapParamMap`]; the `PluginInstance` implementation delegates descriptor,
/// state, and processing to the inner plugin so a SIM processor can be exported
/// through the CLAP surface.
#[derive(Clone, Debug)]
pub struct ClapExportedProcessor<P> {
    inner: ProcessorPlugin<P>,
    param_map: ClapParamMap,
}

impl<P> ClapExportedProcessor<P> {
    /// Builds an exported instance from a `descriptor`, the wrapped `processor`,
    /// and its CLAP-to-SIM `param_map`.
    pub fn new(descriptor: PluginDescriptor, processor: P, param_map: ClapParamMap) -> Self {
        Self {
            inner: ProcessorPlugin::new(descriptor, processor),
            param_map,
        }
    }

    /// Returns the CLAP-to-SIM parameter id map for this exported instance.
    pub fn param_map(&self) -> &ClapParamMap {
        &self.param_map
    }
}

sim_lib_plugin_core::forward_plugin_instance!(ClapExportedProcessor);

/// Exports any SIM [`Processor`] as a [`ClapExportedProcessor`].
///
/// Builds an identity [`ClapParamMap`] over the descriptor's parameter ids, so
/// each CLAP parameter id maps to the matching SIM parameter id.
pub fn export_processor_as_clap<P: Processor>(
    descriptor: PluginDescriptor,
    processor: P,
) -> ClapExportedProcessor<P> {
    let param_map = ClapParamMap::identity(descriptor.parameters.iter().map(|param| param.id));
    ClapExportedProcessor::new(descriptor, processor, param_map)
}

/// Exports a `Gain` DSP node as a CLAP gain plugin at the given `gain` value.
///
/// Uses [`clap_gain_descriptor`] for the descriptor and fails closed if that
/// descriptor cannot be built.
pub fn export_gain_as_clap(gain: f32) -> Result<ClapExportedProcessor<Gain>> {
    Ok(export_processor_as_clap(
        clap_gain_descriptor()?,
        Gain::new(gain),
    ))
}
