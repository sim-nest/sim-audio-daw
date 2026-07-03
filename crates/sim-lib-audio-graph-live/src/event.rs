use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::BlockEvent;
use sim_lib_stream_core::{BackpressureOutcome, StreamDiagnostic, StreamPacket};

/// Result of pushing into a bounded live queue.
pub type LiveQueuePush = BackpressureOutcome;

/// Owned control event moved from the control thread to the audio callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiveControlEvent {
    /// A short MIDI message of up to three bytes.
    Midi {
        /// Sample offset into the block.
        offset: u32,
        /// Message bytes (only the first `len` are valid).
        bytes: [u8; 3],
        /// Number of valid bytes in `bytes`.
        len: u8,
    },
    /// A parameter-set event.
    ParamSet {
        /// Sample offset into the block.
        offset: u32,
        /// Parameter index.
        param: u32,
        /// New parameter value.
        value: f64,
    },
}

/// Owned audio event moved from the audio callback to the control thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiveAudioEvent {
    /// An audio callback received more frames than the prepared maximum.
    Xrun {
        /// Frames delivered to the callback.
        frames: u32,
        /// Maximum prepared block size.
        max_frames: u32,
    },
    /// Control-to-audio queue dropped events under backpressure.
    DroppedControlEvents {
        /// Number of dropped events.
        count: u64,
    },
    /// Audio-to-control queue dropped events under backpressure.
    DroppedAudioEvents {
        /// Number of dropped events.
        count: u64,
    },
    /// A processor emitted a parameter-set event.
    ProcessorParamSet {
        /// Sample offset into the block.
        offset: u32,
        /// Parameter index.
        param: u32,
        /// New parameter value.
        value: f64,
    },
    /// A processor emitted a short MIDI message.
    ProcessorMidi {
        /// Sample offset into the block.
        offset: u32,
        /// Message bytes (only the first `len` are valid).
        bytes: [u8; 3],
        /// Number of valid bytes in `bytes`.
        len: u8,
    },
}

impl LiveControlEvent {
    /// Builds a [`LiveControlEvent::Midi`] from one to three MIDI bytes.
    pub fn midi_short(offset: u32, bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > 3 {
            return Err(Error::Eval(
                "live MIDI event must contain one to three bytes".to_owned(),
            ));
        }
        let mut padded = [0; 3];
        padded[..bytes.len()].copy_from_slice(bytes);
        Ok(Self::Midi {
            offset,
            bytes: padded,
            len: bytes.len() as u8,
        })
    }

    /// Builds a [`LiveControlEvent::ParamSet`], requiring a finite value.
    pub fn param_set(offset: u32, param: u32, value: f64) -> Result<Self> {
        if !value.is_finite() {
            return Err(Error::Eval(
                "live parameter value must be finite".to_owned(),
            ));
        }
        Ok(Self::ParamSet {
            offset,
            param,
            value,
        })
    }

    /// Returns the event's sample offset within its block.
    pub fn offset(self) -> u32 {
        match self {
            Self::Midi { offset, .. } | Self::ParamSet { offset, .. } => offset,
        }
    }

    /// Converts the event into a graph-core [`BlockEvent`].
    pub fn to_block_event(self) -> BlockEvent<'static> {
        match self {
            Self::Midi { offset, bytes, len } => BlockEvent::Midi { offset, bytes, len },
            Self::ParamSet {
                offset,
                param,
                value,
            } => BlockEvent::ParamSet {
                offset,
                param,
                value,
            },
        }
    }
}

impl LiveAudioEvent {
    /// Captures a processor-emitted [`BlockEvent`] as a live audio event.
    ///
    /// Returns `None` for events that are not carried back to the control
    /// thread (long MIDI, note-on, and note-off).
    pub fn from_processor_event(event: BlockEvent<'_>) -> Option<Self> {
        match event {
            BlockEvent::Midi { offset, bytes, len } => {
                Some(Self::ProcessorMidi { offset, bytes, len })
            }
            BlockEvent::ParamSet {
                offset,
                param,
                value,
            } => Some(Self::ProcessorParamSet {
                offset,
                param,
                value,
            }),
            BlockEvent::MidiLong { .. }
            | BlockEvent::NoteOn { .. }
            | BlockEvent::NoteOff { .. } => None,
        }
    }

    /// Renders the event as a stream diagnostic packet for the control thread.
    pub fn to_diagnostic_packet(self) -> StreamPacket {
        let (kind, message) = match self {
            Self::Xrun { frames, max_frames } => (
                Symbol::qualified("stream/diagnostic", "xrun"),
                format!("live callback received {frames} frames, max block is {max_frames}"),
            ),
            Self::DroppedControlEvents { count } => (
                Symbol::qualified("stream/diagnostic", "control-drop"),
                format!("live control-to-audio queue dropped {count} events"),
            ),
            Self::DroppedAudioEvents { count } => (
                Symbol::qualified("stream/diagnostic", "audio-drop"),
                format!("live audio-to-control queue dropped {count} events"),
            ),
            Self::ProcessorParamSet {
                offset,
                param,
                value,
            } => (
                Symbol::qualified("stream/diagnostic", "processor-param"),
                format!("processor emitted param {param}={value} at frame {offset}"),
            ),
            Self::ProcessorMidi { offset, len, .. } => (
                Symbol::qualified("stream/diagnostic", "processor-midi"),
                format!("processor emitted {len}-byte MIDI event at frame {offset}"),
            ),
        };
        StreamPacket::Diagnostic(StreamDiagnostic::new(kind, message))
    }
}
