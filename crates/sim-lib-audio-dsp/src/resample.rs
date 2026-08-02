use std::{error::Error, f64::consts::PI, fmt};

/// Fixed construction policy for [`PolyphaseResampler`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResamplerPolicy {
    /// Number of fractional-delay phases in the coefficient bank.
    pub phases: usize,
    /// Even number of windowed-sinc taps per phase.
    pub taps: usize,
    /// Fraction of the alias-free cutoff retained, in `(0, 1]`.
    pub cutoff_ratio: f64,
    /// Maximum input frames accepted by one callback call.
    pub max_input_frames: usize,
}

impl Default for ResamplerPolicy {
    fn default() -> Self {
        Self {
            phases: 1_024,
            taps: 32,
            cutoff_ratio: 0.94,
            max_input_frames: 4_096,
        }
    }
}

/// Invalid construction or bounded callback request from a resampler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResampleError {
    /// A sample rate or channel count was zero.
    InvalidFormat,
    /// The coefficient-bank policy was outside its finite supported range.
    InvalidPolicy,
    /// Input or output samples did not form whole interleaved frames.
    MisalignedBuffer,
    /// One callback input exceeded the predeclared frame bound.
    InputLimit {
        /// Frames supplied by the caller.
        supplied: usize,
        /// Maximum frames admitted by the policy.
        maximum: usize,
    },
    /// Caller output storage cannot hold every output implied by this input.
    OutputTooSmall {
        /// Output frames required before consuming input.
        required: usize,
        /// Output frames available in the caller buffer.
        available: usize,
    },
    /// Long-running rational time arithmetic exceeded its representable range.
    TimeOverflow,
}

impl fmt::Display for ResampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "resampler rates and channels must be positive"),
            Self::InvalidPolicy => write!(f, "resampler policy is outside supported bounds"),
            Self::MisalignedBuffer => write!(f, "resampler buffer ends mid-frame"),
            Self::InputLimit { supplied, maximum } => {
                write!(
                    f,
                    "resampler input has {supplied} frames, exceeding {maximum}"
                )
            }
            Self::OutputTooSmall {
                required,
                available,
            } => write!(
                f,
                "resampler needs {required} output frames, but only {available} are available"
            ),
            Self::TimeOverflow => write!(f, "resampler rational time counter overflowed"),
        }
    }
}

impl Error for ResampleError {}

/// Per-call accounting from [`PolyphaseResampler::process_interleaved`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResampleReport {
    /// Whole interleaved input frames consumed.
    pub input_frames: usize,
    /// Whole interleaved output frames written.
    pub output_frames: usize,
    /// Fixed causal filter latency measured in source-rate frames.
    pub latency_input_frames: usize,
}

/// Streaming windowed-sinc polyphase resampler with fixed callback state.
///
/// Construction allocates the coefficient bank and the interleaved history
/// ring. [`process_interleaved`](Self::process_interleaved) writes into caller
/// storage and performs no allocation, locking, or I/O. The filter is causal:
/// the first output is delayed until the right half of its impulse response is
/// available, and callers may append zero input to drain a finite stream.
#[derive(Clone, Debug, PartialEq)]
pub struct PolyphaseResampler {
    input_rate_hz: u32,
    output_rate_hz: u32,
    channels: usize,
    policy: ResamplerPolicy,
    coefficients: Vec<f32>,
    history: Vec<f32>,
    input_frames_seen: u128,
    next_output_time: u128,
}

