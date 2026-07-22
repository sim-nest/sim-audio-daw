use crate::{
    AudioGraphNodeConfig, AudioGraphPatchDescriptor, ClockDomain, Graph, LatencyClass, Patch,
    PatchNode, PortDecl, PortDir, PortMedia, PortUri, PrepareConfig, ProcessBlock, Processor,
};

// conformance: audio graph workbench composes checked graph descriptors.

#[derive(Debug)]
struct GainNode {
    gain: f32,
    prepared: Option<PrepareConfig>,
}

impl GainNode {
    fn new(gain: f32) -> Self {
        Self {
            gain,
            prepared: None,
        }
    }
}

impl Processor for GainNode {
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

#[derive(Debug, Default)]
struct PassThroughNode;

impl Processor for PassThroughNode {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            output[..frames].copy_from_slice(&input[..frames]);
        }
    }
}

#[derive(Debug, Default)]
struct ControlSource;

impl Processor for ControlSource {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, _block: &mut ProcessBlock<'_>) {}

    fn ports(&self, _in_channels: u16, _out_channels: u16) -> Vec<PortDecl> {
        vec![PortDecl::new("out", PortMedia::Control, PortDir::Out, 1)]
    }
}

#[derive(Debug, Default)]
struct ControlSink;

impl Processor for ControlSink {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, _block: &mut ProcessBlock<'_>) {}

    fn ports(&self, _in_channels: u16, _out_channels: u16) -> Vec<PortDecl> {
        vec![PortDecl::new("in", PortMedia::Control, PortDir::In, 1)]
    }
}

#[test]
fn port_uri_parse_and_format_round_trip_exactly() {
    let text = "sim-node://graph/main/synth/out:0";
    let uri = text.parse::<PortUri>().expect("parse port URI");
    assert_eq!(uri.to_string(), text);
    assert_eq!(uri.node_id(), Some("synth"));
    assert_eq!(uri.node_port_name(), Some("out"));
}

#[test]
fn default_processor_descriptor_reports_placement_contract() {
    let mut graph = Graph::new();
    graph
        .add_node("gain", Box::new(GainNode::new(0.5)), 1, 1)
        .expect("add gain node");

    let descriptor = graph.node_descriptor("gain").expect("gain descriptor");

    assert_eq!(descriptor.clock_domain(), ClockDomain::Sample);
    assert_eq!(descriptor.latency_class(), LatencyClass::BlockLocal);
    assert!(descriptor.realtime_pin());
    let ports = descriptor.ports();
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0].rate_contract.clock_domain(), ClockDomain::Sample);
    assert_eq!(
        ports[0].rate_contract.latency_class(),
        LatencyClass::SampleExact
    );
}

#[test]
fn control_stream_ports_use_control_rate_contracts() {
    let mut graph = Graph::new();
    graph
        .add_node("source", Box::<ControlSource>::default(), 0, 1)
        .expect("add control source");

    let descriptor = graph.node_descriptor("source").expect("source descriptor");
    let ports = descriptor.ports();

    assert_eq!(ports.len(), 1);
    assert_eq!(ports[0].media, PortMedia::Control);
    assert_eq!(ports[0].rate_contract.clock_domain(), ClockDomain::Control);
    assert_eq!(
        ports[0].rate_contract.latency_class(),
        LatencyClass::Interactive
    );
}

#[test]
fn graph_rejects_mismatched_port_rate_contracts() {
    let mut graph = Graph::new();
    graph
        .add_node("source", Box::<ControlSource>::default(), 0, 1)
        .expect("add control source");
    graph
        .add_node("sink", Box::<PassThroughNode>::default(), 1, 0)
        .expect("add audio sink");

    let error = graph
        .connect(
            PortUri::node("main", "source", "out", 0).expect("source URI"),
            PortUri::node("main", "sink", "in", 0).expect("target URI"),
        )
        .expect_err("control to audio edge must be rejected");

    assert!(error.to_string().contains("incompatible port rate"));
}

#[test]
fn graph_connects_matching_control_stream_ports() {
    let mut graph = Graph::new();
    graph
        .add_node("source", Box::<ControlSource>::default(), 0, 1)
        .expect("add control source");
    graph
        .add_node("sink", Box::<ControlSink>::default(), 1, 0)
        .expect("add control sink");

    graph
        .connect(
            PortUri::node("main", "source", "out", 0).expect("source URI"),
            PortUri::node("main", "sink", "in", 0).expect("target URI"),
        )
        .expect("connect control stream");
}

