use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};
use sim_lib_stream_audio::{PcmSampleFormat, PcmSpec};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{HostBackend, HostDirection, HostStreamConfigRequest};

use crate::{
    PortAudioBackend, PortAudioCallbackBridge, PortAudioDevice, PortAudioHostBuffer,
    install_stream_portaudio_lib, portaudio_backend_priority, portaudio_backend_symbol,
    test_tone_buffer,
};

#[derive(Debug)]
struct CopyGain {
    gain: f32,
    prepared: Option<PrepareConfig>,
}

impl Processor for CopyGain {
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
fn fake_backend_lists_and_opens_default_output_without_portaudio() {
    let backend = PortAudioBackend::fake();
    let inventory = backend.enumerate().unwrap();

    assert_eq!(inventory.backend(), &portaudio_backend_symbol());
    assert!(inventory.devices().iter().any(|device| {
        device.media() == StreamMedia::Pcm && device.direction() == HostDirection::Output
    }));

    let opened = backend.open_default_output(8).unwrap();
    assert_eq!(
        opened.config().device(),
        &Symbol::new("portaudio/default-output")
    );
}

#[test]
fn explicit_open_validates_backend_media_and_direction() {
    let backend = PortAudioBackend::new(vec![
        PortAudioDevice::output("portaudio/out", "Output", 2, 48_000).unwrap(),
    ]);
    let request = HostStreamConfigRequest::new(
        portaudio_backend_symbol(),
        Symbol::new("portaudio/out"),
        StreamMedia::Pcm,
        HostDirection::Output,
        BufferPolicy::bounded(64).unwrap(),
    );
    let opened = backend.open(request).unwrap();

    assert_eq!(opened.config().clock().sample_rate_hz(), Some(48_000));
}

#[test]
fn callback_bridge_converts_i16_host_input_to_process_block_f32() {
    let spec = PcmSpec::f32(2, 48_000).unwrap();
    let mut bridge = PortAudioCallbackBridge::new(
        CopyGain {
            gain: 0.5,
            prepared: None,
        },
        spec,
        2,
        4,
    )
    .unwrap();
    let output = bridge
        .process_interleaved(
            Some(&PortAudioHostBuffer::I16(vec![i16::MAX, 0, 0, i16::MIN])),
            2,
        )
        .unwrap();

    assert_eq!(output.len(), 4);
    assert!((output[0] - 0.5).abs() < 0.0001);
    assert_eq!(output[1], 0.0);
    assert!(output[3] < -0.49);
    assert_eq!(bridge.sample_pos(), 2);
}

#[test]
fn test_tone_plan_uses_default_output_request_and_f32_preview() {
    let backend = PortAudioBackend::fake();
    let plan = backend.test_tone_plan(16, 440.0).unwrap();

    assert_eq!(plan.device(), &Symbol::new("portaudio/default-output"));
    assert_eq!(plan.request().media(), StreamMedia::Pcm);
    assert_eq!(plan.preview().spec().sample_format(), PcmSampleFormat::F32);
    assert_eq!(plan.preview().frames(), 16);
}

#[test]
fn sample_conversion_and_test_tone_support_i16_output_format() {
    let spec = PcmSpec::i16(1, 48_000).unwrap();
    let tone = test_tone_buffer(spec, 4, 440.0, 0.25).unwrap();

    assert_eq!(tone.spec().sample_format(), PcmSampleFormat::I16);
    assert_eq!(tone.samples_i16().len(), 4);
}

#[test]
fn backend_priority_documents_plain_ubuntu_fallback_order() {
    let priority = portaudio_backend_priority();

    assert_eq!(priority[0], Symbol::qualified("stream/host", "pipewire"));
    assert_eq!(priority[1], portaudio_backend_symbol());
    assert_eq!(priority[2], Symbol::qualified("stream/host", "rtaudio"));
    assert_eq!(priority[3], Symbol::qualified("stream/host", "alsa"));
}

#[test]
fn install_stream_portaudio_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_stream_portaudio_lib,
        &Symbol::new("stream-portaudio"),
        &[Symbol::qualified("stream", "PortAudioBackend")],
    );
}

#[test]
#[ignore = "hardware smoke test requires an operator-provided PortAudio default output"]
fn portaudio_hardware_smoke_test_is_ignored_by_default() {
    let backend = PortAudioBackend::default();
    let _devices = backend.list_devices();
}
