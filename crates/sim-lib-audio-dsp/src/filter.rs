use std::f32::consts::PI;

use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::common::{clamp_cutoff, db_to_gain, input_sample, output_channels, prepare_channels};

/// Mode of a [`OnePoleFilter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnePoleMode {
    /// One-pole low-pass response.
    LowPass,
    /// One-pole high-pass response.
    HighPass,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct OnePoleState {
    z1: f32,
}

/// A per-channel one-pole low-/high-pass filter [`Processor`].
#[derive(Clone, Debug, PartialEq)]
pub struct OnePoleFilter {
    mode: OnePoleMode,
    cutoff_hz: f32,
    sample_rate_hz: f32,
    states: Vec<OnePoleState>,
}

impl OnePoleFilter {
    /// Creates a low-pass one-pole filter at the given cutoff.
    pub fn low_pass(cutoff_hz: f32) -> Self {
        Self::new(OnePoleMode::LowPass, cutoff_hz)
    }

    /// Creates a high-pass one-pole filter at the given cutoff.
    pub fn high_pass(cutoff_hz: f32) -> Self {
        Self::new(OnePoleMode::HighPass, cutoff_hz)
    }

    /// Creates a one-pole filter with the given mode and cutoff in hertz.
    pub fn new(mode: OnePoleMode, cutoff_hz: f32) -> Self {
        Self {
            mode,
            cutoff_hz,
            sample_rate_hz: 48_000.0,
            states: Vec::new(),
        }
    }

    fn alpha(&self) -> f32 {
        let cutoff = clamp_cutoff(self.cutoff_hz, self.sample_rate_hz);
        1.0 - (-2.0 * PI * cutoff / self.sample_rate_hz).exp()
    }
}

impl Processor for OnePoleFilter {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
        prepare_channels(
            &mut self.states,
            cfg.out_channels as usize,
            OnePoleState::default(),
        );
    }

    fn reset(&mut self) {
        self.states.fill(OnePoleState::default());
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let channels = output_channels(block);
        if self.states.len() < channels {
            self.states.resize(channels, OnePoleState::default());
        }
        let alpha = self.alpha();
        let frames = block.frames as usize;
        for channel in 0..channels {
            let state = &mut self.states[channel];
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                state.z1 += alpha * (input - state.z1);
                block.out_audio[channel][frame] = match self.mode {
                    OnePoleMode::LowPass => state.z1,
                    OnePoleMode::HighPass => input - state.z1,
                };
            }
        }
    }
}

