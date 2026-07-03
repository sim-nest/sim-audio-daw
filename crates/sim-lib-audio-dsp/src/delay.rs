use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::common::{input_sample, output_channels, prepare_channels};

/// A circular delay buffer with fractional, linearly interpolated reads.
#[derive(Clone, Debug, PartialEq)]
pub struct DelayLine {
    buffer: Vec<f32>,
    write: usize,
}

impl DelayLine {
    /// Creates a delay line holding at least `max_delay_samples` samples.
    pub fn new(max_delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay_samples.max(2) + 2],
            write: 0,
        }
    }

    /// Clears the buffer and resets the write position.
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
    }

    /// Reads the sample `delay_samples` in the past, interpolating fractional
    /// delays.
    pub fn read(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let delay = delay_samples.clamp(0.0, (len - 2) as f32);
        let read = (self.write as f32 - delay).rem_euclid(len as f32);
        let i0 = read.floor() as usize % len;
        let i1 = (i0 + 1) % len;
        let frac = read - i0 as f32;
        self.buffer[i0] * (1.0 - frac) + self.buffer[i1] * frac
    }

    /// Writes a sample at the current position and advances the write head.
    pub fn push(&mut self, sample: f32) {
        self.buffer[self.write] = sample;
        self.write = (self.write + 1) % self.buffer.len();
    }
}

/// A per-channel delay [`Processor`] with feedback and dry/wet mix.
#[derive(Clone, Debug, PartialEq)]
pub struct DelayProcessor {
    delay_seconds: f32,
    max_delay_seconds: f32,
    feedback: f32,
    wet: f32,
    dry: f32,
    sample_rate_hz: f32,
    lines: Vec<DelayLine>,
}

impl DelayProcessor {
    /// Creates a delay processor with the given delay and maximum delay, in
    /// seconds, defaulting to a fully wet, feedback-free mix.
    pub fn new(delay_seconds: f32, max_delay_seconds: f32) -> Self {
        Self {
            delay_seconds: delay_seconds.max(0.0),
            max_delay_seconds: max_delay_seconds.max(delay_seconds).max(0.001),
            feedback: 0.0,
            wet: 1.0,
            dry: 0.0,
            sample_rate_hz: 48_000.0,
            lines: Vec::new(),
        }
    }

    /// Creates a delay processor from delay and maximum delay in milliseconds.
    pub fn milliseconds(delay_ms: f32, max_delay_ms: f32) -> Self {
        Self::new(delay_ms / 1000.0, max_delay_ms / 1000.0)
    }

    /// Returns the processor with explicit dry and wet mix levels.
    pub fn with_mix(mut self, dry: f32, wet: f32) -> Self {
        self.dry = dry;
        self.wet = wet;
        self
    }

    /// Returns the processor with feedback set, clamped to `-0.99..=0.99`.
    pub fn with_feedback(mut self, feedback: f32) -> Self {
        self.feedback = feedback.clamp(-0.99, 0.99);
        self
    }

    fn delay_samples(&self) -> f32 {
        self.delay_seconds * self.sample_rate_hz
    }

    fn max_delay_samples(&self) -> usize {
        (self.max_delay_seconds * self.sample_rate_hz).ceil() as usize
    }
}

impl Processor for DelayProcessor {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
        let line = DelayLine::new(self.max_delay_samples());
        prepare_channels(&mut self.lines, cfg.out_channels as usize, line);
    }

    fn reset(&mut self) {
        for line in &mut self.lines {
            line.reset();
        }
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let channels = output_channels(block);
        if self.lines.len() < channels {
            let max_delay_samples = self.max_delay_samples();
            self.lines
                .resize_with(channels, || DelayLine::new(max_delay_samples));
        }
        let delay = self.delay_samples();
        let frames = block.frames as usize;
        for channel in 0..channels {
            let line = &mut self.lines[channel];
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                let delayed = line.read(delay);
                line.push(input + delayed * self.feedback);
                block.out_audio[channel][frame] = input * self.dry + delayed * self.wet;
            }
        }
    }

    fn tail_frames(&self) -> u64 {
        self.delay_samples().ceil() as u64
    }
}

/// A fully wet fractional delay [`Processor`] wrapping [`DelayProcessor`].
#[derive(Clone, Debug, PartialEq)]
pub struct FractionalDelay {
    inner: DelayProcessor,
}

impl FractionalDelay {
    /// Creates a fractional delay from delay and maximum delay in milliseconds.
    pub fn milliseconds(delay_ms: f32, max_delay_ms: f32) -> Self {
        Self {
            inner: DelayProcessor::milliseconds(delay_ms, max_delay_ms),
        }
    }
}

impl Processor for FractionalDelay {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.inner.prepare(cfg);
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        self.inner.process(block);
    }

    fn tail_frames(&self) -> u64 {
        self.inner.tail_frames()
    }
}

/// A feedback comb filter [`Processor`] built on a delay line.
#[derive(Clone, Debug, PartialEq)]
pub struct CombFilter {
    delay: DelayProcessor,
}

impl CombFilter {
    /// Creates a comb filter with the given delay (ms) and feedback amount.
    pub fn milliseconds(delay_ms: f32, feedback: f32) -> Self {
        Self {
            delay: DelayProcessor::milliseconds(delay_ms, delay_ms.max(1.0))
                .with_feedback(feedback)
                .with_mix(0.0, 1.0),
        }
    }
}

impl Processor for CombFilter {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.delay.prepare(cfg);
    }

    fn reset(&mut self) {
        self.delay.reset();
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        self.delay.process(block);
    }

    fn tail_frames(&self) -> u64 {
        self.delay.tail_frames()
    }
}

/// A Schroeder all-pass filter [`Processor`] with per-channel delay lines.
#[derive(Clone, Debug, PartialEq)]
pub struct AllPassFilter {
    delay_seconds: f32,
    feedback: f32,
    sample_rate_hz: f32,
    lines: Vec<DelayLine>,
}

impl AllPassFilter {
    /// Creates an all-pass filter with the given delay (ms) and feedback.
    pub fn milliseconds(delay_ms: f32, feedback: f32) -> Self {
        Self {
            delay_seconds: (delay_ms / 1000.0).max(0.0),
            feedback: feedback.clamp(-0.99, 0.99),
            sample_rate_hz: 48_000.0,
            lines: Vec::new(),
        }
    }

    fn delay_samples(&self) -> f32 {
        self.delay_seconds * self.sample_rate_hz
    }
}

impl Processor for AllPassFilter {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz as f32;
        let samples = self.delay_samples().ceil() as usize;
        prepare_channels(
            &mut self.lines,
            cfg.out_channels as usize,
            DelayLine::new(samples),
        );
    }

    fn reset(&mut self) {
        for line in &mut self.lines {
            line.reset();
        }
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let channels = output_channels(block);
        if self.lines.len() < channels {
            let delay_samples = self.delay_samples() as usize;
            self.lines
                .resize_with(channels, || DelayLine::new(delay_samples));
        }
        let delay = self.delay_samples();
        let frames = block.frames as usize;
        for channel in 0..channels {
            let line = &mut self.lines[channel];
            for frame in 0..frames {
                let input = input_sample(block, channel, frame);
                let delayed = line.read(delay);
                let output = delayed - self.feedback * input;
                line.push(input + self.feedback * output);
                block.out_audio[channel][frame] = output;
            }
        }
    }

    fn tail_frames(&self) -> u64 {
        self.delay_samples().ceil() as u64
    }
}
