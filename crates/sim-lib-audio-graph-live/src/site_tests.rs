use sim_lib_audio_graph_core::{PrepareConfig, ProcessBlock, Processor};

use crate::{LiveGraphConfig, LivePlacedNode, LivePlacementSite, LiveTransportClock};

#[derive(Clone, Copy, Debug)]
struct GainProcessor {
    gain: f32,
    realtime_pin: bool,
}

impl GainProcessor {
    fn portable(gain: f32) -> Self {
        Self {
            gain,
            realtime_pin: false,
        }
    }

    fn pinned(gain: f32) -> Self {
        Self {
            gain,
            realtime_pin: true,
        }
    }
}

impl Processor for GainProcessor {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            for (source, target) in input.iter().zip(output.iter_mut()).take(frames) {
                *target = *source * self.gain;
            }
        }
    }

    fn realtime_pin(&self) -> bool {
        self.realtime_pin
    }
}

#[test]
fn local_site_names_are_recorded() {
    let names = LivePlacementSite::all()
        .iter()
        .map(|site| site.name())
        .collect::<Vec<_>>();
    let symbols = LivePlacementSite::all()
        .iter()
        .map(|site| site.symbol().as_qualified_str())
        .collect::<Vec<_>>();

    assert_eq!(names, ["coroutine", "thread", "host-callback", "process"]);
    assert_eq!(
        symbols,
        [
            "stream/site/coroutine",
            "stream/site/thread",
            "stream/site/host-callback",
            "stream/site/process"
        ]
    );
}

#[test]
fn coroutine_thread_and_process_sites_render_deterministically() {
    let coroutine = render_at_site(LivePlacementSite::Coroutine);
    let thread = render_at_site(LivePlacementSite::Thread);
    let process = render_at_site(LivePlacementSite::Process);

    assert_eq!(coroutine, [0.5, -0.5, 0.25, -0.25]);
    assert_eq!(thread, coroutine);
    assert_eq!(process, coroutine);
}

#[test]
fn realtime_pinned_nodes_stay_on_host_callback_site() {
    let config = LiveGraphConfig::stereo(48_000, 4).unwrap();
    let host = LivePlacedNode::new(
        GainProcessor::pinned(1.0),
        config,
        LivePlacementSite::HostCallback,
    )
    .unwrap();
    assert_eq!(host.site(), LivePlacementSite::HostCallback);

    for site in [
        LivePlacementSite::Coroutine,
        LivePlacementSite::Thread,
        LivePlacementSite::Process,
    ] {
        let err = LivePlacedNode::new(GainProcessor::pinned(1.0), config, site).unwrap_err();
        assert!(err.to_string().contains("realtime-pinned"));
        assert!(err.to_string().contains(site.name()));
    }
}

#[test]
fn process_site_keeps_preallocated_steady_state_capacity() {
    let config = LiveGraphConfig::stereo(48_000, 4).unwrap();
    let mut node = LivePlacedNode::new(
        GainProcessor::portable(0.25),
        config,
        LivePlacementSite::Process,
    )
    .unwrap();
    let before = node.steady_state_snapshot();
    let input = [1.0, -1.0, 0.5, -0.5];
    let mut output = [0.0; 4];
    let transport = LiveTransportClock::sample_frame(48_000)
        .unwrap()
        .transport_at(0, true);

    node.process_interleaved_f32(Some(&input), &mut output, 2, transport)
        .unwrap();
    node.process_interleaved_f32(Some(&input), &mut output, 2, transport)
        .unwrap();

    assert_eq!(node.steady_state_snapshot(), before);
    assert_eq!(before.process_request_ring().unwrap().allocated_slots(), 1);
    assert_eq!(before.process_response_ring().unwrap().allocated_slots(), 1);
}

fn render_at_site(site: LivePlacementSite) -> [f32; 4] {
    let config = LiveGraphConfig::stereo(48_000, 4).unwrap();
    let mut node = LivePlacedNode::new(GainProcessor::portable(0.5), config, site).unwrap();
    let mut output = [0.0; 4];
    node.process_interleaved_f32(
        Some(&[1.0, -1.0, 0.5, -0.5]),
        &mut output,
        2,
        LiveTransportClock::sample_frame(48_000)
            .unwrap()
            .transport_at(128, true),
    )
    .unwrap();
    output
}
