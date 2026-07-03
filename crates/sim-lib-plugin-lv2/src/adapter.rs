use sim_kernel::Result;
use sim_lib_audio_dsp::Gain;
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};
use sim_lib_plugin_core::{
    HostedPluginProcessor, PluginDescriptor, PluginInstance, PluginState, ProcessorPlugin,
};

use crate::lv2_gain_descriptor;

/// Graph [`Processor`] that drives an LV2-shaped plugin instance.
///
/// Wraps the shared `HostedPluginProcessor` from `sim-lib-plugin-core` so that
/// any `PluginInstance` (such as an [`Lv2ExportedProcessor`]) participates in
/// the audio graph through the standard [`Processor`] contract.
#[derive(Clone, Debug)]
pub struct Lv2HostProcessor<I> {
    hosted: HostedPluginProcessor<I>,
}

impl<I> Lv2HostProcessor<I> {
    /// Hosts `instance` as a graph processor.
    pub fn new(instance: I) -> Self {
        Self {
            hosted: HostedPluginProcessor::new(instance),
        }
    }

    /// Returns a shared reference to the hosted plugin instance.
    pub fn instance(&self) -> &I {
        self.hosted.instance()
    }

    /// Returns a mutable reference to the hosted plugin instance.
    pub fn instance_mut(&mut self) -> &mut I {
        self.hosted.instance_mut()
    }
}

impl<I: PluginInstance> Processor for Lv2HostProcessor<I> {
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

impl<I: PluginInstance> PluginInstance for Lv2HostProcessor<I> {
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

/// A SIM graph [`Processor`] presented as an LV2-shaped [`PluginInstance`].
///
/// Wraps the shared `ProcessorPlugin` from `sim-lib-plugin-core`, pairing a
/// [`PluginDescriptor`] with the inner processor so a native SIM processor can
/// be exported and hosted as if it were an LV2 plugin.
#[derive(Clone, Debug)]
pub struct Lv2ExportedProcessor<P> {
    inner: ProcessorPlugin<P>,
}

impl<P> Lv2ExportedProcessor<P> {
    /// Pairs `processor` with `descriptor` to form an exported plugin instance.
    pub fn new(descriptor: PluginDescriptor, processor: P) -> Self {
        Self {
            inner: ProcessorPlugin::new(descriptor, processor),
        }
    }
}

sim_lib_plugin_core::forward_plugin_instance!(Lv2ExportedProcessor);

/// Exports `processor` as an LV2-shaped plugin instance under `descriptor`.
pub fn export_processor_as_lv2<P: Processor>(
    descriptor: PluginDescriptor,
    processor: P,
) -> Lv2ExportedProcessor<P> {
    Lv2ExportedProcessor::new(descriptor, processor)
}

/// Exports the built-in gain processor as an LV2-shaped plugin instance.
///
/// `gain` is the linear gain multiplier; the instance carries the standard
/// [`lv2_gain_descriptor`](crate::lv2_gain_descriptor) metadata.
pub fn export_gain_as_lv2(gain: f32) -> Result<Lv2ExportedProcessor<Gain>> {
    Ok(export_processor_as_lv2(
        lv2_gain_descriptor()?,
        Gain::new(gain),
    ))
}
