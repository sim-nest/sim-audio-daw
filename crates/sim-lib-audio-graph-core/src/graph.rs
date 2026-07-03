use std::collections::{BTreeMap, VecDeque};

use sim_kernel::{Error, Result};

use crate::{
    BlockArena, NullEventSink, Patch, PatchNode, PortDecl, PortDir, PortUri, PrepareConfig,
    ProcessBlock, Processor, ProcessorDescriptor, RateContract, Transport,
};

/// A directed connection from one node's output port to another's input port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cable {
    /// Source port URI (an output port).
    pub from: PortUri,
    /// Destination port URI (an input port).
    pub to: PortUri,
}

impl Cable {
    /// Creates a cable from a source port to a destination port.
    pub fn new(from: PortUri, to: PortUri) -> Self {
        Self { from, to }
    }
}

/// A directed audio processor graph: nodes plus the cables between their ports.
///
/// Build a graph by adding nodes and connecting ports, [`prepare`](Graph::prepare)
/// it with a sample rate and block size, then render deterministic blocks with
/// [`process_offline`](Graph::process_offline). Nodes are processed in
/// topological order; cycles are rejected at connect time.
pub struct Graph {
    nodes: BTreeMap<String, GraphNode>,
    cables: Vec<Cable>,
    order: Vec<String>,
    prepared: Option<PreparedGraph>,
    arena: BlockArena,
}

