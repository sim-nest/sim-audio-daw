use std::sync::{Arc, Mutex};

use sim_lib_audio_graph_core::{BlockEvent, PrepareConfig, ProcessBlock, Processor};

#[derive(Clone, Debug)]
pub(super) struct RecordingProcessor {
    pub(super) state: Arc<Mutex<RecordingState>>,
    gain: f32,
}

#[derive(Clone, Debug, Default)]
pub(super) struct RecordingState {
    pub(super) prepared: Option<PrepareConfig>,
    pub(super) transport_sample_pos: u64,
    pub(super) events: Vec<OwnedEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum OwnedEvent {
    Midi {
        offset: u32,
        bytes: [u8; 3],
        len: u8,
    },
    Param {
        offset: u32,
        param: u32,
        value: f64,
    },
}

impl RecordingProcessor {
    pub(super) fn new(gain: f32) -> (Self, Arc<Mutex<RecordingState>>) {
        let state = Arc::new(Mutex::new(RecordingState::default()));
        (
            Self {
                state: Arc::clone(&state),
                gain,
            },
            state,
        )
    }
}

impl Processor for RecordingProcessor {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.state.lock().expect("recording lock").prepared = Some(cfg);
    }

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let mut state = self.state.lock().expect("recording lock");
        state.transport_sample_pos = block.transport.sample_pos;
        state.events.clear();
        for event in block.in_events {
            match *event {
                BlockEvent::Midi { offset, bytes, len } => {
                    state.events.push(OwnedEvent::Midi { offset, bytes, len });
                }
                BlockEvent::ParamSet {
                    offset,
                    param,
                    value,
                } => state.events.push(OwnedEvent::Param {
                    offset,
                    param,
                    value,
                }),
                _ => {}
            }
        }
        let frames = block.frames as usize;
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for (source, target) in input.iter().zip(output.iter_mut()).take(frames) {
                *target = *source * self.gain;
            }
        }
    }
}

mod diagnostics;
mod runner;
