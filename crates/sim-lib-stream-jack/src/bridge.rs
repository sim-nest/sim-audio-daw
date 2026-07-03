use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{
    BlockArena, BlockEvent, NullEventSink, PrepareConfig, ProcessBlock, Processor,
};
use sim_lib_stream_audio::{PcmSpec, f32_interleaved_to_planar, f32_planar_to_interleaved};

use crate::JackTransportState;

/// Short MIDI event delivered by JACK for a process block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JackMidiEvent {
    offset: u32,
    bytes: [u8; 3],
    len: u8,
}

/// Drives an audio processor from a JACK process callback.
#[derive(Debug)]
pub struct JackGraphBridge<P> {
    processor: P,
    spec: PcmSpec,
    input_channels: usize,
    max_block_frames: u32,
    scratch: BlockArena,
    last_transport: JackTransportState,
}

impl JackMidiEvent {
    /// Builds a short MIDI event delivered at sample `offset` within the block.
    ///
    /// `bytes` carries one to three status/data bytes; shorter messages are
    /// zero-padded to the fixed three-byte buffer.
    ///
    /// # Errors
    ///
    /// Returns an error when `bytes` is empty or longer than three bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_jack::JackMidiEvent;
    ///
    /// let note_on = JackMidiEvent::short(64, &[0x90, 60, 100]).unwrap();
    /// assert_eq!(note_on.offset(), 64);
    /// assert_eq!(note_on.byte_len(), 3);
    /// assert_eq!(note_on.bytes(), [0x90, 60, 100]);
    ///
    /// let too_long = JackMidiEvent::short(0, &[0, 1, 2, 3]);
    /// assert!(too_long.is_err());
    /// ```
    pub fn short(offset: u32, bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > 3 {
            return Err(Error::Eval(
                "JACK short MIDI event must contain one to three bytes".to_owned(),
            ));
        }
        let mut padded = [0; 3];
        padded[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            offset,
            bytes: padded,
            len: bytes.len() as u8,
        })
    }

    /// Returns the event's sample offset within its process block.
    pub fn offset(self) -> u32 {
        self.offset
    }

    /// Returns the three-byte message buffer, zero-padded past [`byte_len`](Self::byte_len).
    pub fn bytes(self) -> [u8; 3] {
        self.bytes
    }

    /// Returns the number of meaningful bytes in the message (one to three).
    pub fn byte_len(self) -> u8 {
        self.len
    }

    fn block_event(self) -> BlockEvent<'static> {
        BlockEvent::Midi {
            offset: self.offset,
            bytes: self.bytes,
            len: self.len,
        }
    }
}

impl<P: Processor> JackGraphBridge<P> {
    /// Builds a bridge wrapping `processor` for a JACK client.
    ///
    /// The processor is prepared once with the PCM `spec` sample rate, the
    /// `max_block_frames` upper bound, the `input_channels` count, and the
    /// `spec` output channel count. Scratch storage is sized to the largest
    /// block the callback may deliver.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_block_frames` is zero or when either channel
    /// count exceeds `u16`.
    pub fn new(
        mut processor: P,
        spec: PcmSpec,
        input_channels: usize,
        max_block_frames: u32,
    ) -> Result<Self> {
        if max_block_frames == 0 {
            return Err(Error::Eval(
                "JACK block size must be greater than zero".to_owned(),
            ));
        }
        processor.prepare(PrepareConfig::new(
            spec.sample_rate_hz(),
            max_block_frames,
            checked_channels(input_channels, "input")?,
            checked_channels(spec.channels(), "output")?,
        ));
        Ok(Self {
            processor,
            spec,
            input_channels,
            max_block_frames,
            scratch: BlockArena::with_f32_capacity(
                max_block_frames as usize * spec.channels().max(input_channels).max(1),
            ),
            last_transport: JackTransportState::stopped(0),
        })
    }

    /// Runs one process block and returns interleaved output samples.
    ///
    /// `input` is interleaved capture audio (or silence when `None`), `frames`
    /// is the block length, `transport` is the current JACK transport snapshot,
    /// and `midi_events` are the short MIDI events for the block. The samples
    /// are deinterleaved to planar buffers, the wrapped processor runs, and the
    /// planar output is reinterleaved. The transport snapshot is retained for
    /// [`last_transport`](Self::last_transport).
    ///
    /// # Errors
    ///
    /// Returns an error when `frames` exceeds the configured maximum block
    /// size, when interleaving conversions fail, or when the processor's audio
    /// lanes fail validation.
    pub fn process_interleaved_f32(
        &mut self,
        input: Option<&[f32]>,
        frames: usize,
        transport: JackTransportState,
        midi_events: &[JackMidiEvent],
    ) -> Result<Vec<f32>> {
        if frames > self.max_block_frames as usize {
            return Err(Error::Eval(format!(
                "JACK callback received {frames} frames, max block size is {}",
                self.max_block_frames
            )));
        }
        let input_planar = match input {
            Some(samples) => f32_interleaved_to_planar(samples, self.input_channels)?,
            None => vec![vec![0.0; frames]; self.input_channels],
        };
        let mut output_planar = vec![vec![0.0; frames]; self.spec.channels()];
        let input_events = midi_events
            .iter()
            .copied()
            .map(JackMidiEvent::block_event)
            .collect::<Vec<_>>();
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
                in_events: &input_events,
                out_events: &mut event_sink,
                transport: transport.to_graph_transport(),
                scratch: &mut self.scratch,
            };
            block.validate_audio_lanes()?;
            self.processor.process(&mut block);
            block.validate_audio_lanes()?;
        }
        self.last_transport = transport;
        f32_planar_to_interleaved(&output_planar)
    }

    /// Returns the transport snapshot from the most recent process block.
    pub fn last_transport(&self) -> JackTransportState {
        self.last_transport
    }

    /// Resets the wrapped processor, scratch arena, and retained transport.
    pub fn reset(&mut self) {
        self.processor.reset();
        self.last_transport = JackTransportState::stopped(0);
        self.scratch.reset();
    }
}

fn checked_channels(channels: usize, role: &str) -> Result<u16> {
    u16::try_from(channels)
        .map_err(|_| Error::Eval(format!("JACK {role} channel count exceeds u16")))
}
