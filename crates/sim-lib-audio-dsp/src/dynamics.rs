use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::common::{db_to_gain, gain_to_db, input_sample, output_channels, prepare_channels};

/// A peak envelope follower with separate attack and release time constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicsEnvelope {
    value: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl DynamicsEnvelope {
    /// Creates an envelope follower from attack and release times in
    /// milliseconds at the given sample rate.
    pub fn new(sample_rate_hz: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            value: 0.0,
            attack_coeff: time_coeff(sample_rate_hz, attack_ms),
            release_coeff: time_coeff(sample_rate_hz, release_ms),
        }
    }

    /// Advances the envelope with one input sample and returns the new level.
    pub fn next(&mut self, input: f32) -> f32 {
        let level = input.abs();
        let coeff = if level > self.value {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.value = coeff * self.value + (1.0 - coeff) * level;
        self.value
    }

    /// Resets the envelope to silence.
    pub fn reset(&mut self) {
        self.value = 0.0;
    }
}

fn time_coeff(sample_rate_hz: f32, time_ms: f32) -> f32 {
    let samples = (sample_rate_hz * time_ms.max(0.001)) / 1000.0;
    (-1.0 / samples).exp()
}

/// Nonlinear transfer curve used by [`Waveshaper`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waveshape {
    /// Hyperbolic-tangent soft saturation.
    Tanh,
    /// Cubic soft saturation.
    Cubic,
    /// Hard clip to `-1.0..=1.0`.
    HardClip,
}

/// A [`Processor`] applying a nonlinear waveshaping curve with drive and output
/// gain.
#[derive(Clone, Debug, PartialEq)]
pub struct Waveshaper {
    drive: f32,
    output_gain: f32,
    shape: Waveshape,
}

impl Waveshaper {
    /// Creates a waveshaper with the given curve and drive (clamped to `>= 0`).
    pub fn new(shape: Waveshape, drive: f32) -> Self {
        Self {
            drive: drive.max(0.0),
            output_gain: 1.0,
            shape,
        }
    }

    /// Returns the waveshaper with its post-curve output gain set.
    pub fn with_output_gain(mut self, output_gain: f32) -> Self {
        self.output_gain = output_gain;
        self
    }

    /// Applies the drive, curve, and output gain to one sample.
    pub fn process_sample(&self, input: f32) -> f32 {
        let x = input * self.drive;
        let shaped = match self.shape {
            Waveshape::Tanh => x.tanh(),
            Waveshape::Cubic => (x - x.powi(3) / 3.0).clamp(-1.0, 1.0),
            Waveshape::HardClip => x.clamp(-1.0, 1.0),
        };
        shaped * self.output_gain
    }
}

impl Processor for Waveshaper {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for channel in 0..output_channels(block) {
            for frame in 0..frames {
                block.out_audio[channel][frame] =
                    self.process_sample(input_sample(block, channel, frame));
            }
        }
    }
}

/// A [`Processor`] soft-clipping via a tanh [`Waveshaper`].
#[derive(Clone, Debug, PartialEq)]
pub struct SoftClipper {
    inner: Waveshaper,
}

impl SoftClipper {
    /// Creates a soft clipper with the given drive.
    pub fn new(drive: f32) -> Self {
        Self {
            inner: Waveshaper::new(Waveshape::Tanh, drive),
        }
    }
}

impl Processor for SoftClipper {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.inner.prepare(cfg);
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        self.inner.process(block);
    }
}

/// A per-channel feed-forward compressor [`Processor`].
#[derive(Clone, Debug, PartialEq)]
pub struct Compressor {
    threshold_db: f32,
    ratio: f32,
    makeup_gain: f32,
    sample_rate_hz: f32,
    attack_ms: f32,
    release_ms: f32,
    envelopes: Vec<DynamicsEnvelope>,
}