impl PolyphaseResampler {
    /// Builds a fixed-state resampler and precomputes its Blackman-windowed
    /// low-pass coefficient bank.
    pub fn new(
        input_rate_hz: u32,
        output_rate_hz: u32,
        channels: usize,
        policy: ResamplerPolicy,
    ) -> Result<Self, ResampleError> {
        if input_rate_hz == 0 || output_rate_hz == 0 || channels == 0 {
            return Err(ResampleError::InvalidFormat);
        }
        if policy.phases < 2
            || policy.phases > 65_536
            || policy.taps < 8
            || policy.taps > 256
            || !policy.taps.is_multiple_of(2)
            || !policy.cutoff_ratio.is_finite()
            || !(0.0..=1.0).contains(&policy.cutoff_ratio)
            || policy.cutoff_ratio == 0.0
            || policy.max_input_frames == 0
        {
            return Err(ResampleError::InvalidPolicy);
        }
        let coefficients = coefficient_bank(input_rate_hz, output_rate_hz, policy);
        let history_len = policy
            .taps
            .checked_mul(channels)
            .ok_or(ResampleError::InvalidPolicy)?;
        Ok(Self {
            input_rate_hz,
            output_rate_hz,
            channels,
            policy,
            coefficients,
            history: vec![0.0; history_len],
            input_frames_seen: 0,
            next_output_time: 0,
        })
    }

    /// Returns the configured input rate in hertz.
    pub fn input_rate_hz(&self) -> u32 {
        self.input_rate_hz
    }

    /// Returns the configured output rate in hertz.
    pub fn output_rate_hz(&self) -> u32 {
        self.output_rate_hz
    }

    /// Returns the interleaved channel count.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Returns the fixed source-rate latency of the causal filter.
    pub fn latency_input_frames(&self) -> usize {
        self.policy.taps / 2
    }

    /// Returns the output frames that the next input block will produce.
    pub fn required_output_frames(&self, input_frames: usize) -> Result<usize, ResampleError> {
        if input_frames > self.policy.max_input_frames {
            return Err(ResampleError::InputLimit {
                supplied: input_frames,
                maximum: self.policy.max_input_frames,
            });
        }
        if input_frames == 0 {
            return Ok(0);
        }
        let last_seen = self
            .input_frames_seen
            .checked_add(input_frames as u128)
            .and_then(|value| value.checked_sub(1))
            .ok_or(ResampleError::TimeOverflow)?;
        let right = self.latency_input_frames() as u128;
        let denominator = u128::from(self.output_rate_hz);
        let step = u128::from(self.input_rate_hz);
        let mut time = self.next_output_time;
        let mut count = 0usize;
        while time / denominator + right <= last_seen {
            count = count.checked_add(1).ok_or(ResampleError::TimeOverflow)?;
            time = time.checked_add(step).ok_or(ResampleError::TimeOverflow)?;
        }
        Ok(count)
    }

