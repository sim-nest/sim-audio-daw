use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::{PluginDescriptor, PluginState};

/// A live, format-agnostic plugin instance the host can prepare and run.
///
/// Implementors are the format-specific backends (vst3/clap/lv2 and the native
/// `sim` format). The trait pairs a static [`PluginDescriptor`] with the
/// mutable, real-time processing entry points shared by every backend and is
/// `Send` so instances can move between threads.
pub trait PluginInstance: Send {
    /// Returns the descriptor that identifies this instance and its port and
    /// parameter layout.
    fn descriptor(&self) -> &PluginDescriptor;

    /// Captures the instance's current persistable state.
    ///
    /// The default returns an empty [`PluginState`]; backends that carry
    /// parameter or opaque data override this.
    fn state(&self) -> PluginState {
        PluginState::new()
    }

    /// Restores the instance from a captured [`PluginState`].
    ///
    /// The default ignores the state; stateful backends override this.
    fn set_state(&mut self, _state: PluginState) {}

    /// Prepares the instance for processing under the given configuration.
    fn prepare(&mut self, cfg: PrepareConfig);

    /// Clears any internal processing state without releasing resources.
    fn reset(&mut self);

    /// Processes one audio block in place.
    fn process(&mut self, block: &mut ProcessBlock<'_>);

    /// Returns and clears the last backend error hidden behind a trait method
    /// that cannot return [`sim_kernel::Result`].
    ///
    /// The default reports no latent error. Fallible backends override this so
    /// hosts using the trait path can audit failures after `process`,
    /// `set_state`, or other non-`Result` entry points.
    fn take_last_error(&mut self) -> Option<String> {
        None
    }

    /// Returns the instance's reported latency in frames.
    ///
    /// The default reports the descriptor's [`PluginDescriptor::latency_frames`].
    fn latency_frames(&self) -> u32 {
        self.descriptor().latency_frames
    }
}

/// Adapts a [`PluginInstance`] into an audio-graph [`Processor`].
///
/// The wrapper forwards prepare/reset/process to the held instance and maps the
/// instance's reported latency onto the graph's tail-frame contract.
#[derive(Clone, Debug)]
pub struct HostedPluginProcessor<I> {
    instance: I,
}

impl<I> HostedPluginProcessor<I> {
    /// Wraps an instance so it can be inserted into an audio graph.
    pub fn new(instance: I) -> Self {
        Self { instance }
    }

    /// Returns a shared reference to the wrapped instance.
    pub fn instance(&self) -> &I {
        &self.instance
    }

    /// Returns a mutable reference to the wrapped instance.
    pub fn instance_mut(&mut self) -> &mut I {
        &mut self.instance
    }

    /// Consumes the wrapper and returns the wrapped instance.
    pub fn into_inner(self) -> I {
        self.instance
    }
}

impl<I: PluginInstance> Processor for HostedPluginProcessor<I> {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.instance.prepare(cfg);
    }

    fn reset(&mut self) {
        self.instance.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        self.instance.process(block);
    }

    fn tail_frames(&self) -> u64 {
        u64::from(self.instance.latency_frames())
    }
}

/// Presents an audio-graph [`Processor`] as a [`PluginInstance`].
///
/// This is the inverse adapter of [`HostedPluginProcessor`]: it pairs a bare
/// processor with a descriptor and a held [`PluginState`], letting any
/// processor be hosted as a native (`sim`-format) plugin. State is stored on the
/// wrapper rather than pushed into the processor.
#[derive(Clone, Debug)]
pub struct ProcessorPlugin<P> {
    descriptor: PluginDescriptor,
    processor: P,
    state: PluginState,
}

impl<P> ProcessorPlugin<P> {
    /// Pairs a descriptor with a processor, starting from empty state.
    pub fn new(descriptor: PluginDescriptor, processor: P) -> Self {
        Self {
            descriptor,
            processor,
            state: PluginState::new(),
        }
    }

    /// Returns a shared reference to the wrapped processor.
    pub fn processor(&self) -> &P {
        &self.processor
    }

    /// Returns a mutable reference to the wrapped processor.
    pub fn processor_mut(&mut self) -> &mut P {
        &mut self.processor
    }

    /// Consumes the wrapper and returns the wrapped processor.
    pub fn into_processor(self) -> P {
        self.processor
    }
}

impl<P: Processor> PluginInstance for ProcessorPlugin<P> {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    fn set_state(&mut self, state: PluginState) {
        self.state = state;
    }

    fn prepare(&mut self, cfg: PrepareConfig) {
        self.processor.prepare(cfg);
    }

    fn reset(&mut self) {
        self.processor.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        self.processor.process(block);
    }
}

/// Implement [`PluginInstance`] for a `$ty<P>` newtype whose only relevant field
/// is `inner: ProcessorPlugin<P>`, forwarding all six methods to it. The clap,
/// lv2, and vst3 exported-processor adapters shared this forward block verbatim
/// Call it at each adapter, where `Processor`, `ProcessBlock`, and
/// `PrepareConfig` (from `sim_lib_audio_graph_core`) and the plugin-core trait
/// types are already in scope.
#[macro_export]
macro_rules! forward_plugin_instance {
    ($ty:ident) => {
        impl<P: Processor> PluginInstance for $ty<P> {
            fn descriptor(&self) -> &PluginDescriptor {
                self.inner.descriptor()
            }

            fn state(&self) -> PluginState {
                self.inner.state()
            }

            fn set_state(&mut self, state: PluginState) {
                self.inner.set_state(state);
            }

            fn prepare(&mut self, cfg: PrepareConfig) {
                self.inner.prepare(cfg);
            }

            fn reset(&mut self) {
                self.inner.reset();
            }

            fn process(&mut self, block: &mut ProcessBlock<'_>) {
                self.inner.process(block);
            }

            fn take_last_error(&mut self) -> Option<String> {
                self.inner.take_last_error()
            }
        }
    };
}
