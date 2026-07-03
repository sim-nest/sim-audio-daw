use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, BlockEvent, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};
use sim_lib_plugin_core::{PluginFormat, PluginInstance};

use crate::{
    ClapEvent, ClapEventBuffer, ClapHostProcessor, ClapParamMap, clap_plugin_symbols,
    export_gain_as_clap, install_clap_plugin_lib,
};

#[test]
fn clap_events_map_to_block_events_and_sim_param_ids() {
    let mut params = ClapParamMap::new();
    params.insert(1000, 7);
    let events = ClapEventBuffer::new(vec![
        ClapEvent::NoteOn {
            time: 0,
            channel: 1,
            key: 60,
            velocity: 0.75,
        },
        ClapEvent::ParamValue {
            time: 4,
            clap_param_id: 1000,
            value: 0.5,
        },
        ClapEvent::MidiShort {
            time: 8,
            bytes: [0x80, 60, 0],
            len: 3,
        },
    ]);
    let mapped = events.to_block_events(&params);
    assert_eq!(
        mapped,
        vec![
            BlockEvent::NoteOn {
                offset: 0,
                channel: 1,
                key: 60,
                velocity: 0.75,
            },
            BlockEvent::ParamSet {
                offset: 4,
                param: 7,
                value: 0.5,
            },
            BlockEvent::Midi {
                offset: 8,
                bytes: [0x80, 60, 0],
                len: 3,
            },
        ]
    );
}

#[test]
fn sim_gain_exports_as_clap_and_hosts_as_graph_processor() {
    let exported = export_gain_as_clap(0.5).unwrap();
    assert_eq!(exported.descriptor().id.format, PluginFormat::Clap);
    let mut hosted = ClapHostProcessor::new(exported);
    let output = process_stereo(
        &mut hosted,
        &[1.0, 0.5, -0.5, -1.0],
        &[-1.0, -0.5, 0.5, 1.0],
        &[],
    );
    assert_eq!(round6(&output[0]), vec![0.5, 0.25, -0.25, -0.5]);
    assert_eq!(round6(&output[1]), vec![-0.5, -0.25, 0.25, 0.5]);
}

#[test]
fn install_clap_plugin_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_clap_plugin_lib,
        &Symbol::new("plugin-clap"),
        &clap_plugin_symbols(),
    );
}

fn process_stereo<P: Processor>(
    processor: &mut P,
    left: &[f32],
    right: &[f32],
    events: &[BlockEvent<'_>],
) -> Vec<Vec<f32>> {
    let frames = if left.is_empty() && right.is_empty() {
        32
    } else {
        left.len().max(right.len())
    };
    let input_storage = if left.is_empty() && right.is_empty() {
        vec![vec![0.0; frames], vec![0.0; frames]]
    } else {
        vec![left.to_vec(), right.to_vec()]
    };
    let inputs: Vec<&[f32]> = input_storage.iter().map(Vec::as_slice).collect();
    processor.prepare(PrepareConfig::new(48_000, frames as u32, 2, 2));
    let mut output = vec![vec![0.0; frames]; 2];
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(Vec::as_mut_slice).collect();
    let mut sink = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(frames * 2);
    let mut block = ProcessBlock {
        frames: frames as u32,
        in_audio: &inputs,
        out_audio: &mut output_refs,
        in_events: events,
        out_events: &mut sink,
        transport: Transport::default(),
        scratch: &mut scratch,
    };
    processor.process(&mut block);
    output
}

fn round6(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .map(|value| (value * 1_000_000.0).round() / 1_000_000.0)
        .collect()
}
