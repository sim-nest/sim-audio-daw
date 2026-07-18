use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, BlockEvent, NullEventSink, PortDecl, PortDir, PortMedia, PrepareConfig,
    ProcessBlock, Processor, Transport,
};
use sim_lib_plugin_core::{HostedPluginProcessor, PluginFormat, PluginInstance};

use crate::{
    Vst3Event, Vst3EventBuffer, Vst3HostingDecision, Vst3ParamMap, current_vst3_scope,
    export_gain_as_vst3, install_vst3_plugin_lib, vst3_gain_descriptor, vst3_gain_vst3_descriptor,
    vst3_plugin_symbols,
};

#[test]
fn vst3_descriptor_maps_buses_and_params_to_plugin_core() {
    let vst3 = vst3_gain_vst3_descriptor().unwrap();
    assert_eq!(
        vst3.port_decls(),
        vec![
            PortDecl::new("audio-in", PortMedia::Audio, PortDir::In, 2),
            PortDecl::new("audio-out", PortMedia::Audio, PortDir::Out, 2),
            PortDecl::new("events-in", PortMedia::Event, PortDir::In, 1),
        ]
    );
    let descriptor = vst3.to_plugin_descriptor().unwrap();
    assert_eq!(descriptor.id.format, PluginFormat::Vst3);
    assert_eq!(descriptor.parameters[0].id, 0);
    assert_eq!(descriptor.parameters[0].stable_id, "gain");
}

#[test]
fn vst3_events_map_to_block_events_and_sim_param_ids() {
    let mut params = Vst3ParamMap::new();
    params.insert(100, 7);
    let events = Vst3EventBuffer::new(vec![
        Vst3Event::NoteOn {
            sample_offset: 0,
            channel: 1,
            pitch: 60,
            velocity: 0.75,
        },
        Vst3Event::ParamValue {
            sample_offset: 4,
            vst3_param_id: 100,
            normalized: 0.5,
        },
        Vst3Event::Midi {
            sample_offset: 8,
            bytes: [0x80, 60, 0],
            len: 3,
        },
    ]);
    assert_eq!(
        events.to_block_events(&params),
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
fn vst3_checked_event_conversion_rejects_end_of_block_offsets() {
    let params = Vst3ParamMap::new();
    let accepted = Vst3Event::Midi {
        sample_offset: 7,
        bytes: [0x90, 60, 100],
        len: 3,
    };
    assert_eq!(
        accepted.try_to_block_event(&params, 8).unwrap(),
        BlockEvent::Midi {
            offset: 7,
            bytes: [0x90, 60, 100],
            len: 3,
        }
    );

    let rejected = Vst3EventBuffer::new(vec![Vst3Event::ParamValue {
        sample_offset: 8,
        vst3_param_id: 100,
        normalized: 0.5,
    }]);
    let err = rejected
        .try_to_block_events(&params, 8)
        .expect_err("offset equal to frames is outside the block");

    assert!(err.to_string().contains("VST3 event offset 8"));
    assert!(err.to_string().contains("outside block frames 0..8"));
}

#[test]
fn sim_gain_exports_as_vst3_and_runs_via_core_host_processor() {
    let exported = export_gain_as_vst3(0.75).unwrap();
    assert_eq!(exported.descriptor().id.format, PluginFormat::Vst3);
    let mut hosted = HostedPluginProcessor::new(exported);
    let output = process_stereo(
        &mut hosted,
        &[1.0, 0.5, -0.5, -1.0],
        &[-1.0, -0.5, 0.5, 1.0],
    );
    assert_eq!(round6(&output[0]), vec![0.75, 0.375, -0.375, -0.75]);
    assert_eq!(round6(&output[1]), vec![-0.75, -0.375, 0.375, 0.75]);
}

#[test]
fn vst3_scope_documents_native_sdk_and_hosting_decisions() {
    let scope = current_vst3_scope();
    assert!(!scope.native_export_supported());
    assert!(scope.native_export_blocker.contains("Steinberg VST3 SDK"));
    assert_eq!(scope.hosting, Vst3HostingDecision::Deferred);
    assert!(
        scope
            .sdk_requirements
            .iter()
            .any(|requirement| requirement == "host validator or smoke host")
    );
}

#[test]
fn install_vst3_plugin_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_vst3_plugin_lib,
        &Symbol::new("plugin-vst3"),
        &vst3_plugin_symbols(),
    );
}

#[test]
#[ignore = "requires the Steinberg VST3 SDK and a platform VST3 validator"]
fn vst3_native_bundle_smoke_fixture_is_blocked_until_sdk_is_available() {
    let descriptor = vst3_gain_descriptor().unwrap();
    assert_eq!(descriptor.id.stable_id, "53494d2d4741494e2d56535433000001");
}

fn process_stereo<P: Processor>(processor: &mut P, left: &[f32], right: &[f32]) -> Vec<Vec<f32>> {
    let frames = left.len().max(right.len());
    let input_storage = [left.to_vec(), right.to_vec()];
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
        in_events: &[],
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
