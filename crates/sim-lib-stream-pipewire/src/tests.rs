use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};
use sim_lib_stream_audio::{PcmSampleFormat, PcmSpec};
use sim_lib_stream_core::{BufferPolicy, PushResult, StreamMedia, StreamPacket};
use sim_lib_stream_host::{
    HostBackend, HostBackendRegistry, HostCallbackCassette, HostDirection, HostStreamConfigRequest,
};

use crate::{
    PipeWireBackend, PipeWireCaptureBridge, PipeWireGraphBridge, PipeWireNode, PipeWireTiming,
    install_stream_pipewire_lib, linux_audio_backend_priority, pipewire_backend_symbol,
};

#[derive(Debug)]
struct CopyInput {
    gain: f32,
    prepared: Option<PrepareConfig>,
}

impl Processor for CopyInput {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.prepared = Some(cfg);
    }

    fn reset(&mut self) {
        self.prepared = None;
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for (source, target) in input.iter().zip(output.iter_mut()).take(frames) {
                *target = *source * self.gain;
            }
        }
    }
}

#[test]
fn fake_backend_enumerates_default_nodes_and_visible_sim_ports() {
    let backend = PipeWireBackend::fake();
    let inventory = backend.enumerate().unwrap();

    assert_eq!(inventory.backend(), &pipewire_backend_symbol());
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("pipewire/default/playback")
            && device.direction() == HostDirection::Output
    }));
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("pipewire/default/capture")
            && device.direction() == HostDirection::Input
    }));
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("pipewire/sim/client")
            && device.direction() == HostDirection::Duplex
    }));
    assert!(
        inventory
            .ports()
            .iter()
            .any(|port| port.id() == &Symbol::new("pipewire/sim/client/duplex_0"))
    );
}

#[test]
fn backend_opens_default_nodes_and_maps_timing_metadata() {
    let backend = PipeWireBackend::fake();
    let playback = backend.open_default_playback(8).unwrap();
    assert_eq!(playback.config().backend(), &pipewire_backend_symbol());
    assert_eq!(playback.config().clock().sample_rate_hz(), Some(48_000));
    assert_eq!(playback.config().latency().output_frames(), 128);

    let capture = backend.open_default_capture(8).unwrap();
    assert_eq!(
        capture.config().device(),
        &Symbol::new("pipewire/default/capture")
    );
    assert_eq!(capture.config().latency().input_frames(), 128);
}

#[test]
fn registry_selects_pipewire_by_backend_id() {
    let mut registry = HostBackendRegistry::new();
    registry.register(PipeWireBackend::fake()).unwrap();
    let opened = registry
        .open(HostStreamConfigRequest::new(
            pipewire_backend_symbol(),
            Symbol::new("pipewire/default/playback"),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(4).unwrap(),
        ))
        .unwrap();

    assert_eq!(opened.config().backend(), &pipewire_backend_symbol());
}

#[test]
fn custom_node_quantum_and_latency_reach_open_config() {
    let timing = PipeWireTiming::new(96_000, 64, 32, 96).unwrap();
    let backend = PipeWireBackend::new(vec![
        PipeWireNode::playback(
            "pipewire/custom/playback",
            "PipeWire",
            "Fast Sink",
            2,
            timing,
        )
        .unwrap(),
    ]);
    let opened = backend
        .open(HostStreamConfigRequest::new(
            pipewire_backend_symbol(),
            Symbol::new("pipewire/custom/playback"),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(64).unwrap(),
        ))
        .unwrap();

    assert_eq!(opened.config().clock().sample_rate_hz(), Some(96_000));
    assert_eq!(opened.config().latency().output_frames(), 96);
}

#[test]
fn linux_default_priority_prefers_native_pipewire() {
    let priority = linux_audio_backend_priority();

    assert_eq!(priority[0], pipewire_backend_symbol());
    assert_eq!(priority[1], Symbol::qualified("stream/host", "portaudio"));
    assert_eq!(priority[3], Symbol::qualified("stream/host", "alsa"));
}

#[test]
fn graph_bridge_drives_process_callback_with_input() {
    let spec = PcmSpec::f32(2, 48_000).unwrap();
    let mut bridge = PipeWireGraphBridge::new(
        CopyInput {
            gain: 0.5,
            prepared: None,
        },
        spec,
        2,
        4,
    )
    .unwrap();
    let output = bridge
        .process_interleaved_f32(Some(&[1.0, 0.0, 0.0, -1.0]), 2)
        .unwrap();

    assert_eq!(output, vec![0.5, 0.0, 0.0, -0.5]);
    assert_eq!(bridge.sample_pos(), 2);
}

#[test]
fn capture_bridge_records_callback_timeline_as_pcm() {
    let opened = PipeWireBackend::fake().open_default_capture(4).unwrap();
    let bridge =
        PipeWireCaptureBridge::new(opened.queue().clone(), PcmSpec::f32(2, 48_000).unwrap());

    assert_eq!(
        bridge
            .capture_interleaved_i16(&[i16::MAX, 0, 0, i16::MIN])
            .unwrap(),
        PushResult::Accepted
    );
    let items = opened.queue().drain(4).unwrap();
    let StreamPacket::Pcm(packet) = items[0].packet() else {
        panic!("expected PCM packet");
    };
    assert_eq!(packet.frames(), 2);
    assert_eq!(packet.sample_format(), PcmSampleFormat::F32);
}

#[test]
fn cassette_replays_pipewire_callback_timeline_without_daemon() {
    let opened = PipeWireBackend::fake().open_default_capture(4).unwrap();
    let mut cassette = HostCallbackCassette::new();
    cassette.record_packet(StreamPacket::Pcm(
        sim_lib_stream_core::PcmPacket::f32(1, 2, vec![0.0, 0.25]).unwrap(),
    ));

    cassette.replay(opened.queue()).unwrap();

    assert_eq!(opened.queue().drain(4).unwrap().len(), 1);
}

#[test]
fn install_stream_pipewire_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_stream_pipewire_lib,
        &Symbol::new("stream-pipewire"),
        &[Symbol::qualified("stream", "PipeWireBackend")],
    );
}

#[test]
#[ignore = "hardware smoke test requires an operator-provided PipeWire daemon"]
fn pipewire_hardware_smoke_test_is_ignored_by_default() {
    let backend = PipeWireBackend::default();
    let _nodes = backend.list_nodes();
}
