use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{Graph, PrepareConfig, ProcessBlock, Processor};
use sim_lib_stream_audio::{PcmSampleFormat, PcmSpec};
use sim_lib_stream_core::{BufferPolicy, PcmPacket, PushResult, StreamMedia, StreamPacket};
use sim_lib_stream_host::{
    AudioDeviceCard, AudioPlacementRequest, AudioRouter, AudioSiteKey, HostBackend,
    HostBackendRegistry, HostCallbackCassette, HostDirection, HostStreamConfigRequest,
    ModeledAudioSite,
};

use crate::{
    AlsaBackend, AlsaCaptureBridge, AlsaPcmDevice, AlsaPcmName, AlsaPcmNameKind,
    AlsaPlaybackBridge, alsa_backend_symbol, default_modeled_alsa_site, install_stream_alsa_lib,
};

#[derive(Debug)]
struct ConstantOutput {
    value: f32,
    prepared: Option<PrepareConfig>,
}

impl Processor for ConstantOutput {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.prepared = Some(cfg);
    }

    fn reset(&mut self) {
        self.prepared = None;
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for output in block.out_audio.iter_mut() {
            for sample in output.iter_mut().take(frames) {
                *sample = self.value;
            }
        }
    }
}

#[test]
fn fake_backend_enumerates_default_hw_and_plughw_pcm_devices() {
    let backend = AlsaBackend::fake();
    let inventory = backend.enumerate().unwrap();

    assert_eq!(inventory.backend(), &alsa_backend_symbol());
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("alsa/default/playback")
            && device.direction() == HostDirection::Output
    }));
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("alsa/default/capture")
            && device.direction() == HostDirection::Input
    }));
    assert!(
        inventory
            .devices()
            .iter()
            .any(|device| device.id() == &Symbol::new("alsa/hw:0,0/playback"))
    );
    assert!(
        inventory
            .devices()
            .iter()
            .any(|device| device.id() == &Symbol::new("alsa/plughw:1,0/capture"))
    );
}

