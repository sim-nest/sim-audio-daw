use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_stream_clock::ClockChart;
use sim_lib_stream_core::{
    ClockDomain, MidiPacket, MidiPacketEvent, PcmPacket, StreamDirection, StreamInspectorStatus,
    StreamMedia, StreamPacket,
};

use crate::{
    LiveAudioEvent, LiveGraphConfig, LiveGraphRunner, LiveQueuePush, LiveStreamLane,
    LiveTransportClock, install_audio_graph_live_lib, lan_buffered_preview_drop_diagnostic_kind,
    lan_buffered_preview_jitter_diagnostic_kind, lan_buffered_preview_late_packet_diagnostic_kind,
    lan_buffered_preview_reorder_diagnostic_kind, live_clock_symbol,
};

use super::RecordingProcessor;

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
}

#[test]
fn lan_buffered_preview_window_reports_drop_and_late_packets() {
    let mut window = crate::LanBufferedPreviewWindow::new(0, 0);

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
    let config = LiveGraphConfig::new(
        sim_lib_stream_audio::PcmSpec::f32(2, 48_000).unwrap(),
        2,
        4,
        1,
        4,
    )
    .unwrap();
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
