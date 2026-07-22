use sim_kernel::Result;
use sim_lib_stream_core::DomainBridgeKind;

use crate::{
    BlockArena, BlockEvent, ClockDomain, DomainBridgeMetadataProcessor, EventSink, LatencyClass,
    NullEventSink, PortDir, PortMedia, ProcessBlock, Processor, Transport,
};

#[test]
fn metadata_processor_declares_sample_rate_change_contract_and_diagnostics() {
    let bridge =
        DomainBridgeMetadataProcessor::sample_rate_bridge_metadata(48_000, 96_000).unwrap();
    let descriptor = bridge.descriptor(2, 2);
    let bridge_descriptor = bridge.bridge_descriptor();

    assert_eq!(bridge_descriptor.name(), "resampler");
    assert_eq!(bridge_descriptor.kind(), DomainBridgeKind::Resampler);
    assert_eq!(bridge_descriptor.latency().frame_count(), 32);
    assert_eq!(
        bridge_descriptor.diagnostics(),
        &[DomainBridgeKind::Resampler.diagnostic_symbol()]
    );
    assert_eq!(bridge.tail_frames(), 0);
    assert_eq!(descriptor.clock_domain(), ClockDomain::Sample);
    assert_eq!(descriptor.latency_class(), LatencyClass::SampleExact);
    assert!(!descriptor.realtime_pin());

    let ports = descriptor.ports();
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0].name, "in");
    assert_eq!(ports[0].media, PortMedia::Audio);
    assert_eq!(ports[0].dir, PortDir::In);
    assert_eq!(ports[0].channels, 2);
    assert_eq!(ports[0].rate_contract.nominal_rate_hz(), Some(48_000));
    assert_eq!(ports[1].name, "out");
    assert_eq!(ports[1].media, PortMedia::Audio);
    assert_eq!(ports[1].dir, PortDir::Out);
    assert_eq!(ports[1].channels, 2);
    assert_eq!(ports[1].rate_contract.nominal_rate_hz(), Some(96_000));
}

#[test]
fn audio_latency_metadata_reports_contract_but_has_no_executable_tail() {
    let mut bridge = DomainBridgeMetadataProcessor::audio_latency_bridge_metadata(96);
    assert_eq!(bridge.bridge_descriptor().latency().frame_count(), 96);
    assert_eq!(bridge.tail_frames(), 0);

    let left_in = [0.25, -0.5, 0.75];
    let right_in = [1.0, 0.5, -1.0];
    let mut left_out = [0.0; 3];
    let mut right_out = [0.0; 3];
    let in_audio: [&[f32]; 2] = [&left_in, &right_in];
    let mut out_audio: [&mut [f32]; 2] = [&mut left_out, &mut right_out];
    let in_events = [];
    let mut out_events = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(8);
    let mut block = ProcessBlock {
        frames: 3,
        in_audio: &in_audio,
        out_audio: &mut out_audio,
        in_events: &in_events,
        out_events: &mut out_events,
        transport: Transport::default(),
        scratch: &mut scratch,
    };

    bridge.process(&mut block);

    // This is a metadata node, not a delay line: executable behavior is passthrough.
    assert_eq!(left_out, left_in);
    assert_eq!(right_out, right_in);
}

