use std::sync::{Arc, Mutex};

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{BlockEvent, PrepareConfig, ProcessBlock, Processor};
use sim_lib_stream_audio::PcmSpec;
use sim_lib_stream_clock::ClockChart;
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, LatencyClass, MidiPacket, MidiPacketEvent, PcmPacket,
    StreamCapability, StreamDirection, StreamInspectorStatus, StreamMedia, StreamPacket,
    TransportProfile,
};
use sim_lib_stream_host::{
    FakeBackend, HostBackend, HostDirection, HostStreamConfigRequest, fake_backend_symbol,
};

use crate::{
    LanBufferedPreviewWindow, LiveAudioEvent, LiveGraphConfig, LiveGraphRunner, LiveQueuePush,
    LiveStreamLane, LiveTransportClock, install_audio_graph_live_lib,
    lan_buffered_preview_drop_diagnostic_kind, lan_buffered_preview_jitter_diagnostic_kind,
    lan_buffered_preview_late_packet_diagnostic_kind, lan_buffered_preview_reorder_diagnostic_kind,
    live_clock_symbol, realtime_local_audio_profile, refuse_unbuffered_audio_callback_tunnel,
};

#[derive(Clone, Debug)]
struct RecordingProcessor {
    state: Arc<Mutex<RecordingState>>,
    gain: f32,
}

#[derive(Clone, Debug, Default)]
struct RecordingState {
    prepared: Option<PrepareConfig>,
    transport_sample_pos: u64,
    events: Vec<OwnedEvent>,
}

#[derive(Clone, Debug, PartialEq)]
enum OwnedEvent {
    Midi {
        offset: u32,
        bytes: [u8; 3],
        len: u8,
    },
    Param {
        offset: u32,
        param: u32,
        value: f64,
    },
}

impl RecordingProcessor {
    fn new(gain: f32) -> (Self, Arc<Mutex<RecordingState>>) {
        let state = Arc::new(Mutex::new(RecordingState::default()));
        (
            Self {
                state: Arc::clone(&state),
                gain,
            },
            state,
        )
    }
}

impl Processor for RecordingProcessor {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.state.lock().expect("recording lock").prepared = Some(cfg);
    }

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let mut state = self.state.lock().expect("recording lock");
        state.transport_sample_pos = block.transport.sample_pos;
        state.events.clear();
        for event in block.in_events {
            match *event {
                BlockEvent::Midi { offset, bytes, len } => {
                    state.events.push(OwnedEvent::Midi { offset, bytes, len });
                }
                BlockEvent::ParamSet {
                    offset,
                    param,
                    value,
                } => state.events.push(OwnedEvent::Param {
                    offset,
                    param,
                    value,
                }),
                _ => {}
            }
        }
        let frames = block.frames as usize;
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for (source, target) in input.iter().zip(output.iter_mut()).take(frames) {
                *target = *source * self.gain;
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct PlainProcessor;

impl Processor for PlainProcessor {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let offset = block.in_events.len() as f32;
        let frames = block.frames as usize;
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for (source, target) in input.iter().zip(output.iter_mut()).take(frames) {
                *target = *source + offset;
            }
        }
    }
}

#[test]
fn live_graph_runs_under_fake_backend() {
    let backend = FakeBackend::new();
    let opened = backend
        .open(HostStreamConfigRequest::new(
            fake_backend_symbol(),
            Symbol::new("fake/pcm"),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(4).unwrap(),
        ))
        .unwrap();
    let (processor, _) = RecordingProcessor::new(0.5);
    let mut runner =
        LiveGraphRunner::new(processor, LiveGraphConfig::stereo(48_000, 4).unwrap()).unwrap();
    let clock = LiveTransportClock::sample_frame(48_000).unwrap();
    let mut output = [0.0; 4];

    let report = runner
        .process_interleaved_f32(
            Some(&[1.0, -1.0, 0.5, -0.5]),
            &mut output,
            2,
            clock.transport_at(128, true),
        )
        .unwrap();
    opened
        .stream()
        .push_packet(sim_lib_stream_core::StreamItem::new(StreamPacket::Pcm(
            PcmPacket::f32(2, 2, output.to_vec()).unwrap(),
        )))
        .unwrap();

    assert_eq!(report.frames(), 2);
    assert_eq!(output, [0.5, -0.5, 0.25, -0.25]);
    assert_eq!(opened.queue().drain(8).unwrap().len(), 1);
}

