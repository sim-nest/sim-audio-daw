use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::common::{input_sample, prepare_channels, prepared_output_channels};

/// A per-sample nonlinearity that can be wrapped by [`OversamplingWrapper`].
pub trait NonlinearSampleProcessor: Clone + Send {
    /// Clears any internal state.
    fn reset(&mut self);
    /// Maps one input sample to one output sample.
    fn process_sample(&mut self, input: f32) -> f32;
}

/// A tanh soft-clipping nonlinearity.
#[derive(Clone, Debug, PartialEq)]
pub struct TanhClipper {
    drive: f32,
}

impl TanhClipper {
    /// Creates a tanh clipper with the given drive (clamped to `>= 0`).
    pub fn new(drive: f32) -> Self {
        Self {
            drive: drive.max(0.0),
        }
    }
}

impl NonlinearSampleProcessor for TanhClipper {
    fn reset(&mut self) {}

    fn process_sample(&mut self, input: f32) -> f32 {
        (input * self.drive).tanh()
    }
}

/// A [`Processor`] that runs a [`NonlinearSampleProcessor`] at an integer
/// oversampling factor, interpolating each input across the oversampled steps.
#[derive(Clone, Debug, PartialEq)]
pub struct OversamplingWrapper<P: NonlinearSampleProcessor> {
    prototype: P,
    processors: Vec<P>,
    previous_inputs: Vec<f32>,
    factor: u8,
}

impl<P: NonlinearSampleProcessor> OversamplingWrapper<P> {
    /// Wraps a nonlinearity at the given oversampling factor (clamped to
    /// `1..=16`).
    pub fn new(processor: P, factor: u8) -> Self {
        Self {
            prototype: processor,
            processors: Vec::new(),
            previous_inputs: Vec::new(),
            factor: factor.clamp(1, 16),
        }
    }

    fn process_channel_sample(&mut self, channel: usize, input: f32) -> f32 {
        let previous = self.previous_inputs[channel];
        let mut output = 0.0;
        for step in 1..=self.factor {
            let t = step as f32 / self.factor as f32;
            let upsampled = previous + (input - previous) * t;
            output = self.processors[channel].process_sample(upsampled);
        }
        self.previous_inputs[channel] = input;
        output
    }

    #[cfg(all(test, not(debug_assertions)))]
    pub(crate) fn realtime_state_snapshot(&self) -> Vec<usize> {
        vec![self.processors.capacity(), self.previous_inputs.capacity()]
    }
}

impl OversamplingWrapper<TanhClipper> {
    /// Creates an oversampled tanh soft clipper with the given drive and factor.
    pub fn soft_clipper(drive: f32, factor: u8) -> Self {
        Self::new(TanhClipper::new(drive), factor)
    }
}

impl<P: NonlinearSampleProcessor> Processor for OversamplingWrapper<P> {
    fn prepare(&mut self, cfg: PrepareConfig) {
        prepare_channels(
            &mut self.processors,
            cfg.out_channels as usize,
            self.prototype.clone(),
        );
        prepare_channels(&mut self.previous_inputs, cfg.out_channels as usize, 0.0);
    }

    fn reset(&mut self) {
        self.previous_inputs.fill(0.0);
        for processor in &mut self.processors {
            processor.reset();
        }
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let channels =
            prepared_output_channels(block, self.processors.len(), "OversamplingWrapper");
        let frames = block.frames as usize;
        for channel in 0..channels {
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                block.out_audio[channel][frame] = self.process_channel_sample(channel, input);
            }
        }
    }
}

/// An oversampled tanh soft clipper, the default [`OversamplingWrapper`].
pub type OversampledSoftClipper = OversamplingWrapper<TanhClipper>;