impl Compressor {
    /// Creates a compressor with the given threshold (dB) and ratio (clamped to
    /// `>= 1.0`), with default timing and unity makeup gain.
    pub fn new(threshold_db: f32, ratio: f32) -> Self {
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            makeup_gain: 1.0,
            sample_rate_hz: 48_000.0,
            attack_ms: 5.0,
            release_ms: 80.0,
            envelopes: Vec::new(),
        }
    }

    /// Returns the compressor with attack and release times (ms) set.
    pub fn with_timing(mut self, attack_ms: f32, release_ms: f32) -> Self {
        self.attack_ms = attack_ms;
        self.release_ms = release_ms;
        self
    }

    /// Returns the compressor with makeup gain set, in decibels.
    pub fn with_makeup_gain_db(mut self, makeup_db: f32) -> Self {
        self.makeup_gain = db_to_gain(makeup_db);
        self
    }

    fn envelope(&self) -> DynamicsEnvelope {
        DynamicsEnvelope::new(self.sample_rate_hz, self.attack_ms, self.release_ms)
    }

    fn gain_for_level(&self, level: f32) -> f32 {
        let level_db = gain_to_db(level);
        if level_db <= self.threshold_db {
            return self.makeup_gain;
        }
        let compressed_db = self.threshold_db + (level_db - self.threshold_db) / self.ratio;
        db_to_gain(compressed_db - level_db) * self.makeup_gain
    }
}

impl Processor for Compressor {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
        let envelope = self.envelope();
        prepare_channels(&mut self.envelopes, cfg.out_channels as usize, envelope);
    }

    fn reset(&mut self) {
        for envelope in &mut self.envelopes {
            envelope.reset();
        }
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        // The audio callback never allocates: `prepare` sized the per-channel
        // state, so clamp to it rather than grow a wider block in place.
        let prepared = self.envelopes.len();
        debug_assert!(
            output_channels(block) <= prepared,
            "Compressor::process received more channels than prepare configured"
        );
        let channels = output_channels(block).min(prepared);
        let frames = block.frames as usize;
        for channel in 0..channels {
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                let level = self.envelopes[channel].next(input);
                block.out_audio[channel][frame] = input * self.gain_for_level(level);
            }
        }
    }
}

/// A brick-wall limiter [`Processor`] built on a fast, high-ratio compressor.
#[derive(Clone, Debug, PartialEq)]
pub struct Limiter {
    inner: Compressor,
}

impl Limiter {
    /// Creates a limiter at the given threshold in decibels.
    pub fn new(threshold_db: f32) -> Self {
        Self {
            inner: Compressor::new(threshold_db, 20.0).with_timing(0.5, 30.0),
        }
    }
}

impl Processor for Limiter {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.inner.prepare(cfg);
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        self.inner.process(block);
    }
}

/// A per-channel noise gate [`Processor`].
#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    threshold_db: f32,
    closed_gain: f32,
    sample_rate_hz: f32,
    envelopes: Vec<DynamicsEnvelope>,
}

impl Gate {
    /// Creates a gate with an open threshold and closed gain, both in decibels.
    pub fn new(threshold_db: f32, closed_gain_db: f32) -> Self {
        Self {
            threshold_db,
            closed_gain: db_to_gain(closed_gain_db),
            sample_rate_hz: 48_000.0,
            envelopes: Vec::new(),
        }
    }

    fn envelope(&self) -> DynamicsEnvelope {
        DynamicsEnvelope::new(self.sample_rate_hz, 2.0, 40.0)
    }
}

impl Processor for Gate {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
        let envelope = self.envelope();
        prepare_channels(&mut self.envelopes, cfg.out_channels as usize, envelope);
    }

    fn reset(&mut self) {
        for envelope in &mut self.envelopes {
            envelope.reset();
        }
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        // The audio callback never allocates: `prepare` sized the per-channel
        // state, so clamp to it rather than grow a wider block in place.
        let prepared = self.envelopes.len();
        debug_assert!(
            output_channels(block) <= prepared,
            "Gate::process received more channels than prepare configured"
        );
        let channels = output_channels(block).min(prepared);
        let frames = block.frames as usize;
        for channel in 0..channels {
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                let level_db = gain_to_db(self.envelopes[channel].next(input));
                let gain = if level_db < self.threshold_db {
                    self.closed_gain
                } else {
                    1.0
                };
                block.out_audio[channel][frame] = input * gain;
            }
        }
    }
}