#[test]
fn midi_and_param_events_arrive_at_deterministic_offsets() {
    let (processor, state) = RecordingProcessor::new(1.0);
    let mut runner =
        LiveGraphRunner::new(processor, LiveGraphConfig::stereo(48_000, 8).unwrap()).unwrap();
    let mut output = [0.0; 8];

    assert_eq!(
        runner.enqueue_midi_short(1, &[0x90, 60, 100]).unwrap(),
        LiveQueuePush::Accepted
    );
    assert_eq!(
        runner.enqueue_param_set(3, 7, 0.25).unwrap(),
        LiveQueuePush::Accepted
    );
    let report = runner
        .process_interleaved_f32(
            Some(&[0.0; 8]),
            &mut output,
            4,
            LiveTransportClock::sample_frame(48_000)
                .unwrap()
                .transport_at(512, true),
        )
        .unwrap();
    let state = state.lock().expect("recording state");

    assert_eq!(report.control_events(), 2);
    assert_eq!(state.transport_sample_pos, 512);
    assert_eq!(
        state.events,
        vec![
            OwnedEvent::Midi {
                offset: 1,
                bytes: [0x90, 60, 100],
                len: 3
            },
            OwnedEvent::Param {
                offset: 3,
                param: 7,
                value: 0.25
            }
        ]
    );
}

#[test]
fn control_event_at_block_end_is_rejected() {
    let (processor, _) = RecordingProcessor::new(1.0);
    let mut runner =
        LiveGraphRunner::new(processor, LiveGraphConfig::stereo(48_000, 8).unwrap()).unwrap();
    let mut output = [0.0; 8];

    assert_eq!(
        runner.enqueue_param_set(4, 7, 0.25).unwrap(),
        LiveQueuePush::Accepted
    );
    let err = runner
        .process_interleaved_f32(
            Some(&[0.0; 8]),
            &mut output,
            4,
            LiveTransportClock::sample_frame(48_000)
                .unwrap()
                .transport_at(512, true),
        )
        .expect_err("offset equal to frames is outside the block");

    assert!(err.to_string().contains("outside block frames 0..4"));
}

