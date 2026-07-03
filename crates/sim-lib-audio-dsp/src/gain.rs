use std::f32::consts::FRAC_PI_4;

use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::common::{input_sample, output_channels, prepare_channels};

/// A [`Processor`] that scales every channel by a fixed linear gain.
///
/// # Examples
///
/// ```
/// use sim_lib_audio_dsp::Gain;
///
/// let gain = Gain::new(0.5);
/// assert_eq!(gain.gain(), 0.5);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Gain {
    gain: f32,
}

impl Gain {
    /// Creates a gain processor with the given linear gain.
    pub fn new(gain: f32) -> Self {
        Self { gain }
    }

    /// Returns the linear gain.
    pub fn gain(&self) -> f32 {
        self.gain
    }
}

impl Processor for Gain {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for channel in 0..output_channels(block) {
            for frame in 0..frames {
                block.out_audio[channel][frame] = input_sample(block, channel, frame) * self.gain;
            }
        }
    }
}

/// A [`Processor`] applying equal-power stereo panning.
#[derive(Clone, Debug, PartialEq)]
pub struct Pan {
    pan: f32,
}

impl Pan {
    /// Creates a pan processor; `pan` is clamped to `-1.0..=1.0` (left to
    /// right).
    pub fn new(pan: f32) -> Self {
        Self {
            pan: pan.clamp(-1.0, 1.0),
        }
    }

    /// Returns the equal-power left and right channel gains.
    pub fn gains(&self) -> (f32, f32) {
        let angle = (self.pan + 1.0) * FRAC_PI_4;
        (angle.cos(), angle.sin())
    }
}

impl Processor for Pan {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        let (left_gain, right_gain) = self.gains();
        match output_channels(block) {
            0 => {}
            1 => {
                for frame in 0..frames {
                    let mono = input_sample(block, 0, frame);
                    block.out_audio[0][frame] = mono * (left_gain + right_gain) * 0.5;
                }
            }
            _ => {
                for frame in 0..frames {
                    let left = input_sample(block, 0, frame);
                    let right = if block.in_audio.len() > 1 {
                        input_sample(block, 1, frame)
                    } else {
                        left
                    };
                    block.out_audio[0][frame] = left * left_gain;
                    block.out_audio[1][frame] = right * right_gain;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct DcState {
    x1: f32,
    y1: f32,
}

/// A [`Processor`] that removes DC offset with a per-channel one-pole
/// high-pass.
#[derive(Clone, Debug, PartialEq)]
pub struct DcBlocker {
    coefficient: f32,
    states: Vec<DcState>,
}

impl DcBlocker {
    /// Creates a DC blocker; `coefficient` is clamped to `0.0..=0.9999`.
    pub fn new(coefficient: f32) -> Self {
        Self {
            coefficient: coefficient.clamp(0.0, 0.9999),
            states: Vec::new(),
        }
    }
}

impl Default for DcBlocker {
    fn default() -> Self {
        Self::new(0.995)
    }
}

impl Processor for DcBlocker {
    fn prepare(&mut self, cfg: PrepareConfig) {
        prepare_channels(
            &mut self.states,
            cfg.out_channels as usize,
            DcState::default(),
        );
    }

    fn reset(&mut self) {
        self.states.fill(DcState::default());
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let channels = output_channels(block);
        if self.states.len() < channels {
            self.states.resize(channels, DcState::default());
        }
        let frames = block.frames as usize;
        for channel in 0..channels {
            let state = &mut self.states[channel];
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                let output = input - state.x1 + self.coefficient * state.y1;
                state.x1 = input;
                state.y1 = output;
                block.out_audio[channel][frame] = output;
            }
        }
    }
}