#[test]
fn two_node_graph_runs_offline() {
    let mut graph = Graph::new();
    graph
        .add_node("gain", Box::new(GainNode::new(0.5)), 1, 1)
        .expect("add gain node");
    graph
        .add_node("pass", Box::<PassThroughNode>::default(), 1, 1)
        .expect("add pass-through node");
    graph
        .connect(
            PortUri::node("main", "gain", "out", 0).expect("source URI"),
            PortUri::node("main", "pass", "in", 0).expect("target URI"),
        )
        .expect("connect graph");

    graph.prepare(48_000, 4).expect("prepare graph");
    let output = graph
        .process_offline(&[vec![1.0, -0.5, 0.25, 0.0]], 4)
        .expect("process offline");

    assert_eq!(output, vec![vec![0.5, -0.25, 0.125, 0.0]]);
}

#[test]
fn graph_rejects_cycles() {
    let mut graph = Graph::new();
    graph
        .add_node("a", Box::new(PassThroughNode), 1, 1)
        .expect("add node a");
    graph
        .add_node("b", Box::new(PassThroughNode), 1, 1)
        .expect("add node b");
    graph
        .connect(
            PortUri::node("main", "a", "out", 0).expect("a out"),
            PortUri::node("main", "b", "in", 0).expect("b in"),
        )
        .expect("connect acyclic edge");

    let error = graph
        .connect(
            PortUri::node("main", "b", "out", 0).expect("b out"),
            PortUri::node("main", "a", "in", 0).expect("a in"),
        )
        .expect_err("cycle must be rejected");
    assert!(error.to_string().contains("cycle"));
}

#[test]
fn graph_patch_round_trips_as_expr() {
    let mut graph = Graph::new();
    graph
        .add_node("gain", Box::new(GainNode::new(0.25)), 1, 1)
        .expect("add gain node");
    graph
        .add_node("pass", Box::<PassThroughNode>::default(), 1, 1)
        .expect("add pass-through node");
    graph
        .connect(
            PortUri::node("main", "gain", "out", 0).expect("source URI"),
            PortUri::node("main", "pass", "in", 0).expect("target URI"),
        )
        .expect("connect graph");

    let patch = graph.to_patch();
    let rebuilt = Patch::from_expr(&patch.to_expr()).expect("decode patch expression");

    assert_eq!(rebuilt, patch);
}

#[test]
fn citizen_audio_graph_descriptors_round_trip_and_fail_closed() {
    let node = PatchNode {
        id: "gain".to_owned(),
        in_channels: 2,
        out_channels: 2,
    };
    let descriptor = AudioGraphNodeConfig::new(node.clone()).unwrap();
    assert_eq!(descriptor.node().unwrap(), node);

    let patch = Patch {
        nodes: vec![node],
        cables: Vec::new(),
    };
    let descriptor = AudioGraphPatchDescriptor::new(patch.clone()).unwrap();
    assert_eq!(descriptor.patch().unwrap(), patch);

    let err = AudioGraphNodeConfig::from_expr(sim_kernel::Expr::Map(vec![
        (
            sim_kernel::Expr::Symbol(sim_kernel::Symbol::qualified("audio-graph", "tag")),
            sim_kernel::Expr::Symbol(sim_kernel::Symbol::qualified("audio-graph", "node-config")),
        ),
        (
            sim_kernel::Expr::Symbol(sim_kernel::Symbol::qualified("audio-graph", "id")),
            sim_kernel::Expr::String(String::new()),
        ),
        (
            sim_kernel::Expr::Symbol(sim_kernel::Symbol::qualified("audio-graph", "in-channels")),
            sim_kernel::Expr::String("2".to_owned()),
        ),
        (
            sim_kernel::Expr::Symbol(sim_kernel::Symbol::qualified("audio-graph", "out-channels")),
            sim_kernel::Expr::String("2".to_owned()),
        ),
    ]))
    .unwrap_err();
    assert!(format!("{err}").contains("cannot be empty"));
}
