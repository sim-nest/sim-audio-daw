use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostClockInfo, HostDeviceInventory,
    HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream, HostPortSpec, HostStreamConfig,
    HostStreamConfigRequest,
};

use crate::{PipeWireNode, PipeWireTiming};

/// Returns the `stream/host:pipewire` symbol identifying this host backend.
///
/// This is the backend id carried by [`HostBackendInfo`] and matched against
/// incoming [`HostStreamConfigRequest`] backends in [`PipeWireBackend::open`].
pub fn pipewire_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "pipewire")
}

/// Returns the `stream/transport:pipewire` symbol naming the transport surface.
///
/// PipeWire is reported as both the host backend and the transport that moves
/// PCM frames, so the transport symbol is distinct from the backend symbol.
pub fn pipewire_transport_symbol() -> Symbol {
    Symbol::qualified("stream/transport", "pipewire")
}

/// PipeWire host backend with deterministic provider data.
#[derive(Clone, Debug)]
pub struct PipeWireBackend {
    info: HostBackendInfo,
    nodes: Vec<PipeWireNode>,
}

impl Default for PipeWireBackend {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl PipeWireBackend {
    /// Builds a backend from a caller-supplied set of provider-reported nodes.
    ///
    /// The reported [`HostBackendInfo`] is marked hardware-required and its
    /// capabilities are derived from the directions present in `nodes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_pipewire::PipeWireBackend;
    ///
    /// let backend = PipeWireBackend::new(Vec::new());
    /// assert!(backend.list_nodes().is_empty());
    /// ```
    pub fn new(nodes: Vec<PipeWireNode>) -> Self {
        Self {
            info: HostBackendInfo::new(
                pipewire_backend_symbol(),
                pipewire_transport_symbol(),
                StreamMedia::Pcm,
                true,
            )
            .with_capabilities(capabilities_for(&nodes, true)),
            nodes,
        }
    }

    /// Builds an offline backend populated with deterministic default nodes.
    ///
    /// The fixture carries a default playback sink, a default capture source,
    /// and a duplex SIM client, all at [`PipeWireTiming::desktop_default`]. The
    /// reported info is not hardware-required and adds the `Offline` and `Fake`
    /// capabilities, so it drives CI without a running PipeWire daemon.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_pipewire::PipeWireBackend;
    ///
    /// let backend = PipeWireBackend::fake();
    /// assert_eq!(backend.list_nodes().len(), 3);
    /// assert!(backend.sim_client().is_some());
    /// ```
    pub fn fake() -> Self {
        let timing = PipeWireTiming::desktop_default();
        let nodes = vec![
            PipeWireNode::playback(
                "pipewire/default/playback",
                "PipeWire",
                "Default Sink",
                2,
                timing,
            )
            .expect("valid default playback"),
            PipeWireNode::capture(
                "pipewire/default/capture",
                "PipeWire",
                "Default Source",
                2,
                timing,
            )
            .expect("valid default capture"),
            PipeWireNode::duplex_sim_client(2, timing).expect("valid SIM duplex client"),
        ];
        Self {
            info: HostBackendInfo::new(
                pipewire_backend_symbol(),
                pipewire_transport_symbol(),
                StreamMedia::Pcm,
                false,
            )
            .with_capabilities(capabilities_for(&nodes, false)),
            nodes,
        }
    }

    /// Returns the provider-reported nodes visible to this backend.
    pub fn list_nodes(&self) -> &[PipeWireNode] {
        &self.nodes
    }

    /// Returns the `pipewire/default/playback` node if it can serve output.
    ///
    /// Matches the well-known default-sink id whose direction is compatible
    /// with [`HostDirection::Output`].
    pub fn default_playback(&self) -> Option<&PipeWireNode> {
        self.nodes.iter().find(|node| {
            node.id() == &Symbol::new("pipewire/default/playback")
                && node.is_compatible_with(HostDirection::Output)
        })
    }

    /// Returns the `pipewire/default/capture` node if it can serve input.
    ///
    /// Matches the well-known default-source id whose direction is compatible
    /// with [`HostDirection::Input`].
    pub fn default_capture(&self) -> Option<&PipeWireNode> {
        self.nodes.iter().find(|node| {
            node.id() == &Symbol::new("pipewire/default/capture")
                && node.is_compatible_with(HostDirection::Input)
        })
    }

    /// Returns the duplex node whose client is named `SIM`, if present.
    pub fn sim_client(&self) -> Option<&PipeWireNode> {
        self.nodes
            .iter()
            .find(|node| node.client_name() == "SIM" && node.direction() == HostDirection::Duplex)
    }