    /// Consumes interleaved source frames and writes every now-available
    /// resampled frame into `output`.
    ///
    /// The call fails before consuming input when the buffers are misaligned,
    /// the input bound is exceeded, or output storage is too small.
    pub fn process_interleaved(
        &mut self,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<ResampleReport, ResampleError> {
        if !input.len().is_multiple_of(self.channels) || !output.len().is_multiple_of(self.channels)
        {
            return Err(ResampleError::MisalignedBuffer);
        }
        let input_frames = input.len() / self.channels;
        let required = self.required_output_frames(input_frames)?;
        let available = output.len() / self.channels;
        if available < required {
            return Err(ResampleError::OutputTooSmall {
                required,
                available,
            });
        }

        let mut produced = 0usize;
        for frame in 0..input_frames {
            let absolute = self.input_frames_seen;
            let ring_frame = (absolute % self.policy.taps as u128) as usize;
            let ring_base = ring_frame * self.channels;
            let input_base = frame * self.channels;
            self.history[ring_base..ring_base + self.channels]
                .copy_from_slice(&input[input_base..input_base + self.channels]);
            self.input_frames_seen = self
                .input_frames_seen
                .checked_add(1)
                .ok_or(ResampleError::TimeOverflow)?;

            while self.output_ready(absolute) {
                self.render_output_frame(produced, output)?;
                produced += 1;
                self.next_output_time = self
                    .next_output_time
                    .checked_add(u128::from(self.input_rate_hz))
                    .ok_or(ResampleError::TimeOverflow)?;
            }
        }
        debug_assert_eq!(produced, required);
        Ok(ResampleReport {
            input_frames,
            output_frames: produced,
            latency_input_frames: self.latency_input_frames(),
        })
    }

    /// Clears time and sample history while retaining every allocation.
    pub fn reset(&mut self) {
        self.history.fill(0.0);
        self.input_frames_seen = 0;
        self.next_output_time = 0;
    }

    fn output_ready(&self, current_input: u128) -> bool {
        self.next_output_time / u128::from(self.output_rate_hz)
            + self.latency_input_frames() as u128
            <= current_input
    }

    fn render_output_frame(
        &self,
        output_frame: usize,
        output: &mut [f32],
    ) -> Result<(), ResampleError> {
        let denominator = u128::from(self.output_rate_hz);
        let center = self.next_output_time / denominator;
        let remainder = self.next_output_time % denominator;
        let phase = ((remainder * self.policy.phases as u128) / denominator) as usize;
        let coefficient_base = phase.min(self.policy.phases - 1) * self.policy.taps;
        let left = self.policy.taps / 2 - 1;
        for channel in 0..self.channels {
            let mut sample = 0.0f64;
            for tap in 0..self.policy.taps {
                let source = signed_source(center, tap, left);
                let value = source
                    .and_then(|absolute| self.history_sample(absolute, channel))
                    .unwrap_or(0.0);
                sample += f64::from(value) * f64::from(self.coefficients[coefficient_base + tap]);
            }
            let at = output_frame
                .checked_mul(self.channels)
                .and_then(|base| base.checked_add(channel))
                .ok_or(ResampleError::TimeOverflow)?;
            output[at] = sample as f32;
        }
        Ok(())
    }

    fn history_sample(&self, absolute: u128, channel: usize) -> Option<f32> {
        if absolute >= self.input_frames_seen {
            return None;
        }
        let age = self.input_frames_seen - 1 - absolute;
        if age >= self.policy.taps as u128 {
            return None;
        }
        let frame = (absolute % self.policy.taps as u128) as usize;
        Some(self.history[frame * self.channels + channel])
    }

    #[cfg(test)]
    pub(crate) fn realtime_state_snapshot(&self) -> [usize; 2] {
        [self.coefficients.capacity(), self.history.capacity()]
    }
}

fn signed_source(center: u128, tap: usize, left: usize) -> Option<u128> {
    if tap >= left {
        center.checked_add((tap - left) as u128)
    } else {
        center.checked_sub((left - tap) as u128)
    }
}

fn coefficient_bank(input_rate_hz: u32, output_rate_hz: u32, policy: ResamplerPolicy) -> Vec<f32> {
    let rate_ratio = f64::from(output_rate_hz) / f64::from(input_rate_hz);
    let cutoff = 0.5 * rate_ratio.min(1.0) * policy.cutoff_ratio;
    let left = policy.taps / 2 - 1;
    let mut coefficients = Vec::with_capacity(policy.phases * policy.taps);
    for phase in 0..policy.phases {
        let fraction = phase as f64 / policy.phases as f64;
        let start = coefficients.len();
        for tap in 0..policy.taps {
            let distance = tap as f64 - left as f64 - fraction;
            let ideal = 2.0 * cutoff * sinc(2.0 * cutoff * distance);
            let window_position = tap as f64 / (policy.taps - 1) as f64;
            let blackman = 0.42 - 0.5 * (2.0 * PI * window_position).cos()
                + 0.08 * (4.0 * PI * window_position).cos();
            coefficients.push((ideal * blackman) as f32);
        }
        let sum = coefficients[start..]
            .iter()
            .map(|value| f64::from(*value))
            .sum::<f64>();
        for value in &mut coefficients[start..] {
            *value = (f64::from(*value) / sum) as f32;
        }
    }
    coefficients
}

fn sinc(value: f64) -> f64 {
    if value.abs() <= f64::EPSILON {
        1.0
    } else {
        (PI * value).sin() / (PI * value)
    }
}
