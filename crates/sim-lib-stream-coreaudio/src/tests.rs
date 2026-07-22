use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};
use sim_lib_stream_core::{BufferPolicy, StreamMedia, StreamPacket};
use sim_lib_stream_host::{
    HostBackend, HostBackendRegistry, HostCallbackCassette, HostDirection, HostStreamConfigRequest,
};

use crate::{
    CoreAudioBackend, CoreAudioDevice, CoreAudioRenderBridge, CoreAudioTiming,
    coreaudio_audio_backend_candidate, coreaudio_backend_symbol, coreaudio_clock_symbol,
    install_stream_coreaudio_lib, macos_audio_backend_priority, macos_midi_backend_priority,
};

#[derive(Debug)]
struct CopyInput {
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
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for (source, target) in input.iter().zip(output.iter_mut()) {
                *target = *source;
            }
        }
    }
}

#[test]
fn fake_backend_enumerates_default_input_and_output() {
    let backend = CoreAudioBackend::fake();
    let inventory = backend.enumerate().unwrap();

    assert_eq!(inventory.backend(), &coreaudio_backend_symbol());
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("coreaudio/default-output")
            && device.direction() == HostDirection::Output
    }));
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("coreaudio/default-input")
            && device.direction() == HostDirection::Input
    }));
}

#[test]
fn config_probe_candidate_names_coreaudio_backend() {
    assert_eq!(coreaudio_audio_backend_candidate(), "coreaudio");
    assert_eq!(
        coreaudio_backend_symbol().name.as_ref(),
        coreaudio_audio_backend_candidate()
    );
}

#[test]
fn registry_opens_default_coreaudio_devices_by_backend_id() {
    let mut registry = HostBackendRegistry::new();
    registry.register(CoreAudioBackend::fake()).unwrap();
    let output = registry
        .open(HostStreamConfigRequest::new(
            coreaudio_backend_symbol(),
            Symbol::new("coreaudio/default-output"),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(8).unwrap(),
        ))
        .unwrap();

    assert_eq!(output.config().clock().clock(), &coreaudio_clock_symbol());
    assert_eq!(output.config().clock().sample_rate_hz(), Some(48_000));
}

#[test]
fn custom_device_timing_reaches_open_config() {
    let timing = CoreAudioTiming::new(44_100, 64, 24, 32).unwrap();
    let backend = CoreAudioBackend::new(vec![
        CoreAudioDevice::output("coreaudio/vendor/out", "Vendor Out", 2, timing).unwrap(),
    ]);
    let opened = backend
        .open(HostStreamConfigRequest::new(
            coreaudio_backend_symbol(),
            Symbol::new("coreaudio/vendor/out"),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(4).unwrap(),
        ))
        .unwrap();

    assert_eq!(opened.config().clock().sample_rate_hz(), Some(44_100));
    assert_eq!(opened.config().latency().output_frames(), 32);
}

#[test]
fn render_bridge_drives_same_graph_processor_code() {
    let mut bridge = CoreAudioRenderBridge::new(
        CopyInput { prepared: None },
        CoreAudioTiming::default_low_latency(),
        2,
        2,
    )
    .unwrap();
    let left = [1.0, 0.5];
    let right = [0.0, -1.0];
    let output = bridge.render_planar_f32(Some(&[&left, &right]), 2).unwrap();

    assert_eq!(output[0], vec![1.0, 0.5]);
    assert_eq!(output[1], vec![0.0, -1.0]);
    assert_eq!(bridge.sample_pos(), 2);
}

#[test]
fn callback_cassette_replays_fake_coreaudio_queue_without_hardware() {
    let opened = CoreAudioBackend::fake().open_default_output(4).unwrap();
    let mut cassette = HostCallbackCassette::new();
    cassette.record_packet(StreamPacket::Pcm(
        sim_lib_stream_core::PcmPacket::f32(1, 2, vec![0.0, 0.5]).unwrap(),
    ));

    for item in cassette.items() {
        opened.stream().push_packet(item.clone()).unwrap();
    }

    assert_eq!(opened.queue().drain(4).unwrap().len(), 1);
}

#[test]
fn macos_priorities_keep_portable_audio_and_rtmidi_first() {
    let audio = macos_audio_backend_priority();
    let midi = macos_midi_backend_priority();

    assert_eq!(audio[0], Symbol::qualified("stream/host", "portaudio"));
    assert_eq!(audio[2], coreaudio_backend_symbol());
    assert_eq!(midi[0], Symbol::qualified("stream/host", "rtmidi"));
}

#[test]
fn install_stream_coreaudio_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_stream_coreaudio_lib,
        &Symbol::new("stream-coreaudio"),
        &[Symbol::qualified("stream", "CoreAudioBackend")],
    );
}

#[test]
#[ignore = "hardware smoke test requires an operator-provided CoreAudio device"]
fn coreaudio_hardware_smoke_test_is_ignored_by_default() {
    let backend = CoreAudioBackend::default();
    let _devices = backend.list_devices();
}
