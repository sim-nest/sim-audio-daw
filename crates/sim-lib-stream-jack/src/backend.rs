use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostClockInfo, HostDeviceInventory,
    HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream, HostPortSpec, HostStreamConfig,
    HostStreamConfigRequest,
};

use crate::{JackClient, JackTiming, jack_clock_symbol};

/// Returns the symbol that identifies the JACK host backend (`stream/host:jack`).
pub fn jack_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "jack")
}

/// Returns the symbol that identifies the JACK transport (`stream/transport:jack`).
pub fn jack_transport_symbol() -> Symbol {
    Symbol::qualified("stream/transport", "jack")
}

/// JACK host backend with deterministic provider data.
#[derive(Clone, Debug)]
pub struct JackBackend {
    info: HostBackendInfo,
    clients: Vec<JackClient>,
}

impl Default for JackBackend {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl JackBackend {
    /// Builds a live backend over the given JACK clients.
    ///
    /// The backend reports itself as connected (`true`) and derives its
    /// capability set from the supplied clients' audio, MIDI, and duplex ports.
    pub fn new(clients: Vec<JackClient>) -> Self {
        Self {
            info: HostBackendInfo::new(
                jack_backend_symbol(),
                jack_transport_symbol(),
                StreamMedia::Pcm,
                true,
            )
            .with_capabilities(capabilities_for(&clients, true)),
            clients,
        }
    }

    /// Builds an offline backend seeded with the SIM default client.
    ///
    /// The backend reports itself as disconnected and adds the
    /// [`HostBackendCapability::Offline`] and [`HostBackendCapability::Fake`]
    /// capabilities, making it suitable for validation without a running JACK
    /// server.
    pub fn fake() -> Self {
        let clients = vec![JackClient::sim_default().expect("valid SIM JACK client")];
        Self {
            info: HostBackendInfo::new(
                jack_backend_symbol(),
                jack_transport_symbol(),
                StreamMedia::Pcm,
                false,
            )
            .with_capabilities(capabilities_for(&clients, false)),
            clients,
        }
    }

    /// Returns the JACK clients registered with this backend.
    pub fn list_clients(&self) -> &[JackClient] {
        &self.clients
    }

    /// Returns the client named `SIM`, if one is registered.
    pub fn sim_client(&self) -> Option<&JackClient> {
        self.clients.iter().find(|client| client.name() == "SIM")
    }

    /// Opens a duplex stream on the SIM client with a bounded buffer of
    /// `capacity` frames.
    ///
    /// # Errors
    ///
    /// Returns an error if no SIM client is registered or if the open request
    /// is rejected.
    pub fn open_sim_client(&self, capacity: usize) -> Result<HostOpenStream> {
        let client = self
            .sim_client()
            .ok_or_else(|| Error::Eval("JACK SIM client was not found".to_owned()))?;
        self.open(request(client, HostDirection::Duplex, capacity)?)
    }

    fn require_client(&self, client_id: &Symbol, direction: HostDirection) -> Result<&JackClient> {
        let Some(client) = self
            .clients
            .iter()
            .find(|candidate| candidate.id() == client_id)
        else {
            return Err(Error::Eval(format!(
                "JACK client {client_id} was not found"
            )));
        };
        if !client.is_compatible_with(direction) {
            return Err(Error::TypeMismatch {
                expected: "JACK client with requested audio direction",
                found: "JACK client with another audio direction",
            });
        }
        Ok(client)
    }
}

impl HostBackend for JackBackend {
    fn info(&self) -> &HostBackendInfo {
        &self.info
    }

    fn enumerate(&self) -> Result<HostDeviceInventory> {
        let devices = self
            .clients
            .iter()
            .map(|client| {
                Ok(HostDeviceSpec::new(
                    client.id().clone(),
                    jack_backend_symbol(),
                    StreamMedia::Pcm,
                    client.direction(),
                    jack_clock_symbol(),
                    BufferPolicy::bounded(client.timing().block_frames())?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let ports = self
            .clients
            .iter()
            .flat_map(JackClient::ports)
            .map(|port| {
                HostPortSpec::new(
                    port.id().clone(),
                    port.client().clone(),
                    jack_backend_symbol(),
                    port.media(),
                    port.direction(),
                )
            })
            .collect();
        Ok(HostDeviceInventory::new(jack_backend_symbol())
            .with_devices(devices)
            .with_ports(ports))
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "JACK backend cannot open {} requests",
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
        let client = self.require_client(request.device(), direction)?;
        let timing = client.timing();
        let config = HostStreamConfig::from_request(
            request,
            latency_for(direction, timing),
            HostClockInfo::new(jack_clock_symbol(), Some(timing.sample_rate_hz()), true),
        );
        Ok(HostOpenStream::new(config))
    }
}

fn request(
    client: &JackClient,
    direction: HostDirection,
    capacity: usize,
) -> Result<HostStreamConfigRequest> {
    Ok(HostStreamConfigRequest::new(
        jack_backend_symbol(),
        client.id().clone(),
        StreamMedia::Pcm,
        direction,
        BufferPolicy::bounded(capacity)?,
    ))
}

fn capabilities_for(clients: &[JackClient], fake: bool) -> Vec<HostBackendCapability> {
    let mut capabilities = Vec::new();
    if clients.iter().any(|client| client.audio_inputs() > 0) {
        capabilities.push(HostBackendCapability::AudioInput);
    }
    if clients.iter().any(|client| client.audio_outputs() > 0) {
        capabilities.push(HostBackendCapability::AudioOutput);
    }
    if clients.iter().any(|client| client.midi_inputs() > 0) {
        capabilities.push(HostBackendCapability::MidiInput);
    }
    if clients.iter().any(|client| client.midi_outputs() > 0) {
        capabilities.push(HostBackendCapability::MidiOutput);
    }
    if clients
        .iter()
        .any(|client| client.direction() == HostDirection::Duplex)
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

fn latency_for(direction: HostDirection, timing: JackTiming) -> HostLatencyInfo {
    match direction {
        HostDirection::Input => HostLatencyInfo::new(timing.input_latency_frames(), 0),
        HostDirection::Output => HostLatencyInfo::new(0, timing.output_latency_frames()),
        HostDirection::Duplex => HostLatencyInfo::new(
            timing.input_latency_frames(),
            timing.output_latency_frames(),
        ),
    }
}
