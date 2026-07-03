use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};

use crate::CoreAudioTiming;

/// Drives a processor from a CoreAudio render callback.
#[derive(Debug)]
pub struct CoreAudioRenderBridge<P> {
    processor: P,
    timing: CoreAudioTiming,
    input_channels: usize,
    output_channels: usize,
    max_block_frames: u32,
    scratch: BlockArena,
    sample_pos: u64,
}

impl<P: Processor> CoreAudioRenderBridge<P> {
    /// Builds a render bridge wrapping `processor` for the given timing and
    /// channel counts.
    ///
    /// The processor is prepared immediately with a [`PrepareConfig`] derived
    /// from `timing`, and a scratch [`BlockArena`] is sized for the largest
    /// lane count. Returns an error when both channel counts are zero or when
    /// the buffer/channel sizes exceed the kernel's frame and channel widths.
    pub fn new(
        mut processor: P,
        timing: CoreAudioTiming,
        input_channels: usize,
        output_channels: usize,
    ) -> Result<Self> {
        if input_channels == 0 && output_channels == 0 {
            return Err(Error::Eval(
                "CoreAudio bridge needs at least one audio lane".to_owned(),
            ));
        }
        let max_block_frames = checked_frames(timing.buffer_frames())?;
        processor.prepare(PrepareConfig::new(
            timing.sample_rate_hz(),
            max_block_frames,
            checked_channels(input_channels, "input")?,
            checked_channels(output_channels, "output")?,
        ));
        Ok(Self {
            processor,
            timing,
            input_channels,
            output_channels,
            max_block_frames,
            scratch: BlockArena::with_f32_capacity(
                timing.buffer_frames() * input_channels.max(output_channels).max(1),
            ),
            sample_pos: 0,
        })
    }

    /// Runs one processing block of `frames` planar frames and returns the
    /// rendered output lanes.
    ///
    /// `input` supplies one slice per input channel; `None` substitutes silence.
    /// The wrapped processor's audio lanes are validated before and after the
    /// call, and the bridge sample position advances by `frames`. Returns an
    /// error when `frames` exceeds the prepared maximum block size or the input
    /// lane shape does not match the configured channel count.
    pub fn render_planar_f32(
        &mut self,
        input: Option<&[&[f32]]>,
        frames: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if frames > self.max_block_frames as usize {
            return Err(Error::Eval(format!(
                "CoreAudio callback received {frames} frames, max block size is {}",
                self.max_block_frames
            )));
        }
        let input_planar = input_planar(input, self.input_channels, frames)?;
        let mut output_planar = vec![vec![0.0; frames]; self.output_channels];
        {
            let input_refs = input_planar.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let mut output_refs = output_planar
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            self.scratch.reset();
            let mut event_sink = NullEventSink;
            let mut block = ProcessBlock {
                frames: frames as u32,
                in_audio: &input_refs,
                out_audio: &mut output_refs,
                in_events: &[],
                out_events: &mut event_sink,
                transport: Transport {
                    sample_pos: self.sample_pos,
                    ..Transport::default()
                },
                scratch: &mut self.scratch,
            };
            block.validate_audio_lanes()?;
            self.processor.process(&mut block);
            block.validate_audio_lanes()?;
        }
        self.sample_pos += frames as u64;
        Ok(output_planar)
    }

    /// Returns the running sample position, in frames rendered so far.
    pub fn sample_pos(&self) -> u64 {
        self.sample_pos
    }

    /// Returns the timing configuration this bridge was built with.
    pub fn timing(&self) -> CoreAudioTiming {
        self.timing
    }

    /// Resets the wrapped processor, the sample position, and the scratch arena.
    pub fn reset(&mut self) {
        self.processor.reset();
        self.sample_pos = 0;
        self.scratch.reset();
    }
}

fn input_planar(input: Option<&[&[f32]]>, channels: usize, frames: usize) -> Result<Vec<Vec<f32>>> {
    let Some(input) = input else {
        return Ok(vec![vec![0.0; frames]; channels]);
    };
    if input.len() != channels {
        return Err(Error::Eval(format!(
            "CoreAudio input has {} channels, expected {channels}",
            input.len()
        )));
    }
    input
        .iter()
        .map(|lane| {
            if lane.len() < frames {
                return Err(Error::Eval(format!(
                    "CoreAudio input lane has {} frames, expected {frames}",
                    lane.len()
                )));
            }
            Ok(lane[..frames].to_vec())
        })
        .collect()
}

fn checked_frames(frames: usize) -> Result<u32> {
    u32::try_from(frames).map_err(|_| Error::Eval("CoreAudio buffer size exceeds u32".to_owned()))
}

fn checked_channels(channels: usize, role: &str) -> Result<u16> {
    u16::try_from(channels)
        .map_err(|_| Error::Eval(format!("CoreAudio {role} channel count exceeds u16")))
}
