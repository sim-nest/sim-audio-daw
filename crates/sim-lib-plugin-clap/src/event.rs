use std::collections::BTreeMap;

use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::BlockEvent;

/// A CLAP input event, mirroring the CLAP event union SIM accepts from a host.
///
/// Each variant carries a sample-relative `time` offset and maps to a matching
/// `BlockEvent` via [`ClapEvent::to_block_event`].
#[derive(Clone, Debug, PartialEq)]
pub enum ClapEvent {
    /// A short (up to 3-byte) raw MIDI message.
    MidiShort {
        /// Sample offset of the event within the block.
        time: u32,
        /// Raw MIDI bytes; only the first `len` are significant.
        bytes: [u8; 3],
        /// Number of valid bytes in `bytes`.
        len: u8,
    },
    /// A note-on event.
    NoteOn {
        /// Sample offset of the event within the block.
        time: u32,
        /// MIDI channel (0-based).
        channel: u8,
        /// Note key number.
        key: u8,
        /// Note velocity, normalized to 0.0 to 1.0.
        velocity: f32,
    },
    /// A note-off event.
    NoteOff {
        /// Sample offset of the event within the block.
        time: u32,
        /// MIDI channel (0-based).
        channel: u8,
        /// Note key number.
        key: u8,
        /// Release velocity, normalized to 0.0 to 1.0.
        velocity: f32,
    },
    /// A parameter value change addressed by CLAP parameter id.
    ParamValue {
        /// Sample offset of the event within the block.
        time: u32,
        /// CLAP parameter id, translated through a [`ClapParamMap`].
        clap_param_id: u32,
        /// New parameter value.
        value: f64,
    },
}

impl ClapEvent {
    /// Translates this CLAP event into the audio-graph `BlockEvent`.
    ///
    /// Note and MIDI events pass through unchanged; a [`ClapEvent::ParamValue`]
    /// has its CLAP parameter id resolved to a SIM parameter id through
    /// `params`.
    pub fn to_block_event(&self, params: &ClapParamMap) -> BlockEvent<'_> {
        match self {
            Self::MidiShort { time, bytes, len } => BlockEvent::Midi {
                offset: *time,
                bytes: *bytes,
                len: *len,
            },
            Self::NoteOn {
                time,
                channel,
                key,
                velocity,
            } => BlockEvent::NoteOn {
                offset: *time,
                channel: *channel,
                key: *key,
                velocity: *velocity,
            },
            Self::NoteOff {
                time,
                channel,
                key,
                velocity,
            } => BlockEvent::NoteOff {
                offset: *time,
                channel: *channel,
                key: *key,
                velocity: *velocity,
            },
            Self::ParamValue {
                time,
                clap_param_id,
                value,
            } => BlockEvent::ParamSet {
                offset: *time,
                param: params.sim_param_for(*clap_param_id),
                value: *value,
            },
        }
    }

    /// Translates this CLAP event after checking that its offset is inside the
    /// current processing block.
    pub fn try_to_block_event(&self, params: &ClapParamMap, frames: u32) -> Result<BlockEvent<'_>> {
        let event = self.to_block_event(params);
        let offset = block_event_offset(&event);
        if offset >= frames {
            return Err(Error::Eval(format!(
                "CLAP event offset {offset} is outside block frames 0..{frames}"
            )));
        }
        Ok(event)
    }
}

fn block_event_offset(event: &BlockEvent<'_>) -> u32 {
    match *event {
        BlockEvent::Midi { offset, .. }
        | BlockEvent::MidiLong { offset, .. }
        | BlockEvent::ParamSet { offset, .. }
        | BlockEvent::NoteOn { offset, .. }
        | BlockEvent::NoteOff { offset, .. } => offset,
    }
}

/// A translation table from CLAP parameter ids to SIM parameter ids.
///
/// Unmapped CLAP ids resolve to themselves, so an empty map behaves as the
/// identity mapping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClapParamMap {
    clap_to_sim: BTreeMap<u32, u32>,
}

impl ClapParamMap {
    /// Creates an empty map (identity translation for every id).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a map sending each id in `ids` to itself.
    pub fn identity(ids: impl IntoIterator<Item = u32>) -> Self {
        let mut map = Self::new();
        for id in ids {
            map.insert(id, id);
        }
        map
    }

    /// Records that `clap_param_id` translates to `sim_param_id`.
    pub fn insert(&mut self, clap_param_id: u32, sim_param_id: u32) {
        self.clap_to_sim.insert(clap_param_id, sim_param_id);
    }

    /// Resolves a CLAP parameter id to its SIM parameter id.
    ///
    /// Returns `clap_param_id` unchanged when no mapping is recorded.
    pub fn sim_param_for(&self, clap_param_id: u32) -> u32 {
        self.clap_to_sim
            .get(&clap_param_id)
            .copied()
            .unwrap_or(clap_param_id)
    }
}

/// An ordered buffer of [`ClapEvent`]s for a single processing block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClapEventBuffer {
    events: Vec<ClapEvent>,
}

impl ClapEventBuffer {
    /// Builds a buffer from a pre-collected list of events.
    pub fn new(events: Vec<ClapEvent>) -> Self {
        Self { events }
    }

    /// Returns the buffered events in order.
    pub fn events(&self) -> &[ClapEvent] {
        &self.events
    }

    /// Appends one event to the end of the buffer.
    pub fn push(&mut self, event: ClapEvent) {
        self.events.push(event);
    }

    /// Translates every buffered event into an audio-graph `BlockEvent`,
    /// resolving parameter ids through `params`.
    pub fn to_block_events(&self, params: &ClapParamMap) -> Vec<BlockEvent<'_>> {
        self.events
            .iter()
            .map(|event| event.to_block_event(params))
            .collect()
    }

    /// Translates buffered events after checking every event offset against the
    /// current processing block.
    pub fn try_to_block_events(
        &self,
        params: &ClapParamMap,
        frames: u32,
    ) -> Result<Vec<BlockEvent<'_>>> {
        self.events
            .iter()
            .map(|event| event.try_to_block_event(params, frames))
            .collect()
    }
}