#[test]
fn backend_opens_default_playback_and_capture_by_backend_id() {
    let mut registry = HostBackendRegistry::new();
    registry.register(AlsaBackend::fake()).unwrap();
    let backend = registry.backend(&alsa_backend_symbol()).unwrap();
    let inventory = backend.enumerate().unwrap();
    assert_eq!(inventory.devices().len(), 4);

    let playback = registry
        .open(HostStreamConfigRequest::new(
            alsa_backend_symbol(),
            Symbol::new("alsa/default/playback"),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(8).unwrap(),
        ))
        .unwrap();
    assert_eq!(playback.config().backend(), &alsa_backend_symbol());
    assert_eq!(playback.config().clock().sample_rate_hz(), Some(48_000));

    let capture = registry
        .open(HostStreamConfigRequest::new(
            alsa_backend_symbol(),
            Symbol::new("alsa/default/capture"),
            StreamMedia::Pcm,
            HostDirection::Input,
            BufferPolicy::bounded(8).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        capture.config().device(),
        &Symbol::new("alsa/default/capture")
    );
}

#[test]
fn default_open_helpers_use_default_pcm_fallbacks() {
    let backend = AlsaBackend::fake();

    assert_eq!(
        backend.open_default_playback(4).unwrap().config().device(),
        &Symbol::new("alsa/default/playback")
    );
    assert_eq!(
        backend.open_default_capture(4).unwrap().config().device(),
        &Symbol::new("alsa/default/capture")
    );
}

#[test]
fn modeled_alsa_site_placement_round_trip() {
    let mut router = AudioRouter::new();
    router.register(default_modeled_alsa_site());

    let key = AudioSiteKey::new("sim:alsa-modeled");
    assert!(router.site(&key).is_some());
    let opened = router
        .open_placement(AudioPlacementRequest {
            site_key: key,
            stream_request: HostStreamConfigRequest::new(
                alsa_backend_symbol(),
                Symbol::new("alsa/default/playback"),
                StreamMedia::Pcm,
                HostDirection::Output,
                BufferPolicy::bounded(8).unwrap(),
            ),
        })
        .unwrap();

    assert_eq!(opened.config().media(), StreamMedia::Pcm);
    opened.close().unwrap();
}

#[test]
fn same_graph_two_modeled_sites() {
    let mono_key = AudioSiteKey::new("sim:test-mono");
    let stereo_key = AudioSiteKey::new("sim:test-stereo");
    let mut router = AudioRouter::new();
    router.register(Arc::new(ModeledAudioSite::new(
        AudioDeviceCard {
            key: mono_key.clone(),
            display_name: "Modeled Mono".to_owned(),
            channels_out: 1,
            channels_in: 0,
            sample_rates: vec![48_000],
            hardware_required: false,
        },
        Arc::new(AlsaBackend::fake()),
    )));
    router.register(Arc::new(ModeledAudioSite::new(
        AudioDeviceCard {
            key: stereo_key.clone(),
            display_name: "Modeled Stereo".to_owned(),
            channels_out: 2,
            channels_in: 0,
            sample_rates: vec![48_000],
            hardware_required: false,
        },
        Arc::new(AlsaBackend::fake()),
    )));
    assert_eq!(
        router.sites_by_capability(2, &[48_000]),
        vec![stereo_key.clone()]
    );

    let mut graph = make_test_pcm_graph();
    let frames = 3;
    graph.prepare(48_000, frames).unwrap();
    let rendered = graph.process_offline(&[], frames).unwrap();

    for key in [mono_key, stereo_key] {
        let site = router.site(&key).unwrap();
        let expected_channels = usize::from(site.card().channels_out);
        let opened = router
            .open_placement(AudioPlacementRequest {
                site_key: key,
                stream_request: HostStreamConfigRequest::new(
                    alsa_backend_symbol(),
                    Symbol::new("alsa/default/playback"),
                    StreamMedia::Pcm,
                    HostDirection::Output,
                    BufferPolicy::bounded(8).unwrap(),
                ),
            })
            .unwrap();
        let expected_packet =
            packet_from_graph_output(&rendered, expected_channels, frames as usize);

        opened
            .queue()
            .callback_packet(StreamPacket::Pcm(expected_packet.clone()))
            .unwrap();
        let delivered = opened.queue().drain(1).unwrap();
        let StreamPacket::Pcm(packet) = delivered[0].packet() else {
            panic!("expected PCM packet");
        };
        assert_eq!(packet, &expected_packet);
        assert_eq!(packet.channels(), expected_channels);
        opened.close().unwrap();
    }
}

#[test]
fn pcm_name_parser_accepts_documented_alsa_names() {
    let default = AlsaPcmName::parse("default").unwrap();
    let hw = AlsaPcmName::parse("hw:2,0").unwrap();
    let plughw = AlsaPcmName::parse("plughw:USB").unwrap();

    assert_eq!(default.kind(), AlsaPcmNameKind::Default);
    assert_eq!(hw.kind(), AlsaPcmNameKind::Hw);
    assert_eq!(plughw.kind(), AlsaPcmNameKind::PlugHw);
    assert!(AlsaPcmName::parse("pulse").is_err());
    assert!(AlsaPcmName::parse("hw:").is_err());
}

fn make_test_pcm_graph() -> Graph {
    let mut graph = Graph::new();
    graph
        .add_node(
            "constant",
            Box::new(ConstantOutput {
                value: 0.25,
                prepared: None,
            }),
            0,
            2,
        )
        .unwrap();
    graph
}

fn packet_from_graph_output(output: &[Vec<f32>], channels: usize, frames: usize) -> PcmPacket {
    let mut samples = Vec::with_capacity(channels * frames);
    for frame in 0..frames {
        for channel in output.iter().take(channels) {
            samples.push(channel[frame]);
        }
    }
    PcmPacket::f32(channels, frames, samples).unwrap()
}

#[test]
fn explicit_open_rejects_wrong_direction() {
    let backend = AlsaBackend::new(vec![
        AlsaPcmDevice::playback("hw:0,0", "Output", 2, 48_000).unwrap(),
    ]);
    let request = HostStreamConfigRequest::new(
        alsa_backend_symbol(),
        Symbol::new("alsa/hw:0,0/playback"),
        StreamMedia::Pcm,
        HostDirection::Input,
        BufferPolicy::bounded(8).unwrap(),
    );

    assert!(backend.open(request).is_err());
}

#[test]
fn playback_bridge_drives_processor_and_returns_pcm_buffer() {
    let spec = PcmSpec::f32(2, 48_000).unwrap();
    let mut bridge = AlsaPlaybackBridge::new(
        ConstantOutput {
            value: 0.25,
            prepared: None,
        },
        spec,
        8,
    )
    .unwrap();
    let rendered = bridge.render_buffer(3).unwrap();

    assert_eq!(rendered.spec().sample_format(), PcmSampleFormat::F32);
    assert_eq!(rendered.frames(), 3);
    assert_eq!(rendered.samples_f32(), &[0.25; 6]);
    assert_eq!(bridge.sample_pos(), 3);
}

#[test]
fn capture_bridge_records_host_input_as_pcm_packets() {
    let opened = AlsaBackend::fake().open_default_capture(4).unwrap();
    let bridge = AlsaCaptureBridge::new(opened.queue().clone(), PcmSpec::f32(2, 48_000).unwrap());

    assert_eq!(
        bridge
            .capture_interleaved_i16(&[i16::MAX, 0, 0, i16::MIN])
            .unwrap(),
        PushResult::Accepted
    );
    let items = opened.queue().drain(4).unwrap();
    assert_eq!(items.len(), 1);
    let StreamPacket::Pcm(packet) = items[0].packet() else {
        panic!("expected PCM packet");
    };
    assert_eq!(packet.frames(), 2);
    assert_eq!(packet.channels(), 2);
    assert_eq!(packet.sample_format(), PcmSampleFormat::F32);
}

#[test]
fn capture_queue_replays_pcm_cassette_without_hardware() {
    let opened = AlsaBackend::fake().open_default_capture(4).unwrap();
    let mut cassette = HostCallbackCassette::new();
    cassette.record_packet(StreamPacket::Pcm(
        sim_lib_stream_core::PcmPacket::f32(1, 2, vec![0.0, 0.5]).unwrap(),
    ));

    cassette.replay(opened.queue()).unwrap();

    assert_eq!(opened.queue().drain(4).unwrap().len(), 1);
}

#[test]
fn install_stream_alsa_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_stream_alsa_lib,
        &Symbol::new("stream-alsa"),
        &[Symbol::qualified("stream", "AlsaBackend")],
    );
}

#[test]
#[ignore = "hardware smoke test requires an operator-provided ALSA PCM device"]
fn alsa_hardware_smoke_test_is_ignored_by_default() {
    let backend = AlsaBackend::default();
    let _devices = backend.list_devices();
}