/// Response type of a [`BiquadFilter`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BiquadKind {
    /// Low-pass response.
    LowPass,
    /// High-pass response.
    HighPass,
    /// Band-pass response.
    BandPass,
    /// Band-reject (notch) response.
    Notch,
    /// Peaking EQ with the given gain.
    Peaking {
        /// Peak gain in decibels.
        gain_db: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Coefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Default for Coefficients {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BiquadState {
    z1: f32,
    z2: f32,
}

/// A per-channel RBJ biquad filter [`Processor`].
#[derive(Clone, Debug, PartialEq)]
pub struct BiquadFilter {
    kind: BiquadKind,
    frequency_hz: f32,
    q: f32,
    sample_rate_hz: f32,
    coefficients: Coefficients,
    states: Vec<BiquadState>,
}

impl BiquadFilter {
    /// Creates a biquad filter of the given kind, frequency, and Q (clamped to
    /// `>= 0.05`).
    pub fn new(kind: BiquadKind, frequency_hz: f32, q: f32) -> Self {
        let mut filter = Self {
            kind,
            frequency_hz,
            q: q.max(0.05),
            sample_rate_hz: 48_000.0,
            coefficients: Coefficients::default(),
            states: Vec::new(),
        };
        filter.update_coefficients();
        filter
    }

    /// Creates a low-pass biquad at the given frequency and Q.
    pub fn low_pass(frequency_hz: f32, q: f32) -> Self {
        Self::new(BiquadKind::LowPass, frequency_hz, q)
    }

    /// Creates a high-pass biquad at the given frequency and Q.
    pub fn high_pass(frequency_hz: f32, q: f32) -> Self {
        Self::new(BiquadKind::HighPass, frequency_hz, q)
    }

    /// Creates a band-pass biquad at the given frequency and Q.
    pub fn band_pass(frequency_hz: f32, q: f32) -> Self {
        Self::new(BiquadKind::BandPass, frequency_hz, q)
    }

    /// Creates a notch biquad at the given frequency and Q.
    pub fn notch(frequency_hz: f32, q: f32) -> Self {
        Self::new(BiquadKind::Notch, frequency_hz, q)
    }

    fn update_coefficients(&mut self) {
        let frequency = clamp_cutoff(self.frequency_hz, self.sample_rate_hz);
        let omega = 2.0 * PI * frequency / self.sample_rate_hz;
        let sin = omega.sin();
        let cos = omega.cos();
        let alpha = sin / (2.0 * self.q.max(0.05));
        let (b0, b1, b2, a0, a1, a2) = match self.kind {
            BiquadKind::LowPass => (
                (1.0 - cos) * 0.5,
                1.0 - cos,
                (1.0 - cos) * 0.5,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            BiquadKind::HighPass => (
                (1.0 + cos) * 0.5,
                -(1.0 + cos),
                (1.0 + cos) * 0.5,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            BiquadKind::BandPass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BiquadKind::Notch => (1.0, -2.0 * cos, 1.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BiquadKind::Peaking { gain_db } => {
                let amp = db_to_gain(gain_db).sqrt();
                (
                    1.0 + alpha * amp,
                    -2.0 * cos,
                    1.0 - alpha * amp,
                    1.0 + alpha / amp,
                    -2.0 * cos,
                    1.0 - alpha / amp,
                )
            }
        };
        self.coefficients = Coefficients {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        };
    }
}

impl Processor for BiquadFilter {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
        self.update_coefficients();
        prepare_channels(
            &mut self.states,
            cfg.out_channels as usize,
            BiquadState::default(),
        );
    }

    fn reset(&mut self) {
        self.states.fill(BiquadState::default());
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let channels = output_channels(block);
        if self.states.len() < channels {
            self.states.resize(channels, BiquadState::default());
        }
        let c = self.coefficients;
        let frames = block.frames as usize;
        for channel in 0..channels {
            let state = &mut self.states[channel];
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                let output = c.b0 * input + state.z1;
                state.z1 = c.b1 * input - c.a1 * output + state.z2;
                state.z2 = c.b2 * input - c.a2 * output;
                block.out_audio[channel][frame] = output;
            }
        }
    }
}

/// Output tap selected from a [`StateVariableFilter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateVariableMode {
    /// Low-pass output.
    LowPass,
    /// High-pass output.
    HighPass,
    /// Band-pass output.
    BandPass,
    /// Notch (low + high) output.
    Notch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SvfState {
    ic1eq: f32,
    ic2eq: f32,
}

/// A per-channel zero-delay-feedback state-variable filter [`Processor`].
#[derive(Clone, Debug, PartialEq)]
pub struct StateVariableFilter {
    mode: StateVariableMode,
    frequency_hz: f32,
    q: f32,
    sample_rate_hz: f32,
    states: Vec<SvfState>,
}

impl StateVariableFilter {
    /// Creates a state-variable filter with the given output mode, frequency,
    /// and Q (clamped to `>= 0.05`).
    pub fn new(mode: StateVariableMode, frequency_hz: f32, q: f32) -> Self {
        Self {
            mode,
            frequency_hz,
            q: q.max(0.05),
            sample_rate_hz: 48_000.0,
            states: Vec::new(),
        }
    }

    fn process_sample(&self, state: &mut SvfState, input: f32) -> f32 {
        let frequency = clamp_cutoff(self.frequency_hz, self.sample_rate_hz);
        let g = (PI * frequency / self.sample_rate_hz).tan();
        let k = 1.0 / self.q.max(0.05);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = input - state.ic2eq;
        let v1 = a1 * state.ic1eq + a2 * v3;
        let v2 = state.ic2eq + a2 * state.ic1eq + a3 * v3;
        state.ic1eq = 2.0 * v1 - state.ic1eq;
        state.ic2eq = 2.0 * v2 - state.ic2eq;
        let low = v2;
        let high = input - k * v1 - v2;
        match self.mode {
            StateVariableMode::LowPass => low,
            StateVariableMode::HighPass => high,
            StateVariableMode::BandPass => v1,
            StateVariableMode::Notch => low + high,
        }
    }
}

impl Processor for StateVariableFilter {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
        prepare_channels(
            &mut self.states,
            cfg.out_channels as usize,
            SvfState::default(),
        );
    }

    fn reset(&mut self) {
        self.states.fill(SvfState::default());
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let channels = output_channels(block);
        if self.states.len() < channels {
            self.states.resize(channels, SvfState::default());
        }
        let frames = block.frames as usize;
        for channel in 0..channels {
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                let mut state = self.states[channel];
                let output = self.process_sample(&mut state, input);
                self.states[channel] = state;
                block.out_audio[channel][frame] = output;
            }
        }
    }
}
