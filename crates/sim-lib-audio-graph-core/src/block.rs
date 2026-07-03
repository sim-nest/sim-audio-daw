use sim_kernel::{Error, Result};

use crate::{BlockArena, BlockEvent, EventSink, Transport};

/// One processing block handed to a [`Processor`](crate::Processor): input and
/// output audio lanes, event streams, transport state, and a scratch arena.
pub struct ProcessBlock<'a> {
    /// Number of valid frames in each audio lane for this block.
    pub frames: u32,
    /// Input audio lanes, one slice per input channel.
    pub in_audio: &'a [&'a [f32]],
    /// Output audio lanes, one mutable slice per output channel.
    pub out_audio: &'a mut [&'a mut [f32]],
    /// Input events delivered to the processor for this block.
    pub in_events: &'a [BlockEvent<'a>],
    /// Sink the processor pushes outgoing events into.
    pub out_events: &'a mut dyn EventSink,
    /// Transport state (playhead, tempo, play flag) for this block.
    pub transport: Transport,
    /// Per-block scratch allocator for temporary buffers.
    pub scratch: &'a mut BlockArena,
}

impl ProcessBlock<'_> {
    /// Validates that every input and output audio lane holds at least
    /// [`frames`](Self::frames) samples.
    pub fn validate_audio_lanes(&self) -> Result<()> {
        let frames = self.frames as usize;
        if let Some(len) = self
            .in_audio
            .iter()
            .find_map(|lane| (lane.len() < frames).then_some(lane.len()))
        {
            return Err(Error::Eval(format!(
                "input audio lane has {len} frames, expected at least {frames}"
            )));
        }
        if let Some(len) = self
            .out_audio
            .iter()
            .find_map(|lane| (lane.len() < frames).then_some(lane.len()))
        {
            return Err(Error::Eval(format!(
                "output audio lane has {len} frames, expected at least {frames}"
            )));
        }
        Ok(())
    }
}
