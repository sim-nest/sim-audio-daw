use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostClockInfo, HostDeviceInventory,
    HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream, HostPortSpec, HostStreamConfig,
    HostStreamConfigRequest,
};

use crate::AlsaPcmDevice;

/// Returns the backend identity symbol `stream/host:alsa`.
pub fn alsa_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "alsa")
}

/// Returns the transport identity symbol `stream/transport:alsa`.
pub fn alsa_transport_symbol() -> Symbol {
    Symbol::qualified("stream/transport", "alsa")
}

/// ALSA host backend with provider-supplied deterministic PCM devices.
#[derive(Clone, Debug)]
pub struct AlsaBackend {
    info: HostBackendInfo,
    devices: Vec<AlsaPcmDevice>,
}

impl Default for AlsaBackend {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl AlsaBackend {
    /// Builds a hardware-required backend over the supplied PCM `devices`.
    ///
    /// Capabilities are derived from the devices' directions; the backend
    /// reports that real audio hardware is required.
    pub fn new(devices: Vec<AlsaPcmDevice>) -> Self {
        Self {
            info: HostBackendInfo::new(
                alsa_backend_symbol(),
                alsa_transport_symbol(),
                StreamMedia::Pcm,
                true,
            )
            .with_capabilities(capabilities_for(&devices, true)),
            devices,
        }
    }

    /// Builds an offline backend with a deterministic set of fake PCM devices.
    ///
    /// The backend reports that hardware is not required and carries the
    /// `Offline` and `Fake` capabilities, so it can drive validation without an
    /// ALSA development package or sound hardware. It exposes a default
    /// playback, a default capture, a `hw:0,0` playback, and a `plughw:1,0`
    /// capture device.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_alsa::AlsaBackend;
    ///
    /// let backend = AlsaBackend::fake();
    /// assert_eq!(backend.list_devices().len(), 4);
    /// assert!(backend.default_playback().is_some());
    /// assert!(backend.default_capture().is_some());
    /// ```
    pub fn fake() -> Self {
        let devices = vec![
            AlsaPcmDevice::default_playback(2, 48_000)
                .expect("valid default playback")
                .with_buffer_frames(128)
                .expect("valid default playback buffer"),
            AlsaPcmDevice::default_capture(2, 48_000)
                .expect("valid default capture")
                .with_buffer_frames(128)
                .expect("valid default capture buffer"),
            AlsaPcmDevice::playback("hw:0,0", "Fake ALSA hw Playback", 2, 48_000)
                .expect("valid hw playback"),
            AlsaPcmDevice::capture("plughw:1,0", "Fake ALSA plughw Capture", 1, 48_000)
                .expect("valid plughw capture"),
        ];
        Self {
            info: HostBackendInfo::new(
                alsa_backend_symbol(),
                alsa_transport_symbol(),
                StreamMedia::Pcm,
                false,
            )
            .with_capabilities(capabilities_for(&devices, false)),
            devices,
        }
    }

    /// Returns the backend's PCM devices in registration order.
    pub fn list_devices(&self) -> &[AlsaPcmDevice] {
        &self.devices
    }

    /// Returns the first default device that can serve output, if any.
    pub fn default_playback(&self) -> Option<&AlsaPcmDevice> {
        self.devices
            .iter()
            .find(|device| device.is_default() && device.is_compatible_with(HostDirection::Output))
    }

    /// Returns the first default device that can serve input, if any.
    pub fn default_capture(&self) -> Option<&AlsaPcmDevice> {
        self.devices
            .iter()
            .find(|device| device.is_default() && device.is_compatible_with(HostDirection::Input))
    }

    /// Opens an output stream on the default playback device.
    ///
    /// Errors when no default playback device is present, or when the open
    /// request cannot be satisfied. `capacity` bounds the stream buffer.
    pub fn open_default_playback(&self, capacity: usize) -> Result<HostOpenStream> {
        let device = self
            .default_playback()
            .ok_or_else(|| Error::Eval("ALSA default playback PCM was not found".to_owned()))?;
        self.open(request(device, HostDirection::Output, capacity)?)
    }

