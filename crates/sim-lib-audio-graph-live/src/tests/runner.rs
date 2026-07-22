use sim_kernel::Symbol;
use sim_lib_audio_graph_core::{ProcessBlock, Processor};
use sim_lib_stream_audio::PcmSpec;
use sim_lib_stream_core::{
    BufferPolicy, LatencyClass, PcmPacket, StreamCapability, StreamMedia, StreamPacket,
    TransportProfile,
};
use sim_lib_stream_host::{
    FakeBackend, HostBackend, HostDirection, HostStreamConfigRequest, fake_backend_symbol,
};

use crate::{
    LanBufferedPreviewWindow, LiveGraphConfig, LiveGraphRunner, LiveQueuePush, LiveStreamLane,
    LiveTransportClock, lan_buffered_preview_jitter_diagnostic_kind,
    lan_buffered_preview_reorder_diagnostic_kind, realtime_local_audio_profile,
    refuse_unbuffered_audio_callback_tunnel,
};

use super::{OwnedEvent, RecordingProcessor};

#[derive(Clone, Debug, Default)]
struct PlainProcessor;

impl Processor for PlainProcessor {
    fn prepare(&mut self, _cfg: sim_lib_audio_graph_core::PrepareConfig) {}

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
fn lan_buffered_preview_window_reports_jitter_and_reorder_packets() {
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
