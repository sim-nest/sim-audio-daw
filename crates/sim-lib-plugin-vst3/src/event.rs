use std::collections::BTreeMap;

use sim_lib_audio_graph_core::BlockEvent;

/// A VST3 input event for one processing block.
///
/// Mirrors the VST3 event surface; [`to_block_event`](Vst3Event::to_block_event)
/// translates each variant into a graph `BlockEvent`, remapping parameter ids
/// through a [`Vst3ParamMap`].
#[derive(Clone, Debug, PartialEq)]
pub enum Vst3Event {
    /// A note-on event.
    NoteOn {
        /// The sample offset of the event within the block.
        sample_offset: u32,
        /// The MIDI channel.
        channel: u8,
        /// The note pitch (MIDI key number).
        pitch: u8,
        /// The note-on velocity.
        velocity: f32,
    },
    /// A note-off event.
    NoteOff {
        /// The sample offset of the event within the block.
        sample_offset: u32,
        /// The MIDI channel.
        channel: u8,
        /// The note pitch (MIDI key number).
        pitch: u8,
        /// The note-off (release) velocity.
        velocity: f32,
    },
    /// A raw MIDI event of up to three bytes.
    Midi {
        /// The sample offset of the event within the block.
        sample_offset: u32,
        /// The MIDI message bytes.
        bytes: [u8; 3],
        /// The number of valid bytes in `bytes`.
        len: u8,
    },
    /// A normalized parameter value change.
    ParamValue {
        /// The sample offset of the event within the block.
        sample_offset: u32,
        /// The VST3 parameter id, remapped to a SIM id on conversion.
        vst3_param_id: u32,
        /// The normalized parameter value.
        normalized: f64,
    },
}

impl Vst3Event {
    /// Translates this event into a graph `BlockEvent`, remapping any parameter
    /// id through `params`.
    pub fn to_block_event(&self, params: &Vst3ParamMap) -> BlockEvent<'_> {
        match self {
            Self::NoteOn {
                sample_offset,
                channel,
                pitch,
                velocity,
            } => BlockEvent::NoteOn {
                offset: *sample_offset,
                channel: *channel,
                key: *pitch,
                velocity: *velocity,
            },
            Self::NoteOff {
                sample_offset,
                channel,
                pitch,
                velocity,
            } => BlockEvent::NoteOff {
                offset: *sample_offset,
                channel: *channel,
                key: *pitch,
                velocity: *velocity,
            },
            Self::Midi {
                sample_offset,
                bytes,
                len,
            } => BlockEvent::Midi {
                offset: *sample_offset,
                bytes: *bytes,
                len: *len,
            },
            Self::ParamValue {
                sample_offset,
                vst3_param_id,
                normalized,
            } => BlockEvent::ParamSet {
                offset: *sample_offset,
                param: params.sim_param_for(*vst3_param_id),
                value: *normalized,
            },
        }
    }
}

/// A mapping from host-facing VST3 parameter ids to SIM parameter ids.
///
/// Unmapped ids pass through unchanged, so an empty map behaves as the identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vst3ParamMap {
    vst3_to_sim: BTreeMap<u32, u32>,
}

impl Vst3ParamMap {
    /// Creates an empty map (every id passes through unchanged).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a map where each id in `ids` maps to itself.
    pub fn identity(ids: impl IntoIterator<Item = u32>) -> Self {
        let mut map = Self::new();
        for id in ids {
            map.insert(id, id);
        }
        map
    }

    /// Records that `vst3_param_id` maps to `sim_param_id`.
    pub fn insert(&mut self, vst3_param_id: u32, sim_param_id: u32) {
        self.vst3_to_sim.insert(vst3_param_id, sim_param_id);
    }

    /// Returns the SIM parameter id for `vst3_param_id`, or the input id itself
    /// when it is unmapped.
    pub fn sim_param_for(&self, vst3_param_id: u32) -> u32 {
        self.vst3_to_sim
            .get(&vst3_param_id)
            .copied()
            .unwrap_or(vst3_param_id)
    }
}

/// An ordered buffer of VST3 events for one processing block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Vst3EventBuffer {
    events: Vec<Vst3Event>,
}

impl Vst3EventBuffer {
    /// Creates a buffer from an existing `events` vector.
    pub fn new(events: Vec<Vst3Event>) -> Self {
        Self { events }
    }

    /// Returns the buffered events in order.
    pub fn events(&self) -> &[Vst3Event] {
        &self.events
    }

    /// Appends `event` to the buffer.
    pub fn push(&mut self, event: Vst3Event) {
        self.events.push(event);
    }

    /// Translates every buffered event into a graph `BlockEvent`, remapping
    /// parameter ids through `params`.
    pub fn to_block_events(&self, params: &Vst3ParamMap) -> Vec<BlockEvent<'_>> {
        self.events
            .iter()
            .map(|event| event.to_block_event(params))
            .collect()
    }
}