#[test]
fn steady_state_callback_path_keeps_capacity_with_plain_processor() {
    let mut runner =
        LiveGraphRunner::new(PlainProcessor, LiveGraphConfig::stereo(48_000, 8).unwrap()).unwrap();
    let before = runner.steady_state_snapshot();
    let mut output = [0.0; 8];
    let transport = LiveTransportClock::sample_frame(48_000)
        .unwrap()
        .transport_at(64, true);

    assert_eq!(
        runner.enqueue_midi_short(1, &[0x90, 60, 100]).unwrap(),
        LiveQueuePush::Accepted
    );
    assert_eq!(
        runner.enqueue_param_set(2, 7, 0.5).unwrap(),
        LiveQueuePush::Accepted
    );
    let report = runner
        .process_interleaved_f32(
            Some(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]),
            &mut output,
            4,
            transport,
        )
        .unwrap();

    assert_eq!(report.control_events(), 2);
    assert_eq!(output, [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    assert_eq!(runner.steady_state_snapshot(), before);

    let report = runner
        .process_interleaved_f32(Some(&[1.0; 8]), &mut output, 4, transport)
        .unwrap();

    assert_eq!(report.control_events(), 0);
    assert_eq!(output, [1.0; 8]);
    assert_eq!(runner.steady_state_snapshot(), before);
}

#[test]
fn steady_state_processing_keeps_preallocated_capacities() {
    let (processor, _) = RecordingProcessor::new(1.0);
    let mut runner =
        LiveGraphRunner::new(processor, LiveGraphConfig::stereo(48_000, 8).unwrap()).unwrap();
    let mut output = [0.0; 8];
    let transport = LiveTransportClock::sample_frame(48_000)
        .unwrap()
        .transport_at(0, true);
    let before = runner.steady_state_snapshot();

    runner
        .process_interleaved_f32(Some(&[0.0; 8]), &mut output, 4, transport)
        .unwrap();
    runner
        .process_interleaved_f32(Some(&[0.0; 8]), &mut output, 4, transport)
        .unwrap();

    assert_eq!(runner.steady_state_snapshot(), before);
}

#[test]
fn live_lanes_expose_stream_metadata_for_envelopes() {
    for lane in LiveStreamLane::all().iter().copied() {
        let metadata = lane.metadata(4).unwrap();
        assert_eq!(metadata.id().clone(), lane.stream_id());
        assert_eq!(metadata.media(), lane.media());
        assert_eq!(metadata.direction(), lane.direction());
        assert_eq!(
            ClockDomain::from_symbol(metadata.clock()).unwrap(),
            lane.clock_domain()
        );
        assert_eq!(metadata.buffer().capacity(), 4);
    }

    let packet = StreamPacket::Midi(
        MidiPacket::new(vec![
            MidiPacketEvent::new(0, 480, vec![0x90, 60, 100]).unwrap(),
        ])
        .unwrap(),
    );
    let envelope = LiveStreamLane::Midi
        .realtime_envelope(3, Vec::new(), packet)
        .unwrap();

    assert_eq!(envelope.media(), StreamMedia::Midi);
    assert_eq!(envelope.direction(), StreamDirection::Source);
    assert_eq!(envelope.clock_domain(), ClockDomain::MidiTick);
    assert_eq!(
        envelope.profile().name(),
        realtime_local_audio_profile().name()
    );
}

#[test]
fn realtime_entry_rejects_remote_and_unbounded_streams_before_callback() {
    let (processor, _) = RecordingProcessor::new(1.0);
    let config = LiveGraphConfig::stereo(48_000, 8).unwrap();
    let mut runner =
        LiveGraphRunner::new_realtime(processor, config, &realtime_local_audio_profile()).unwrap();
    let mut output = [0.0; 8];
    runner
        .process_interleaved_f32(
            Some(&[0.0; 8]),
            &mut output,
            4,
            LiveTransportClock::sample_frame(48_000)
                .unwrap()
                .transport_at(0, true),
        )
        .unwrap();

    let remote = TransportProfile::new(
        Symbol::qualified("stream/profile", "remote-only"),
        LatencyClass::RemoteCollaboration,
        vec![StreamCapability::Remote],
    )
    .unwrap();
    let (processor, _) = RecordingProcessor::new(1.0);
    let err = LiveGraphRunner::new_realtime(
        processor,
        LiveGraphConfig::stereo(48_000, 8).unwrap(),
        &remote,
    )
    .unwrap_err();
    assert!(err.to_string().contains("remote"));

    let err = LiveGraphConfig::new(PcmSpec::f32(2, 48_000).unwrap(), 2, 8, 0, 4).unwrap_err();
    assert!(err.to_string().contains("bounded and non-zero"));
}

#[test]
fn processor_blocks_convert_to_buffered_pcm_preview_chunks() {
    let (processor, _) = RecordingProcessor::new(0.25);
    let mut runner = LiveGraphRunner::new_realtime(
        processor,
        LiveGraphConfig::stereo(48_000, 4).unwrap(),
        &realtime_local_audio_profile(),
    )
    .unwrap();
    let mut output = [0.0; 4];

    runner
        .process_interleaved_f32(
            Some(&[1.0, -1.0, 0.5, -0.5]),
            &mut output,
            2,
            LiveTransportClock::sample_frame(48_000)
                .unwrap()
                .transport_at(0, true),
        )
        .unwrap();
    let envelope = runner.buffered_preview_chunk(&output, 2, 9).unwrap();

    assert_eq!(envelope.media(), StreamMedia::Pcm);
    assert_eq!(envelope.clock_domain(), ClockDomain::Sample);
    assert_eq!(
        envelope.profile().latency_class(),
        LatencyClass::BufferedPreview
    );
    assert_eq!(
        envelope.profile().name(),
        TransportProfile::lan_buffered_audio_preview().name()
    );
    assert!(envelope.profile().has_capability(StreamCapability::Preview));
    let StreamPacket::Pcm(packet) = envelope.packet() else {
        panic!("preview envelope should carry PCM");
    };
    assert_eq!(packet.frames(), 2);
    assert_eq!(packet.channels(), 2);
    assert_eq!(packet.samples_f32(), &[0.25, -0.25, 0.125, -0.125]);
}

#[test]
fn unbuffered_audio_callback_tunneling_is_refused_by_default() {
    let err = refuse_unbuffered_audio_callback_tunnel(&TransportProfile::realtime_local_audio())
        .unwrap_err();

    assert!(err.to_string().contains("refused by default"));
    assert!(err.to_string().contains("lan-buffered-audio-preview"));
    refuse_unbuffered_audio_callback_tunnel(&TransportProfile::lan_buffered_audio_preview())
        .unwrap();
}

#[test]
fn lan_buffered_preview_window_reports_jitter_reorder_drop_and_late_packets() {
    let mut window = LanBufferedPreviewWindow::new(0, 1);

    assert_eq!(
        sequence_numbers(window.push(lan_preview_envelope(0)).unwrap()),
        vec![0]
    );
    assert!(window.push(lan_preview_envelope(2)).unwrap().is_empty());
    let kinds = diagnostic_kinds(window.drain_diagnostics());
    assert!(kinds.contains(&lan_buffered_preview_jitter_diagnostic_kind()));
    assert!(kinds.contains(&lan_buffered_preview_reorder_diagnostic_kind()));
    assert_eq!(
        sequence_numbers(window.push(lan_preview_envelope(1)).unwrap()),
        vec![1, 2]
    );

    let mut window = LanBufferedPreviewWindow::new(0, 0);
    assert_eq!(
        sequence_numbers(window.push(lan_preview_envelope(2)).unwrap()),
        vec![2]
    );
    let kinds = diagnostic_kinds(window.drain_diagnostics());
    assert!(kinds.contains(&lan_buffered_preview_jitter_diagnostic_kind()));
    assert!(kinds.contains(&lan_buffered_preview_reorder_diagnostic_kind()));
    assert!(kinds.contains(&lan_buffered_preview_drop_diagnostic_kind()));
    assert!(window.push(lan_preview_envelope(1)).unwrap().is_empty());
    let kinds = diagnostic_kinds(window.drain_diagnostics());
    assert!(kinds.contains(&lan_buffered_preview_late_packet_diagnostic_kind()));
}

#[test]
fn bounded_control_queue_reports_drops_as_stream_diagnostics() {
    let (processor, _) = RecordingProcessor::new(1.0);
    let config = LiveGraphConfig::new(PcmSpec::f32(2, 48_000).unwrap(), 2, 4, 1, 4).unwrap();
    let mut runner = LiveGraphRunner::new(processor, config).unwrap();
    let mut output = [0.0; 4];

    assert_eq!(
        runner.enqueue_midi_short(0, &[0x90, 60, 100]).unwrap(),
        LiveQueuePush::Accepted
    );
    assert_eq!(
        runner.enqueue_param_set(1, 1, 0.5).unwrap(),
        LiveQueuePush::DroppedNewest
    );
    let report = runner
        .process_interleaved_f32(
            Some(&[0.0; 4]),
            &mut output,
            2,
            LiveTransportClock::sample_frame(48_000)
                .unwrap()
                .transport_at(0, true),
        )
        .unwrap();
    let inspector = runner.diagnostic_inspector().unwrap();
    assert_eq!(inspector.status, StreamInspectorStatus::Live);
    assert_eq!(inspector.queue_depth, 1);
    let diagnostics = runner.drain_audio_diagnostics();

    assert_eq!(report.dropped_control_events(), 1);
    assert!(matches!(diagnostics[0], StreamPacket::Diagnostic(_)));
    let envelope = LiveStreamLane::Diagnostic
        .realtime_envelope(0, Vec::new(), diagnostics[0].clone())
        .unwrap();
    assert_eq!(envelope.media(), StreamMedia::Diagnostic);
    assert_eq!(
        LiveStreamLane::Diagnostic
            .metadata(4)
            .unwrap()
            .buffer()
            .capacity(),
        4
    );
}

#[test]
fn oversized_blocks_emit_xrun_diagnostics() {
    let (processor, _) = RecordingProcessor::new(1.0);
    let mut runner =
        LiveGraphRunner::new(processor, LiveGraphConfig::stereo(48_000, 2).unwrap()).unwrap();
    let mut output = [0.0; 8];

    let err = runner
        .process_interleaved_f32(
            Some(&[0.0; 8]),
            &mut output,
            4,
            LiveTransportClock::sample_frame(48_000)
                .unwrap()
                .transport_at(0, true),
        )
        .expect_err("oversized callback should fail");
    let diagnostics = runner.drain_audio_events();

    assert!(err.to_string().contains("max block"));
    assert!(matches!(diagnostics[0], LiveAudioEvent::Xrun { .. }));
}

#[test]
fn stream_clock_metadata_feeds_transport() {
    let clock = LiveTransportClock::sample_frame(96_000).unwrap();
    let transport = clock.transport_at(4096, true);

    assert_eq!(clock.clock().id(), &live_clock_symbol());
    assert_eq!(clock.clock().domain(), ClockDomain::Sample);
    assert_eq!(
        clock.clock().chart(),
        &ClockChart::Frames {
            frames_per_second: 96_000
        }
    );
    assert_eq!(transport.sample_pos, 4096);
    assert!(transport.playing);
}

#[test]
fn install_audio_graph_live_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_audio_graph_live_lib(&mut cx).expect("install");
    install_audio_graph_live_lib(&mut cx).expect("idempotent install");

    assert!(
        cx.registry()
            .lib(&Symbol::new("audio-graph-live"))
            .expect("registered")
            .manifest
            .exports
            .iter()
            .any(|export| *export.symbol() == Symbol::qualified("stream", "LiveGraphRunner"))
    );
    assert!(
        cx.registry()
            .lib(&Symbol::new("audio-graph-live"))
            .expect("registered")
            .manifest
            .exports
            .iter()
            .any(|export| {
                *export.symbol() == Symbol::qualified("stream", "RealtimeLocalAudioProfile")
            })
    );
    assert!(
        cx.registry()
            .lib(&Symbol::new("audio-graph-live"))
            .expect("registered")
            .manifest
            .exports
            .iter()
            .any(|export| {
                *export.symbol() == Symbol::qualified("stream", "LanBufferedAudioPreviewProfile")
            })
    );
}

fn lan_preview_envelope(sequence: u64) -> sim_lib_stream_core::StreamEnvelope {
    LiveStreamLane::AudioOutput
        .lan_buffered_preview_envelope(
            sequence,
            Vec::new(),
            StreamPacket::Pcm(PcmPacket::f32(2, 1, vec![sequence as f32, 0.0]).unwrap()),
        )
        .unwrap()
}

fn sequence_numbers(envelopes: Vec<sim_lib_stream_core::StreamEnvelope>) -> Vec<u64> {
    envelopes
        .into_iter()
        .map(|envelope| envelope.sequence())
        .collect()
}

fn diagnostic_kinds(packets: Vec<StreamPacket>) -> Vec<Symbol> {
    packets
        .into_iter()
        .filter_map(|packet| match packet {
            StreamPacket::Diagnostic(diagnostic) => Some(diagnostic.kind().clone()),
            _ => None,
        })
        .collect()
}
