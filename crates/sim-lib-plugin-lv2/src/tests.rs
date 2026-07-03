use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PortDecl, PortDir, PortMedia, PrepareConfig, ProcessBlock,
    Processor, Transport,
};
use sim_lib_plugin_core::{PluginFormat, PluginInstance, PluginState};

use crate::{
    Lv2HostProcessor, Lv2Port, Lv2PortKind, Lv2StatePatch, export_gain_as_lv2,
    install_lv2_plugin_lib, lv2_gain_descriptor, lv2_gain_lv2_descriptor, lv2_plugin_symbols,
};

#[test]
fn lv2_ports_map_to_graph_port_decls_and_parameters() {
    let lv2 = lv2_gain_lv2_descriptor().unwrap();
    assert_eq!(
        lv2.port_decls(),
        vec![
            PortDecl::new("audio-in", PortMedia::Audio, PortDir::In, 2),
            PortDecl::new("audio-out", PortMedia::Audio, PortDir::Out, 2),
            PortDecl::new("gain", PortMedia::Control, PortDir::In, 1),
        ]
    );
    let descriptor = lv2.to_plugin_descriptor().unwrap();
    assert_eq!(descriptor.id.format, PluginFormat::Lv2);
    assert_eq!(descriptor.parameters[0].id, 2);
    assert_eq!(descriptor.parameters[0].stable_id, "gain");
}

#[test]
fn lv2_atom_ports_map_to_event_lanes() {
    let port = Lv2Port::atom_input(3, "events").unwrap();
    assert_eq!(port.kind, Lv2PortKind::AtomSequence);
    assert_eq!(
        port.to_port_decl(),
        PortDecl::new("events", PortMedia::Event, PortDir::In, 1)
    );
}

#[test]
fn lv2_state_maps_to_patch_expr() {
    let mut state = PluginState::new();
    state.set_param(2, 0.75);
    state.insert_data("preset", Expr::String("wide".to_owned()));

    let patch = Lv2StatePatch::new("https://sim.dev/lv2/gain", state.clone()).unwrap();
    let parsed = Lv2StatePatch::from_expr(&patch.to_expr()).unwrap();

    assert_eq!(parsed.plugin_uri(), "https://sim.dev/lv2/gain");
    assert_eq!(parsed.state(), &state);
}

#[test]
fn sim_gain_exports_as_lv2_and_hosts_as_graph_processor() {
    let exported = export_gain_as_lv2(0.25).unwrap();
    assert_eq!(exported.descriptor().id.format, PluginFormat::Lv2);
    let mut hosted = Lv2HostProcessor::new(exported);
    let output = process_stereo(
        &mut hosted,
        &[1.0, 0.5, -0.5, -1.0],
        &[-1.0, -0.5, 0.5, 1.0],
    );
    assert_eq!(round6(&output[0]), vec![0.25, 0.125, -0.125, -0.25]);
    assert_eq!(round6(&output[1]), vec![-0.25, -0.125, 0.125, 0.25]);
}

#[test]
fn install_lv2_plugin_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_lv2_plugin_lib,
        &Symbol::new("plugin-lv2"),
        &lv2_plugin_symbols(),
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_lv2_plugin_smoke_fixture_is_available() {
    let descriptor = lv2_gain_descriptor().unwrap();
    assert_eq!(descriptor.id.stable_id, "https://sim.dev/lv2/gain");
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
