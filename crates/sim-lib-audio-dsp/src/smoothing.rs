use sim_lib_audio_graph_core::{BlockEvent, PrepareConfig, ProcessBlock, Processor};

use crate::common::{input_sample, output_channels};

/// A linearly ramped scalar that glides from its current value to a target over
/// a fixed number of samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmoothValue {
    current: f32,
    target: f32,
    step: f32,
    remaining: u32,
}

impl SmoothValue {
    /// Creates a smoothed value starting and resting at `value`.
    pub fn new(value: f32) -> Self {
        Self {
            current: value,
            target: value,
            step: 0.0,
            remaining: 0,
        }
    }

    /// Returns the current (possibly mid-ramp) value.
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Returns the target value being ramped toward.
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Sets a new target reached over `samples` samples; `0` jumps immediately.
    pub fn set_target(&mut self, target: f32, samples: u32) {
        self.target = target;
        if samples == 0 {
            self.current = target;
            self.step = 0.0;
            self.remaining = 0;
            return;
        }
        self.remaining = samples;
        self.step = (target - self.current) / samples as f32;
    }

    /// Advances one sample along the ramp and returns the new current value.
    pub fn next_sample(&mut self) -> f32 {
        if self.remaining > 0 {
            self.current += self.step;
            self.remaining -= 1;
            if self.remaining == 0 {
                self.current = self.target;
            }
        }
        self.current
    }

    /// Resets the ramp to rest immediately at `value`.
    pub fn reset(&mut self, value: f32) {
        *self = Self::new(value);
    }
}

/// A [`Processor`] applying a click-free gain that ramps toward parameter-set
/// targets received as block events.
#[derive(Clone, Debug, PartialEq)]
pub struct SmoothedGain {
    gain: SmoothValue,
    ramp_ms: f32,
    param_id: u32,
    sample_rate_hz: f32,
}

impl SmoothedGain {
    /// Creates a smoothed gain at `initial_gain` ramping over `ramp_ms`.
    pub fn new(initial_gain: f32, ramp_ms: f32) -> Self {
        Self {
            gain: SmoothValue::new(initial_gain),
            ramp_ms,
            param_id: 0,
            sample_rate_hz: 48_000.0,
        }
    }

    /// Returns the gain bound to react to the given parameter id.
    pub fn with_param(mut self, param_id: u32) -> Self {
        self.param_id = param_id;
        self
    }

    fn ramp_samples(&self) -> u32 {
        ((self.sample_rate_hz * self.ramp_ms.max(0.0)) / 1000.0).round() as u32
    }
}

impl Processor for SmoothedGain {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
    }

    fn reset(&mut self) {
        let value = self.gain.target();
        self.gain.reset(value);
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        let channels = output_channels(block);
        for frame in 0..frames {
            for event in block.in_events {
                if let BlockEvent::ParamSet {
                    offset,
                    param,
                    value,
                } = *event
                    && offset as usize == frame
                    && param == self.param_id
                {
                    self.gain.set_target(value as f32, self.ramp_samples());
                }
            }
            let gain = self.gain.next_sample();
            for channel in 0..channels {
                block.out_audio[channel][frame] = input_sample(block, channel, frame) * gain;
            }
        }
    }
}
