use std::f32::consts::TAU;

use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::{
    common::{input_sample, prepare_channels, prepared_output_channels},
    delay::DelayLine,
};

/// An LFO-modulated delay [`Processor`] underpinning chorus, flanger, and
/// vibrato effects.
#[derive(Clone, Debug, PartialEq)]
pub struct ModulatedDelayProcessor {
    base_delay_seconds: f32,
    depth_seconds: f32,
    rate_hz: f32,
    feedback: f32,
    wet: f32,
    dry: f32,
    sample_rate_hz: f32,
    phase: f32,
    lines: Vec<DelayLine>,
}

impl ModulatedDelayProcessor {
    /// Creates a modulated delay from base delay, modulation depth (both in
    /// milliseconds), and LFO rate in hertz.
    pub fn new(base_delay_ms: f32, depth_ms: f32, rate_hz: f32) -> Self {
        Self {
            base_delay_seconds: (base_delay_ms / 1000.0).max(0.0),
            depth_seconds: (depth_ms / 1000.0).max(0.0),
            rate_hz: rate_hz.max(0.0),
            feedback: 0.0,
            wet: 0.5,
            dry: 0.5,
            sample_rate_hz: 48_000.0,
            phase: 0.0,
            lines: Vec::new(),
        }
    }

    /// Returns the processor with feedback set, clamped to `-0.99..=0.99`.
    pub fn with_feedback(mut self, feedback: f32) -> Self {
        self.feedback = feedback.clamp(-0.99, 0.99);
        self
    }

    /// Returns the processor with explicit dry and wet mix levels.
    pub fn with_mix(mut self, dry: f32, wet: f32) -> Self {
        self.dry = dry;
        self.wet = wet;
        self
    }

    fn max_delay_samples(&self) -> usize {
        ((self.base_delay_seconds + self.depth_seconds) * self.sample_rate_hz).ceil() as usize + 2
    }

    fn current_delay_samples(&self) -> f32 {
        let lfo = self.phase.sin() * 0.5 + 0.5;
        (self.base_delay_seconds + self.depth_seconds * lfo) * self.sample_rate_hz
    }

    fn advance_phase(&mut self) {
        if self.sample_rate_hz > 0.0 {
            self.phase = (self.phase + TAU * self.rate_hz / self.sample_rate_hz).rem_euclid(TAU);
        }
    }

    #[cfg(all(test, not(debug_assertions)))]
    pub(crate) fn realtime_state_snapshot(&self) -> Vec<usize> {
        let mut snapshot = Vec::with_capacity(self.lines.len() + 1);
        snapshot.push(self.lines.capacity());
        snapshot.extend(self.lines.iter().map(DelayLine::allocated_capacity));
        snapshot
    }
}

impl Processor for ModulatedDelayProcessor {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
        let line = DelayLine::new(self.max_delay_samples());
        prepare_channels(&mut self.lines, cfg.out_channels as usize, line);
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        for line in &mut self.lines {
            line.reset();
        }
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let channels = prepared_output_channels(block, self.lines.len(), "ModulatedDelayProcessor");
        let frames = block.frames as usize;
        for frame in 0..frames {
            let delay = self.current_delay_samples();
            for channel in 0..channels {
                let input = input_sample(block, channel, frame);
                let line = &mut self.lines[channel];
                let delayed = line.read(delay);
                line.push(input + delayed * self.feedback);
                block.out_audio[channel][frame] = input * self.dry + delayed * self.wet;
            }
            self.advance_phase();
        }
    }
}

/// A chorus [`Processor`] built on a modulated delay.
#[derive(Clone, Debug, PartialEq)]
pub struct Chorus {
    inner: ModulatedDelayProcessor,
}

impl Chorus {
    /// Creates a chorus with the given LFO rate (Hz) and depth (ms).
    pub fn new(rate_hz: f32, depth_ms: f32) -> Self {
        Self {
            inner: ModulatedDelayProcessor::new(18.0, depth_ms, rate_hz).with_mix(0.65, 0.35),
        }
    }

    #[cfg(all(test, not(debug_assertions)))]
    pub(crate) fn realtime_state_snapshot(&self) -> Vec<usize> {
        self.inner.realtime_state_snapshot()
    }
}

impl Processor for Chorus {
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

/// A flanger [`Processor`] built on a feedback-modulated delay.
#[derive(Clone, Debug, PartialEq)]
pub struct Flanger {
    inner: ModulatedDelayProcessor,
}

impl Flanger {
    /// Creates a flanger with the given LFO rate (Hz), depth (ms), and feedback.
    pub fn new(rate_hz: f32, depth_ms: f32, feedback: f32) -> Self {
        Self {
            inner: ModulatedDelayProcessor::new(2.5, depth_ms, rate_hz)
                .with_feedback(feedback)
                .with_mix(0.55, 0.45),
        }
    }

    #[cfg(all(test, not(debug_assertions)))]
    pub(crate) fn realtime_state_snapshot(&self) -> Vec<usize> {
        self.inner.realtime_state_snapshot()
    }
}

impl Processor for Flanger {
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

/// A vibrato [`Processor`] (fully wet modulated delay).
#[derive(Clone, Debug, PartialEq)]
pub struct Vibrato {
    inner: ModulatedDelayProcessor,
}

impl Vibrato {
    /// Creates a vibrato with the given LFO rate (Hz) and depth (ms).
    pub fn new(rate_hz: f32, depth_ms: f32) -> Self {
        Self {
            inner: ModulatedDelayProcessor::new(depth_ms, depth_ms, rate_hz).with_mix(0.0, 1.0),
        }
    }

    #[cfg(all(test, not(debug_assertions)))]
    pub(crate) fn realtime_state_snapshot(&self) -> Vec<usize> {
        self.inner.realtime_state_snapshot()
    }
}

impl Processor for Vibrato {
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
