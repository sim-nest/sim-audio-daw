use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostClockInfo, HostDeviceInventory,
    HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream, HostPortSpec, HostStreamConfig,
    HostStreamConfigRequest,
};

use crate::{PortAudioDevice, PortAudioTestTonePlan};

/// Returns the `stream/host` backend symbol that identifies this PortAudio
/// host backend across the streaming fabric.
pub fn portaudio_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "portaudio")
}

/// Returns the `stream/transport` symbol that names the PortAudio transport
/// carried by this backend.
pub fn portaudio_transport_symbol() -> Symbol {
    Symbol::qualified("stream/transport", "portaudio")
}

/// PortAudio host backend with deterministic provider data.
#[derive(Clone, Debug)]
pub struct PortAudioBackend {
    info: HostBackendInfo,
    devices: Vec<PortAudioDevice>,
}

impl Default for PortAudioBackend {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl PortAudioBackend {
    /// Builds a backend over the given devices.
    ///
    /// The backend advertises audio-output, hotplug, and reconnect
    /// capabilities, and additionally advertises audio input when any device
    /// faces a direction other than [`HostDirection::Output`].
    pub fn new(devices: Vec<PortAudioDevice>) -> Self {
        let mut capabilities = vec![
            HostBackendCapability::AudioOutput,
            HostBackendCapability::Hotplug,
            HostBackendCapability::Reconnect,
        ];
        if devices
            .iter()
            .any(|device| device.direction() != HostDirection::Output)
        {
            capabilities.push(HostBackendCapability::AudioInput);
        }
        Self {
            info: HostBackendInfo::new(
                portaudio_backend_symbol(),
                portaudio_transport_symbol(),
                StreamMedia::Pcm,
                true,
            )
            .with_capabilities(capabilities),
            devices,
        }
    }

    /// Builds a deterministic offline backend that needs no PortAudio
    /// installation.
    ///
    /// It exposes a single fake stereo 48 kHz default output and advertises
    /// the offline and fake capabilities, so enumeration and stream opening can
    /// be validated without audio hardware.
    pub fn fake() -> Self {
        let device = PortAudioDevice::output(
            "portaudio/default-output",
            "Fake PortAudio Default Output",
            2,
            48_000,
        )
        .expect("valid fake output")
        .with_default_output()
        .with_buffer_frames(128)
        .expect("valid fake buffer");
        let mut backend = Self::new(vec![device]);
        backend.info = HostBackendInfo::new(
            portaudio_backend_symbol(),
            portaudio_transport_symbol(),
            StreamMedia::Pcm,
            false,
        )
        .with_capabilities(vec![
            HostBackendCapability::AudioOutput,
            HostBackendCapability::Offline,
            HostBackendCapability::Fake,
        ]);
        backend
    }

    /// Returns the devices known to this backend.
    pub fn list_devices(&self) -> &[PortAudioDevice] {
        &self.devices
    }

    /// Returns the device flagged as the default output, if any.
    pub fn default_output(&self) -> Option<&PortAudioDevice> {
        self.devices.iter().find(|device| device.default_output())
    }

    /// Opens an output stream on the default output device.
    ///
    /// `capacity` bounds the stream buffer policy. Returns an error when no
    /// default output device is present.
    pub fn open_default_output(&self, capacity: usize) -> Result<HostOpenStream> {
        let device = self
            .default_output()
            .ok_or_else(|| Error::Eval("PortAudio default output was not found".to_owned()))?;
        self.open(HostStreamConfigRequest::new(
            portaudio_backend_symbol(),
            device.id().clone(),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(capacity)?,
        ))
    }

    /// Builds a [`PortAudioTestTonePlan`] targeting the default output.
    ///
    /// The plan carries a stream request sized to the device buffer plus a
    /// rendered preview of `frames` samples at `frequency_hz`. Returns an error
    /// when no default output device is present.
    pub fn test_tone_plan(
        &self,
        frames: usize,
        frequency_hz: f32,
    ) -> Result<PortAudioTestTonePlan> {
        let device = self
            .default_output()
            .ok_or_else(|| Error::Eval("PortAudio default output was not found".to_owned()))?;
        let request = HostStreamConfigRequest::new(
            portaudio_backend_symbol(),
            device.id().clone(),
            StreamMedia::Pcm,
            HostDirection::Output,
            BufferPolicy::bounded(device.buffer_frames())?,
        );
        PortAudioTestTonePlan::new(request, device.spec()?, frames, frequency_hz)
    }

    fn require_device(
        &self,
        device: &Symbol,
        direction: HostDirection,
    ) -> Result<&PortAudioDevice> {
        let Some(device) = self
            .devices
            .iter()
            .find(|candidate| candidate.id() == device)
        else {
            return Err(Error::Eval(format!(
                "PortAudio device {device} was not found"
            )));
        };
        if !device.is_compatible_with(direction) {
            return Err(Error::TypeMismatch {
                expected: "PortAudio device with requested direction",
                found: "PortAudio device with another direction",
            });
        }
        Ok(device)
    }
}

impl HostBackend for PortAudioBackend {
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
                    portaudio_backend_symbol(),
                    StreamMedia::Pcm,
                    device.direction(),
                    Symbol::qualified("clock", "portaudio"),
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
                    portaudio_backend_symbol(),
                    StreamMedia::Pcm,
                    device.direction(),
                )
            })
            .collect();
        Ok(HostDeviceInventory::new(portaudio_backend_symbol())
            .with_devices(devices)
            .with_ports(ports))
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "PortAudio backend cannot open {} requests",
                request.backend()
            )));
        }
        if request.media() != StreamMedia::Pcm {
            return Err(Error::TypeMismatch {
                expected: "PCM stream request",
                found: "non-PCM stream request",
            });
        }
        let device = self.require_device(request.device(), request.direction())?;
        let config = HostStreamConfig::from_request(
            request,
            HostLatencyInfo::new(0, device.buffer_frames() as u32),
            HostClockInfo::new(
                Symbol::qualified("clock", "portaudio"),
                Some(device.sample_rate_hz()),
                !self.info.hardware_required(),
            ),
        );
        Ok(HostOpenStream::new(config))
    }
}