#[test]
fn event_block_gate_metadata_declares_control_to_block_ports_and_forwards_events() {
    let mut bridge =
        DomainBridgeMetadataProcessor::event_block_gate_metadata(ClockDomain::MidiTick).unwrap();
    let descriptor = bridge.descriptor(1, 1);
    let bridge_descriptor = bridge.bridge_descriptor();

    assert_eq!(bridge_descriptor.name(), "event-rate-gate");
    assert_eq!(bridge_descriptor.latency().frame_count(), 0);
    assert_eq!(bridge_descriptor.latency().packet_count(), 0);
    assert_eq!(descriptor.clock_domain(), ClockDomain::Block);
    assert_eq!(descriptor.latency_class(), LatencyClass::BlockLocal);
    let ports = descriptor.ports();
    assert_eq!(ports[0].media, PortMedia::Event);
    assert_eq!(ports[0].rate_contract.clock_domain(), ClockDomain::MidiTick);
    assert_eq!(ports[1].media, PortMedia::Event);
    assert_eq!(ports[1].rate_contract.clock_domain(), ClockDomain::Block);

    let in_audio: [&[f32]; 0] = [];
    let mut out_audio: [&mut [f32]; 0] = [];
    let in_events = [BlockEvent::NoteOn {
        offset: 2,
        channel: 1,
        key: 64,
        velocity: 0.5,
    }];
    let mut out_events = CapturingEventSink::default();
    let mut scratch = BlockArena::empty();
    let mut block = ProcessBlock {
        frames: 4,
        in_audio: &in_audio,
        out_audio: &mut out_audio,
        in_events: &in_events,
        out_events: &mut out_events,
        transport: Transport::default(),
        scratch: &mut scratch,
    };

    bridge.process(&mut block);

    assert_eq!(out_events.note_on, Some((2, 1, 64, 0.5)));
}

#[test]
fn control_rate_gate_declares_control_stream_ports() {
    let bridge =
        DomainBridgeMetadataProcessor::control_block_gate_metadata(ClockDomain::Control).unwrap();
    let descriptor = bridge.descriptor(1, 1);
    let ports = descriptor.ports();

    assert_eq!(descriptor.clock_domain(), ClockDomain::Block);
    assert_eq!(ports[0].media, PortMedia::Control);
    assert_eq!(ports[0].dir, PortDir::In);
    assert_eq!(ports[0].rate_contract.clock_domain(), ClockDomain::Control);
    assert_eq!(ports[1].media, PortMedia::Control);
    assert_eq!(ports[1].dir, PortDir::Out);
    assert_eq!(ports[1].rate_contract.clock_domain(), ClockDomain::Block);
}

#[test]
fn jitter_bridge_metadata_declares_packet_latency_and_forwards_events() {
    let mut bridge = DomainBridgeMetadataProcessor::event_jitter_bridge_metadata(2);
    let descriptor = bridge.descriptor(1, 1);
    let bridge_descriptor = bridge.bridge_descriptor();

    assert_eq!(bridge_descriptor.name(), "jitter-buffer");
    assert_eq!(bridge_descriptor.kind(), DomainBridgeKind::JitterBuffer);
    assert_eq!(bridge_descriptor.latency().packet_count(), 2);
    assert_eq!(bridge.tail_frames(), 0);
    let ports = descriptor.ports();
    assert_eq!(ports[0].media, PortMedia::Event);
    assert_eq!(ports[0].rate_contract.clock_domain(), ClockDomain::Wall);
    assert_eq!(ports[1].media, PortMedia::Event);
    assert_eq!(ports[1].rate_contract.clock_domain(), ClockDomain::Wall);

    let in_audio: [&[f32]; 0] = [];
    let mut out_audio: [&mut [f32]; 0] = [];
    let in_events = [BlockEvent::NoteOff {
        offset: 1,
        channel: 1,
        key: 64,
        velocity: 0.25,
    }];
    let mut out_events = CapturingEventSink::default();
    let mut scratch = BlockArena::empty();
    let mut block = ProcessBlock {
        frames: 4,
        in_audio: &in_audio,
        out_audio: &mut out_audio,
        in_events: &in_events,
        out_events: &mut out_events,
        transport: Transport::default(),
        scratch: &mut scratch,
    };

    bridge.process(&mut block);

    assert_eq!(out_events.note_off, Some((1, 1, 64, 0.25)));
}

#[derive(Default)]
struct CapturingEventSink {
    note_on: Option<(u32, u8, u8, f32)>,
    note_off: Option<(u32, u8, u8, f32)>,
}

impl EventSink for CapturingEventSink {
    fn push(&mut self, event: BlockEvent<'_>) -> Result<()> {
        if let BlockEvent::NoteOn {
            offset,
            channel,
            key,
            velocity,
        } = event
        {
            self.note_on = Some((offset, channel, key, velocity));
        }
        if let BlockEvent::NoteOff {
            offset,
            channel,
            key,
            velocity,
        } = event
        {
            self.note_off = Some((offset, channel, key, velocity));
        }
        Ok(())
    }
}