struct GraphNode {
    processor: Box<dyn Processor>,
    in_channels: u16,
    out_channels: u16,
    descriptor: ProcessorDescriptor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreparedGraph {
    max_block_frames: u32,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    /// Creates an empty graph.
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            cables: Vec::new(),
            order: Vec::new(),
            prepared: None,
            arena: BlockArena::empty(),
        }
    }

    /// Adds a processor node with the given id and channel counts.
    ///
    /// Fails if the id is empty or already present. Adding a node clears any
    /// prepared state, requiring a fresh [`prepare`](Graph::prepare).
    pub fn add_node(
        &mut self,
        id: impl Into<String>,
        processor: Box<dyn Processor>,
        in_channels: u16,
        out_channels: u16,
    ) -> Result<()> {
        let id = id.into();
        if id.is_empty() {
            return Err(Error::Eval(
                "audio graph node id cannot be empty".to_owned(),
            ));
        }
        if self.nodes.contains_key(&id) {
            return Err(Error::Eval(format!("duplicate audio graph node: {id}")));
        }
        let descriptor = processor.descriptor(in_channels, out_channels);
        self.nodes.insert(
            id,
            GraphNode {
                processor,
                in_channels,
                out_channels,
                descriptor,
            },
        );
        self.order.clear();
        self.prepared = None;
        Ok(())
    }

    /// Connects an output port to an input port.
    ///
    /// Validates both endpoints and their rate contracts, and rejects the cable
    /// if it would introduce a cycle.
    pub fn connect(&mut self, from: PortUri, to: PortUri) -> Result<()> {
        let source_rate = self.validate_endpoint(&from, PortDirection::Out)?;
        let target_rate = self.validate_endpoint(&to, PortDirection::In)?;
        source_rate.ensure_compatible(target_rate)?;
        self.cables.push(Cable::new(from, to));
        match self.topological_order() {
            Ok(order) => {
                self.order = order;
                self.prepared = None;
                Ok(())
            }
            Err(error) => {
                self.cables.pop();
                Err(error)
            }
        }
    }

    /// Prepares every node for the given sample rate and maximum block size,
    /// sizing the scratch arena. Fails on a zero rate/size or a graph cycle.
    pub fn prepare(&mut self, sample_rate_hz: u32, max_block_frames: u32) -> Result<()> {
        if sample_rate_hz == 0 {
            return Err(Error::Eval("sample rate must be nonzero".to_owned()));
        }
        if max_block_frames == 0 {
            return Err(Error::Eval("max block frames must be nonzero".to_owned()));
        }
        let order = self.topological_order()?;
        let mut max_channels = 1usize;
        for id in &order {
            let node = self
                .nodes
                .get_mut(id)
                .ok_or_else(|| Error::Eval(format!("missing audio graph node: {id}")))?;
            max_channels = max_channels.max(usize::from(node.in_channels));
            max_channels = max_channels.max(usize::from(node.out_channels));
            node.processor.prepare(PrepareConfig::new(
                sample_rate_hz,
                max_block_frames,
                node.in_channels,
                node.out_channels,
            ));
        }
        self.order = order;
        self.prepared = Some(PreparedGraph { max_block_frames });
        self.arena = BlockArena::with_f32_capacity(max_block_frames as usize * max_channels);
        Ok(())
    }

    /// Renders one block offline, feeding `input` lanes to the source nodes and
    /// returning the output node's channel buffers.
    ///
    /// Requires a prepared graph and `frames` no larger than the prepared block
    /// size; each input lane must hold at least `frames` samples.
    pub fn process_offline(&mut self, input: &[Vec<f32>], frames: u32) -> Result<Vec<Vec<f32>>> {
        let prepared = self.prepared.ok_or_else(|| {
            Error::Eval("audio graph must be prepared before processing".to_owned())
        })?;
        if frames > prepared.max_block_frames {
            return Err(Error::Eval(format!(
                "process block has {frames} frames, max prepared block is {}",
                prepared.max_block_frames
            )));
        }
        if self.order.is_empty() {
            return Err(Error::Eval("audio graph has no nodes".to_owned()));
        }
        let frames_len = frames as usize;
        for (index, lane) in input.iter().enumerate() {
            if lane.len() < frames_len {
                return Err(Error::Eval(format!(
                    "input audio lane {index} has {} frames, expected at least {frames_len}",
                    lane.len()
                )));
            }
        }

        let mut node_outputs = BTreeMap::<String, Vec<Vec<f32>>>::new();
        let order = self.order.clone();
        for id in &order {
            let (in_channels, out_channels) = self.node_channel_counts(id)?;
            let incoming = self.incoming_edges(id)?;
            let mut in_buffers = vec![vec![0.0; frames_len]; usize::from(in_channels)];
            if incoming.is_empty() {
                for (channel, buffer) in in_buffers.iter_mut().enumerate() {
                    if let Some(source) = input.get(channel) {
                        buffer.copy_from_slice(&source[..frames_len]);
                    }
                }
            } else {
                for edge in incoming {
                    let source_outputs = node_outputs.get(&edge.source_node).ok_or_else(|| {
                        Error::Eval(format!(
                            "missing processed output for node {}",
                            edge.source_node
                        ))
                    })?;
                    let source = source_outputs.get(edge.source_index).ok_or_else(|| {
                        Error::Eval(format!(
                            "source output channel {} is out of range for node {}",
                            edge.source_index, edge.source_node
                        ))
                    })?;
                    let target = in_buffers.get_mut(edge.target_index).ok_or_else(|| {
                        Error::Eval(format!(
                            "target input channel {} is out of range for node {id}",
                            edge.target_index
                        ))
                    })?;
                    target.copy_from_slice(&source[..frames_len]);
                }
            }

            let mut out_buffers = vec![vec![0.0; frames_len]; usize::from(out_channels)];
            {
                let in_audio = in_buffers.iter().map(Vec::as_slice).collect::<Vec<_>>();
                let mut out_audio = out_buffers
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                let mut out_events = NullEventSink;
                let in_events = [];
                self.arena.reset();
                let mut block = ProcessBlock {
                    frames,
                    in_audio: in_audio.as_slice(),
                    out_audio: out_audio.as_mut_slice(),
                    in_events: &in_events,
                    out_events: &mut out_events,
                    transport: Transport::default(),
                    scratch: &mut self.arena,
                };
                block.validate_audio_lanes()?;
                let processor = self
                    .nodes
                    .get_mut(id)
                    .ok_or_else(|| Error::Eval(format!("missing audio graph node: {id}")))?;
                processor.processor.process(&mut block);
            }
            node_outputs.insert(id.clone(), out_buffers);
        }

        let output_node = order
            .last()
            .ok_or_else(|| Error::Eval("audio graph has no output node".to_owned()))?;
        node_outputs
            .remove(output_node)
            .ok_or_else(|| Error::Eval(format!("missing graph output for node {output_node}")))
    }

    /// Returns the graph as a portable [`Patch`] of nodes and cables.
    pub fn to_patch(&self) -> Patch {
        Patch {
            nodes: self.patch_nodes(),
            cables: self.cables.clone(),
        }
    }

    /// Returns the graph's cables.
    pub fn cables(&self) -> &[Cable] {
        &self.cables
    }

    /// Returns the graph's nodes as portable [`PatchNode`] records.
    pub fn patch_nodes(&self) -> Vec<PatchNode> {
        self.nodes
            .iter()
            .map(|(id, node)| PatchNode {
                id: id.clone(),
                in_channels: node.in_channels,
                out_channels: node.out_channels,
            })
            .collect()
    }

    /// Returns the node ids in a stable topological order, failing on a cycle.
    pub fn topological_node_order(&self) -> Result<Vec<String>> {
        self.topological_order()
    }

    /// Returns the processor descriptor for a node, or an error if unknown.
    pub fn node_descriptor(&self, id: &str) -> Result<&ProcessorDescriptor> {
        self.nodes
            .get(id)
            .map(|node| &node.descriptor)
            .ok_or_else(|| Error::Eval(format!("unknown audio graph node: {id}")))
    }

    fn validate_endpoint(&self, uri: &PortUri, direction: PortDirection) -> Result<RateContract> {
        let node_id = uri.node_id().ok_or_else(|| {
            Error::Eval(format!("port URI does not reference a graph node: {uri}"))
        })?;
        let node = self
            .nodes
            .get(node_id)
            .ok_or_else(|| Error::Eval(format!("unknown audio graph node: {node_id}")))?;
        let port = descriptor_port(&node.descriptor, uri, direction)?;
        if uri.index >= u32::from(port.channels) {
            return Err(Error::Eval(format!(
                "port index {} is out of range for node {node_id}",
                uri.index
            )));
        }
        Ok(port.rate_contract)
    }

    fn node_channel_counts(&self, id: &str) -> Result<(u16, u16)> {
        self.nodes
            .get(id)
            .map(|node| (node.in_channels, node.out_channels))
            .ok_or_else(|| Error::Eval(format!("missing audio graph node: {id}")))
    }

    fn incoming_edges(&self, node_id: &str) -> Result<Vec<Edge>> {
        self.cables
            .iter()
            .filter(|cable| cable.to.node_id() == Some(node_id))
            .map(|cable| {
                Ok(Edge {
                    source_node: cable_source_node(cable)?,
                    source_index: cable.from.index as usize,
                    target_index: cable.to.index as usize,
                })
            })
            .collect()
    }

    fn topological_order(&self) -> Result<Vec<String>> {
        let mut indegree = self
            .nodes
            .keys()
            .map(|id| (id.clone(), 0usize))
            .collect::<BTreeMap<_, _>>();
        let mut outgoing = self
            .nodes
            .keys()
            .map(|id| (id.clone(), Vec::<String>::new()))
            .collect::<BTreeMap<_, _>>();

        for cable in &self.cables {
            let source = cable_source_node(cable)?;
            let target = cable_target_node(cable)?;
            if !self.nodes.contains_key(&source) {
                return Err(Error::Eval(format!("unknown audio graph node: {source}")));
            }
            if !self.nodes.contains_key(&target) {
                return Err(Error::Eval(format!("unknown audio graph node: {target}")));
            }
            *indegree
                .get_mut(&target)
                .ok_or_else(|| Error::Eval(format!("missing node indegree: {target}")))? += 1;
            outgoing
                .get_mut(&source)
                .ok_or_else(|| Error::Eval(format!("missing node outputs: {source}")))?
                .push(target);
        }
        for targets in outgoing.values_mut() {
            targets.sort();
        }

        let mut ready = indegree
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
            .collect::<VecDeque<_>>();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(id) = ready.pop_front() {
            order.push(id.clone());
            let targets = outgoing
                .get(&id)
                .ok_or_else(|| Error::Eval(format!("missing outgoing list for node {id}")))?;
            for target in targets {
                let count = indegree
                    .get_mut(target)
                    .ok_or_else(|| Error::Eval(format!("missing indegree for node {target}")))?;
                *count = count.saturating_sub(1);
                if *count == 0 {
                    ready.push_back(target.clone());
                }
            }
        }
        if order.len() != self.nodes.len() {
            return Err(Error::Eval("audio graph contains a cycle".to_owned()));
        }
        Ok(order)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PortDirection {
    In,
    Out,
}

impl PortDirection {
    fn port_dir(self) -> PortDir {
        match self {
            Self::In => PortDir::In,
            Self::Out => PortDir::Out,
        }
    }
}

fn descriptor_port<'a>(
    descriptor: &'a ProcessorDescriptor,
    uri: &PortUri,
    direction: PortDirection,
) -> Result<&'a PortDecl> {
    let port_name = uri
        .node_port_name()
        .ok_or_else(|| Error::Eval(format!("port URI does not name a graph node port: {uri}")))?;
    let direction_ports = descriptor
        .ports()
        .iter()
        .filter(|port| port.dir == direction.port_dir())
        .collect::<Vec<_>>();
    if let Some(port) = direction_ports
        .iter()
        .copied()
        .find(|port| port.name == port_name)
    {
        return Ok(port);
    }
    if direction_ports.len() == 1 {
        return Ok(direction_ports[0]);
    }
    Err(Error::Eval(format!(
        "unknown audio graph node port {port_name}"
    )))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Edge {
    source_node: String,
    source_index: usize,
    target_index: usize,
}

fn cable_source_node(cable: &Cable) -> Result<String> {
    cable
        .from
        .node_id()
        .map(str::to_owned)
        .ok_or_else(|| Error::Eval(format!("invalid source port URI: {}", cable.from)))
}

fn cable_target_node(cable: &Cable) -> Result<String> {
    cable
        .to
        .node_id()
        .map(str::to_owned)
        .ok_or_else(|| Error::Eval(format!("invalid target port URI: {}", cable.to)))
}
