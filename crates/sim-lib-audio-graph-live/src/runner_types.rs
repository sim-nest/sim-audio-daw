use sim_kernel::{Error, Result};
use sim_lib_stream_audio::PcmSpec;

pub(crate) const MAX_LIVE_CHANNELS: usize = 2;
pub(crate) const MAX_LIVE_EVENTS: usize = 64;

/// Live runner configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveGraphConfig {
    pub(crate) spec: PcmSpec,
    pub(crate) input_channels: usize,
    pub(crate) max_block_frames: u32,
    pub(crate) control_queue_capacity: usize,
    pub(crate) audio_queue_capacity: usize,
}

/// Process result for one live callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveProcessReport {
    pub(crate) frames: u32,
    pub(crate) control_events: usize,
    pub(crate) dropped_control_events: u64,
}

/// Capacity snapshot used to verify steady-state processing does not grow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveSteadyStateSnapshot {
    pub(crate) input_lane_capacity: Vec<usize>,
    pub(crate) output_lane_capacity: Vec<usize>,
    pub(crate) scratch_capacity: usize,
    pub(crate) control_queue_capacity: usize,
    pub(crate) audio_queue_capacity: usize,
}

impl LiveGraphConfig {
    /// Builds a runner configuration, validating block size, channel counts
    /// (up to stereo), and bounded queue capacities.
    pub fn new(
        spec: PcmSpec,
        input_channels: usize,
        max_block_frames: u32,
        control_queue_capacity: usize,
        audio_queue_capacity: usize,
    ) -> Result<Self> {
        if max_block_frames == 0 {
            return Err(Error::Eval(
                "live graph max block frames must be greater than zero".to_owned(),
            ));
        }
        if input_channels > MAX_LIVE_CHANNELS || spec.channels() > MAX_LIVE_CHANNELS {
            return Err(Error::Eval(format!(
                "live graph runner supports up to {MAX_LIVE_CHANNELS} channels"
            )));
        }
        if control_queue_capacity == 0 || audio_queue_capacity == 0 {
            return Err(Error::Eval(
                "live graph queues must be bounded and non-zero".to_owned(),
            ));
        }
        if control_queue_capacity > MAX_LIVE_EVENTS {
            return Err(Error::Eval(format!(
                "live graph control queue supports up to {MAX_LIVE_EVENTS} events per block"
            )));
        }
        if audio_queue_capacity > MAX_LIVE_EVENTS {
            return Err(Error::Eval(format!(
                "live graph audio diagnostic queue supports up to {MAX_LIVE_EVENTS} events per block"
            )));
        }
        Ok(Self {
            spec,
            input_channels,
            max_block_frames,
            control_queue_capacity,
            audio_queue_capacity,
        })
    }

    /// Builds a stereo-in/stereo-out configuration with full event queues.
    pub fn stereo(sample_rate_hz: u32, max_block_frames: u32) -> Result<Self> {
        Self::new(
            PcmSpec::f32(2, sample_rate_hz)?,
            2,
            max_block_frames,
            MAX_LIVE_EVENTS,
            MAX_LIVE_EVENTS,
        )
    }

    /// Returns the output PCM spec.
    pub fn spec(self) -> PcmSpec {
        self.spec
    }

    /// Returns the input channel count.
    pub fn input_channels(self) -> usize {
        self.input_channels
    }

    /// Returns the output channel count.
    pub fn output_channels(self) -> usize {
        self.spec.channels()
    }

    /// Returns the maximum block size in frames.
    pub fn max_block_frames(self) -> u32 {
        self.max_block_frames
    }
}

impl LiveProcessReport {
    /// Returns the number of frames processed.
    pub fn frames(self) -> u32 {
        self.frames
    }

    /// Returns the number of control events applied this block.
    pub fn control_events(self) -> usize {
        self.control_events
    }

    /// Returns the number of control events dropped before this block.
    pub fn dropped_control_events(self) -> u64 {
        self.dropped_control_events
    }
}
