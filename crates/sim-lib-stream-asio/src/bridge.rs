use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};

use crate::AsioTiming;

/// Drives a processor from an ASIO-style buffer switch callback.
#[derive(Debug)]
pub struct AsioBufferSwitchBridge<P> {
    processor: P,
    timing: AsioTiming,
    input_channels: usize,
    output_channels: usize,
    max_block_frames: u32,
    scratch: BlockArena,
    sample_pos: u64,
}

impl<P: Processor> AsioBufferSwitchBridge<P> {
    /// Builds a bridge over `processor`, calling its `prepare` with the timing's
    /// sample rate, buffer size, and channel counts, and sizing scratch for the
    /// wider of the input/output lane counts.
    ///
    /// Returns an error if both channel counts are zero, the buffer size does
    /// not fit in `u32`, or either channel count does not fit in `u16`.
    pub fn new(
        mut processor: P,
        timing: AsioTiming,
        input_channels: usize,
        output_channels: usize,
    ) -> Result<Self> {
        if input_channels == 0 && output_channels == 0 {
            return Err(Error::Eval(
                "ASIO bridge needs at least one audio lane".to_owned(),
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

    /// Runs one buffer switch: wraps the planar f32 `input` (or silence when
    /// `None`) and `frames` count into a `ProcessBlock`, validates lanes, runs
    /// the processor, advances the sample position, and returns the planar
    /// output lanes.
    ///
    /// Returns an error if `frames` exceeds the prepared maximum block size or
    /// the supplied input does not match the configured channel count and frame
    /// length.
    pub fn process_planar_f32(
        &mut self,
        input: Option<&[&[f32]]>,
        frames: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if frames > self.max_block_frames as usize {
            return Err(Error::Eval(format!(
                "ASIO callback received {frames} frames, max block size is {}",
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

    /// Returns the number of frames processed since construction or the last
    /// [`reset`](Self::reset).
    pub fn sample_pos(&self) -> u64 {
        self.sample_pos
    }

    /// Returns the timing this bridge was constructed with.
    pub fn timing(&self) -> AsioTiming {
        self.timing
    }

    /// Resets the wrapped processor, rewinds the sample position to zero, and
    /// clears the scratch arena.
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
            "ASIO input has {} channels, expected {channels}",
            input.len()
        )));
    }
    input
        .iter()
        .map(|lane| {
            if lane.len() < frames {
                return Err(Error::Eval(format!(
                    "ASIO input lane has {} frames, expected {frames}",
                    lane.len()
                )));
            }
            Ok(lane[..frames].to_vec())
        })
        .collect()
}

fn checked_frames(frames: usize) -> Result<u32> {
    u32::try_from(frames).map_err(|_| Error::Eval("ASIO buffer size exceeds u32".to_owned()))
}

fn checked_channels(channels: usize, role: &str) -> Result<u16> {
    u16::try_from(channels)
        .map_err(|_| Error::Eval(format!("ASIO {role} channel count exceeds u16")))
}
