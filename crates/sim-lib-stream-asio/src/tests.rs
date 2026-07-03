use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};
use sim_lib_stream_core::{BufferPolicy, StreamMedia, StreamPacket};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendRegistry, HostCallbackCassette, HostDirection,
    HostStreamConfigRequest,
};

use crate::{
    AsioBackend, AsioBufferSwitchBridge, AsioDriver, AsioTiming, asio_backend_symbol,
    asio_clock_symbol, asio_sdk_build_requirements, install_stream_asio_lib,
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
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for (source, target) in input.iter().zip(output.iter_mut()) {
                *target = *source * self.gain;
            }
        }
    }
}

#[test]
fn fake_backend_enumerates_sim_asio_driver_and_ports() {
    let backend = AsioBackend::fake();
    let inventory = backend.enumerate().unwrap();

    assert_eq!(inventory.backend(), &asio_backend_symbol());
    assert!(inventory.devices().iter().any(|device| {
        device.id() == &Symbol::new("asio/SIM-ASIO/driver")
            && device.direction() == HostDirection::Duplex
    }));
    assert!(
        inventory
            .ports()
            .iter()
            .any(|port| port.id() == &Symbol::new("asio/SIM-ASIO/driver/output_0"))
    );
}

#[test]
fn fake_backend_capabilities_include_duplex_and_offline() {
    let backend = AsioBackend::fake();
    let capabilities = backend.info().capabilities();

    assert!(capabilities.contains(&HostBackendCapability::AudioInput));
    assert!(capabilities.contains(&HostBackendCapability::AudioOutput));
    assert!(capabilities.contains(&HostBackendCapability::Duplex));
    assert!(capabilities.contains(&HostBackendCapability::Offline));
    assert!(capabilities.contains(&HostBackendCapability::Fake));
}

#[test]
fn registry_opens_sim_driver_by_backend_id() {
    let mut registry = HostBackendRegistry::new();
    registry.register(AsioBackend::fake()).unwrap();
    let opened = registry
        .open(HostStreamConfigRequest::new(
            asio_backend_symbol(),
            Symbol::new("asio/SIM-ASIO/driver"),
            StreamMedia::Pcm,
            HostDirection::Duplex,
            BufferPolicy::bounded(8).unwrap(),
        ))
        .unwrap();

    assert_eq!(opened.config().backend(), &asio_backend_symbol());
    assert_eq!(opened.config().clock().clock(), &asio_clock_symbol());
    assert_eq!(opened.config().clock().sample_rate_hz(), Some(48_000));
}

#[test]
fn custom_driver_timing_reaches_open_config() {
    let timing = AsioTiming::new(96_000, 64, 32, 96).unwrap();
    let backend = AsioBackend::new(vec![AsioDriver::new("Vendor", timing, 1, 2).unwrap()]);
    let opened = backend
        .open(HostStreamConfigRequest::new(
            asio_backend_symbol(),
            Symbol::new("asio/Vendor/driver"),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(4).unwrap(),
        ))
        .unwrap();

    assert_eq!(opened.config().clock().sample_rate_hz(), Some(96_000));
    assert_eq!(opened.config().latency().output_frames(), 96);
}

#[test]
fn buffer_switch_bridge_drives_same_graph_processor_code() {
    let mut bridge = AsioBufferSwitchBridge::new(
        CopyGain {
            gain: 0.25,
            prepared: None,
        },
        AsioTiming::pro_audio_default(),
        2,
        2,
    )
    .unwrap();
    let left = [1.0, 0.5];
    let right = [0.0, -1.0];
    let output = bridge
        .process_planar_f32(Some(&[&left, &right]), 2)
        .unwrap();

    assert_eq!(output[0], vec![0.25, 0.125]);
    assert_eq!(output[1], vec![0.0, -0.25]);
    assert_eq!(bridge.sample_pos(), 2);
}

#[test]
fn callback_cassette_replays_fake_asio_queue_without_hardware() {
    let opened = AsioBackend::fake().open_sim_driver(4).unwrap();
    let mut cassette = HostCallbackCassette::new();
    cassette.record_packet(StreamPacket::Pcm(
        sim_lib_stream_core::PcmPacket::f32(1, 2, vec![0.0, 0.5]).unwrap(),
    ));

    cassette.replay(opened.queue()).unwrap();

    assert_eq!(opened.queue().drain(4).unwrap().len(), 1);
}

#[test]
fn sdk_requirements_document_optional_native_build() {
    let requirements = asio_sdk_build_requirements();

    assert!(requirements.iter().any(|item| item.contains("ASIO SDK")));
    assert!(requirements.iter().any(|item| item.contains("stream-asio")));
}

#[test]
fn install_stream_asio_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_stream_asio_lib,
        &Symbol::new("stream-asio"),
        &[Symbol::qualified("stream", "AsioBackend")],
    );
}

#[test]
#[ignore = "hardware smoke test requires an operator-provided Windows ASIO driver"]
fn asio_hardware_smoke_test_is_ignored_by_default() {
    let backend = AsioBackend::default();
    let _drivers = backend.list_drivers();
}