    /// Opens an output stream against the default playback node.
    ///
    /// Builds a bounded [`HostStreamConfigRequest`] of `capacity` frames and
    /// resolves it through [`PipeWireBackend::open`]. Errors if no default
    /// playback node is present.
    pub fn open_default_playback(&self, capacity: usize) -> Result<HostOpenStream> {
        let node = self.default_playback().ok_or_else(|| {
            Error::Eval("PipeWire default playback node was not found".to_owned())
        })?;
        self.open(request(node, HostDirection::Output, capacity)?)
    }

    /// Opens an input stream against the default capture node.
    ///
    /// Builds a bounded [`HostStreamConfigRequest`] of `capacity` frames and
    /// resolves it through [`PipeWireBackend::open`]. Errors if no default
    /// capture node is present.
    pub fn open_default_capture(&self, capacity: usize) -> Result<HostOpenStream> {
        let node = self
            .default_capture()
            .ok_or_else(|| Error::Eval("PipeWire default capture node was not found".to_owned()))?;
        self.open(request(node, HostDirection::Input, capacity)?)
    }

    fn require_node(&self, node_id: &Symbol, direction: HostDirection) -> Result<&PipeWireNode> {
        let Some(node) = self
            .nodes
            .iter()
            .find(|candidate| candidate.id() == node_id)
        else {
            return Err(Error::Eval(format!(
                "PipeWire node {node_id} was not found"
            )));
        };
        if !node.is_compatible_with(direction) {
            return Err(Error::TypeMismatch {
                expected: "PipeWire node with requested direction",
                found: "PipeWire node with another direction",
            });
        }
        Ok(node)
    }
}

impl HostBackend for PipeWireBackend {
    fn info(&self) -> &HostBackendInfo {
        &self.info
    }

    fn enumerate(&self) -> Result<HostDeviceInventory> {
        let devices = self
            .nodes
            .iter()
            .map(|node| {
                Ok(HostDeviceSpec::new(
                    node.id().clone(),
                    pipewire_backend_symbol(),
                    StreamMedia::Pcm,
                    node.direction(),
                    Symbol::qualified("clock", "pipewire"),
                    BufferPolicy::bounded(node.timing().quantum_frames())?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let ports = self
            .nodes
            .iter()
            .flat_map(PipeWireNode::ports)
            .map(|port| {
                HostPortSpec::new(
                    port.id().clone(),
                    port.node().clone(),
                    pipewire_backend_symbol(),
                    StreamMedia::Pcm,
                    port.direction(),
                )
            })
            .collect();
        Ok(HostDeviceInventory::new(pipewire_backend_symbol())
            .with_devices(devices)
            .with_ports(ports))
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "PipeWire backend cannot open {} requests",
                request.backend()
            )));
        }
        if request.media() != StreamMedia::Pcm {
            return Err(Error::TypeMismatch {
                expected: "PCM stream request",
                found: "non-PCM stream request",
            });
        }
        let direction = request.direction();
        let node = self.require_node(request.device(), direction)?;
        let timing = node.timing();
        let config = HostStreamConfig::from_request(
            request,
            latency_for(direction, timing),
            HostClockInfo::new(
                Symbol::qualified("clock", "pipewire"),
                Some(timing.sample_rate_hz()),
                !self.info.hardware_required(),
            ),
        );
        Ok(HostOpenStream::new(config))
    }
}

fn request(
    node: &PipeWireNode,
    direction: HostDirection,
    capacity: usize,
) -> Result<HostStreamConfigRequest> {
    Ok(HostStreamConfigRequest::new(
        pipewire_backend_symbol(),
        node.id().clone(),
        StreamMedia::Pcm,
        direction,
        BufferPolicy::bounded(capacity)?,
    ))
}

fn capabilities_for(nodes: &[PipeWireNode], fake: bool) -> Vec<HostBackendCapability> {
    let mut capabilities = Vec::new();
    if nodes
        .iter()
        .any(|node| node.is_compatible_with(HostDirection::Output))
    {
        capabilities.push(HostBackendCapability::AudioOutput);
    }
    if nodes
        .iter()
        .any(|node| node.is_compatible_with(HostDirection::Input))
    {
        capabilities.push(HostBackendCapability::AudioInput);
    }
    if nodes
        .iter()
        .any(|node| node.direction() == HostDirection::Duplex)
    {
        capabilities.push(HostBackendCapability::Duplex);
    }
    capabilities.push(HostBackendCapability::Hotplug);
    capabilities.push(HostBackendCapability::Reconnect);
    if fake {
        capabilities.push(HostBackendCapability::Offline);
        capabilities.push(HostBackendCapability::Fake);
    }
    capabilities
}

fn latency_for(direction: HostDirection, timing: PipeWireTiming) -> HostLatencyInfo {
    match direction {
        HostDirection::Input => HostLatencyInfo::new(timing.input_latency_frames(), 0),
        HostDirection::Output => HostLatencyInfo::new(0, timing.output_latency_frames()),
        HostDirection::Duplex => HostLatencyInfo::new(
            timing.input_latency_frames(),
            timing.output_latency_frames(),
        ),
    }
}
