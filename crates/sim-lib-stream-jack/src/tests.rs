use std::sync::{Arc, Mutex};

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};
use sim_lib_stream_audio::PcmSpec;
use sim_lib_stream_clock::ClockChart;
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendRegistry, HostDirection, HostStreamConfigRequest,
};

use crate::{
    JackBackend, JackClient, JackGraphBridge, JackMidiEvent, JackTiming, JackTransportState,
    install_stream_jack_lib, jack_backend_symbol, jack_clock_symbol,
};

#[derive(Clone, Debug, Default)]
struct Recorder {
    state: Arc<Mutex<RecorderState>>,
}

#[derive(Clone, Debug, Default)]
struct RecorderState {
    prepared: Option<PrepareConfig>,
    last_sample_pos: u64,
    last_event_count: usize,
}

impl Processor for Recorder {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.state.lock().expect("recorder lock").prepared = Some(cfg);
    }

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let mut state = self.state.lock().expect("recorder lock");
        state.last_sample_pos = block.transport.sample_pos;
        state.last_event_count = block.in_events.len();
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for (source, target) in input.iter().zip(output.iter_mut()) {
                *target = *source;
            }
        }
    }
}

#[test]
fn fake_backend_enumerates_sim_client_audio_and_midi_ports() {
    let backend = JackBackend::fake();
    let inventory = backend.enumerate().unwrap();

    assert_eq!(inventory.backend(), &jack_backend_symbol());
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("jack/SIM/client")
            && device.direction() == HostDirection::Duplex
    }));
    assert!(
        inventory
            .ports()
            .iter()
            .any(|port| port.id() == &Symbol::new("jack/SIM/audio_in_0")
                && port.media() == StreamMedia::Pcm)
    );
    assert!(
        inventory
            .ports()
            .iter()
            .any(|port| port.id() == &Symbol::new("jack/SIM/midi_out_0")
                && port.media() == StreamMedia::Midi)
    );
}

#[test]
fn fake_backend_capabilities_include_audio_midi_and_duplex() {
    let backend = JackBackend::fake();
    let capabilities = backend.info().capabilities();

    assert!(capabilities.contains(&HostBackendCapability::AudioInput));
    assert!(capabilities.contains(&HostBackendCapability::AudioOutput));
    assert!(capabilities.contains(&HostBackendCapability::MidiInput));
    assert!(capabilities.contains(&HostBackendCapability::MidiOutput));
    assert!(capabilities.contains(&HostBackendCapability::Duplex));
}

#[test]
fn backend_opens_sim_client_and_maps_jack_clock_metadata() {
    let opened = JackBackend::fake().open_sim_client(16).unwrap();

    assert_eq!(opened.config().backend(), &jack_backend_symbol());
    assert_eq!(opened.config().device(), &Symbol::new("jack/SIM/client"));
    assert_eq!(opened.config().clock().clock(), &jack_clock_symbol());
    assert_eq!(opened.config().clock().sample_rate_hz(), Some(48_000));
    assert_eq!(opened.config().latency().input_frames(), 128);
    assert_eq!(opened.config().latency().output_frames(), 128);
}

#[test]
fn registry_selects_jack_by_backend_id() {
    let mut registry = HostBackendRegistry::new();
    registry.register(JackBackend::fake()).unwrap();
    let opened = registry
        .open(HostStreamConfigRequest::new(
            jack_backend_symbol(),
            Symbol::new("jack/SIM/client"),
            StreamMedia::Pcm,
            HostDirection::Duplex,
            BufferPolicy::bounded(4).unwrap(),
        ))
        .unwrap();

    assert_eq!(opened.config().backend(), &jack_backend_symbol());
}

#[test]
fn custom_client_ports_and_timing_reach_inventory_and_open_config() {
    let timing = JackTiming::new(96_000, 64, 32, 96).unwrap();
    let backend = JackBackend::new(vec![JackClient::new("SIM", timing, 1, 2, 0, 1).unwrap()]);
    let inventory = backend.enumerate().unwrap();
    let opened = backend.open_sim_client(64).unwrap();

    assert_eq!(inventory.devices()[0].buffer().capacity(), 64);
    assert_eq!(opened.config().clock().sample_rate_hz(), Some(96_000));
    assert_eq!(opened.config().latency().output_frames(), 96);
    assert!(
        inventory
            .ports()
            .iter()
            .any(|port| port.id() == &Symbol::new("jack/SIM/midi_out_0"))
    );
}

#[test]
fn transport_snapshot_maps_to_graph_transport_and_stream_clock() {
    let transport = JackTransportState::rolling(2048, 132.0, 16.5).unwrap();
    let graph_transport = transport.to_graph_transport();
    let clock = JackTiming::pro_audio_default().frame_clock().unwrap();

    assert!(graph_transport.playing);
    assert_eq!(graph_transport.sample_pos, 2048);
    assert_eq!(graph_transport.tempo_bpm, 132.0);
    assert_eq!(clock.id(), &jack_clock_symbol());
    assert_eq!(
        clock.chart(),
        &ClockChart::Frames {
            frames_per_second: 48_000
        }
    );
}

#[test]
fn graph_bridge_drives_process_callback_with_transport_and_midi() {
    let recorder = Recorder::default();
    let state = Arc::clone(&recorder.state);
    let mut bridge =
        JackGraphBridge::new(recorder, PcmSpec::f32(2, 48_000).unwrap(), 2, 4).unwrap();
    let midi = [JackMidiEvent::short(1, &[0x90, 60, 100]).unwrap()];
    let output = bridge
        .process_interleaved_f32(
            Some(&[1.0, 0.0, 0.0, -1.0]),
            2,
            JackTransportState::rolling(4096, 120.0, 32.0).unwrap(),
            &midi,
        )
        .unwrap();
    let state = state.lock().expect("recorder state");

    assert_eq!(output, vec![1.0, 0.0, 0.0, -1.0]);
    assert_eq!(bridge.last_transport().sample_pos(), 4096);
    assert_eq!(state.last_sample_pos, 4096);
    assert_eq!(state.last_event_count, 1);
    assert_eq!(state.prepared, Some(PrepareConfig::new(48_000, 4, 2, 2)));
}

#[test]
fn install_stream_jack_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_stream_jack_lib,
        &Symbol::new("stream-jack"),
        &[Symbol::qualified("stream", "JackBackend")],
    );
}

#[test]
#[ignore = "hardware smoke test requires an operator-provided JACK server"]
fn jack_hardware_smoke_test_is_ignored_by_default() {
    let backend = JackBackend::default();
    let _clients = backend.list_clients();
}