    /// Opens an input stream on the default capture device.
    ///
    /// Errors when no default capture device is present, or when the open
    /// request cannot be satisfied. `capacity` bounds the stream buffer.
    pub fn open_default_capture(&self, capacity: usize) -> Result<HostOpenStream> {
        let device = self
            .default_capture()
            .ok_or_else(|| Error::Eval("ALSA default capture PCM was not found".to_owned()))?;
        self.open(request(device, HostDirection::Input, capacity)?)
    }

    fn require_device(
        &self,
        device_id: &Symbol,
        direction: HostDirection,
    ) -> Result<&AlsaPcmDevice> {
        let Some(device) = self
            .devices
            .iter()
            .find(|candidate| candidate.id() == device_id)
        else {
            return Err(Error::Eval(format!(
                "ALSA PCM device {device_id} was not found"
            )));
        };
        if !device.is_compatible_with(direction) {
            return Err(Error::TypeMismatch {
                expected: "ALSA PCM device with requested direction",
                found: "ALSA PCM device with another direction",
            });
        }
        Ok(device)
    }
}

impl HostBackend for AlsaBackend {
    fn info(&self) -> &HostBackendInfo {
        &self.info
    }

    fn enumerate(&self) -> Result<HostDeviceInventory> {
        let devices = self
            .devices
            .iter()
            .map(|device| {
                Ok(HostDeviceSpec::new(
                    device.id().clone(),
                    alsa_backend_symbol(),
                    StreamMedia::Pcm,
                    device.direction(),
                    Symbol::qualified("clock", "alsa"),
                    BufferPolicy::bounded(device.buffer_frames())?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let ports = self
            .devices
            .iter()
            .map(|device| {
                HostPortSpec::new(
                    device.port_symbol(),
                    device.id().clone(),
                    alsa_backend_symbol(),
                    StreamMedia::Pcm,
                    device.direction(),
                )
            })
            .collect();
        Ok(HostDeviceInventory::new(alsa_backend_symbol())
            .with_devices(devices)
            .with_ports(ports))
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "ALSA backend cannot open {} requests",
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
        let device = self.require_device(request.device(), direction)?;
        let config = HostStreamConfig::from_request(
            request,
            latency_for(direction, device.buffer_frames()),
            HostClockInfo::new(
                Symbol::qualified("clock", "alsa"),
                Some(device.sample_rate_hz()),
                !self.info.hardware_required(),
            ),
        );
        Ok(HostOpenStream::new(config))
    }
}

fn request(
    device: &AlsaPcmDevice,
    direction: HostDirection,
    capacity: usize,
) -> Result<HostStreamConfigRequest> {
    Ok(HostStreamConfigRequest::new(
        alsa_backend_symbol(),
        device.id().clone(),
        StreamMedia::Pcm,
        direction,
        BufferPolicy::bounded(capacity)?,
    ))
}

fn capabilities_for(devices: &[AlsaPcmDevice], fake: bool) -> Vec<HostBackendCapability> {
    let mut capabilities = Vec::new();
    if devices
        .iter()
        .any(|device| device.is_compatible_with(HostDirection::Output))
    {
        capabilities.push(HostBackendCapability::AudioOutput);
    }
    if devices
        .iter()
        .any(|device| device.is_compatible_with(HostDirection::Input))
    {
        capabilities.push(HostBackendCapability::AudioInput);
    }
    if devices
        .iter()
        .any(|device| device.direction() == HostDirection::Duplex)
    {
        capabilities.push(HostBackendCapability::Duplex);
    }
    capabilities.push(HostBackendCapability::Reconnect);
    if fake {
        capabilities.push(HostBackendCapability::Offline);
        capabilities.push(HostBackendCapability::Fake);
    }
    capabilities
}

fn latency_for(direction: HostDirection, frames: usize) -> HostLatencyInfo {
    let frames = frames as u32;
    match direction {
        HostDirection::Input => HostLatencyInfo::new(frames, 0),
        HostDirection::Output => HostLatencyInfo::new(0, frames),
        HostDirection::Duplex => HostLatencyInfo::new(frames, frames),
    }
}
