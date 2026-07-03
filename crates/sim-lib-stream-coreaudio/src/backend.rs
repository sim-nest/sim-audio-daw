use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostClockInfo, HostDeviceInventory,
    HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream, HostPortSpec, HostStreamConfig,
    HostStreamConfigRequest,
};

use crate::{CoreAudioDevice, CoreAudioTiming};

/// Returns the `stream/host` symbol identifying the CoreAudio host backend.
///
/// This is the backend id carried by the backend `HostBackendInfo` and matched
/// against an incoming `HostStreamConfigRequest` backend when routing opens.
pub fn coreaudio_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "coreaudio")
}

/// Returns the `stream/transport` symbol for the CoreAudio transport surface.
pub fn coreaudio_transport_symbol() -> Symbol {
    Symbol::qualified("stream/transport", "coreaudio")
}

/// Returns the `clock` symbol stamped onto streams opened by this backend.
pub fn coreaudio_clock_symbol() -> Symbol {
    Symbol::qualified("clock", "coreaudio")
}

/// CoreAudio host backend with provider-supplied deterministic devices.
#[derive(Clone, Debug)]
pub struct CoreAudioBackend {
    info: HostBackendInfo,
    devices: Vec<CoreAudioDevice>,
}

impl Default for CoreAudioBackend {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl CoreAudioBackend {
    /// Builds a backend over the given provider-supplied devices.
    ///
    /// The advertised capabilities are derived from the device directions, and
    /// the backend reports itself as a hardware (non-fake) backend.
    pub fn new(devices: Vec<CoreAudioDevice>) -> Self {
        Self {
            info: HostBackendInfo::new(
                coreaudio_backend_symbol(),
                coreaudio_transport_symbol(),
                StreamMedia::Pcm,
                true,
            )
            .with_capabilities(capabilities_for(&devices, false)),
            devices,
        }
    }

    /// Builds a deterministic offline backend with a fake default output and
    /// default input device.
    ///
    /// The backend reports the `Offline` and `Fake` capabilities so it can be
    /// exercised in tests without Apple frameworks or audio hardware.
    pub fn fake() -> Self {
        let timing = CoreAudioTiming::default_low_latency();
        let devices = vec![
            CoreAudioDevice::output(
                "coreaudio/default-output",
                "Fake CoreAudio Default Output",
                2,
                timing,
            )
            .expect("valid fake output")
            .with_default_output(),
            CoreAudioDevice::input(
                "coreaudio/default-input",
                "Fake CoreAudio Default Input",
                2,
                timing,
            )
            .expect("valid fake input")
            .with_default_input(),
        ];
        Self {
            info: HostBackendInfo::new(
                coreaudio_backend_symbol(),
                coreaudio_transport_symbol(),
                StreamMedia::Pcm,
                false,
            )
            .with_capabilities(capabilities_for(&devices, true)),
            devices,
        }
    }

    /// Returns the devices known to this backend.
    pub fn list_devices(&self) -> &[CoreAudioDevice] {
        &self.devices
    }

    /// Returns the first device flagged as the default output, if any.
    pub fn default_output(&self) -> Option<&CoreAudioDevice> {
        self.devices.iter().find(|device| device.default_output())
    }

    /// Returns the first device flagged as the default input, if any.
    pub fn default_input(&self) -> Option<&CoreAudioDevice> {
        self.devices.iter().find(|device| device.default_input())
    }

    /// Opens an output stream on the default output device.
    ///
    /// `capacity` bounds the open request's buffer policy. Returns an error if
    /// no default output device is present.
    pub fn open_default_output(&self, capacity: usize) -> Result<HostOpenStream> {
        let device = self
            .default_output()
            .ok_or_else(|| Error::Eval("CoreAudio default output was not found".to_owned()))?;
        self.open(request(device, HostDirection::Output, capacity)?)
    }

    /// Opens an input stream on the default input device.
    ///
    /// `capacity` bounds the open request's buffer policy. Returns an error if
    /// no default input device is present.
    pub fn open_default_input(&self, capacity: usize) -> Result<HostOpenStream> {
        let device = self
            .default_input()
            .ok_or_else(|| Error::Eval("CoreAudio default input was not found".to_owned()))?;
        self.open(request(device, HostDirection::Input, capacity)?)
    }

    fn require_device(
        &self,
        device_id: &Symbol,
        direction: HostDirection,
    ) -> Result<&CoreAudioDevice> {
        let Some(device) = self
            .devices
            .iter()
            .find(|candidate| candidate.id() == device_id)
        else {
            return Err(Error::Eval(format!(
                "CoreAudio device {device_id} was not found"
            )));
        };
        if !device.is_compatible_with(direction) {
            return Err(Error::TypeMismatch {
                expected: "CoreAudio device with requested direction",
                found: "CoreAudio device with another direction",
            });
        }
        Ok(device)
    }
}

impl HostBackend for CoreAudioBackend {
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
                    coreaudio_backend_symbol(),
                    StreamMedia::Pcm,
                    device.direction(),
                    coreaudio_clock_symbol(),
                    BufferPolicy::bounded(device.timing().buffer_frames())?,
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
                    coreaudio_backend_symbol(),
                    StreamMedia::Pcm,
                    device.direction(),
                )
            })
            .collect();
        Ok(HostDeviceInventory::new(coreaudio_backend_symbol())
            .with_devices(devices)
            .with_ports(ports))
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "CoreAudio backend cannot open {} requests",
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
            latency_for(direction, device.timing()),
            HostClockInfo::new(
                coreaudio_clock_symbol(),
                Some(device.timing().sample_rate_hz()),
                true,
            ),
        );
        Ok(HostOpenStream::new(config))
    }
}

fn request(
    device: &CoreAudioDevice,
    direction: HostDirection,
    capacity: usize,
) -> Result<HostStreamConfigRequest> {
    Ok(HostStreamConfigRequest::new(
        coreaudio_backend_symbol(),
        device.id().clone(),
        StreamMedia::Pcm,
        direction,
        BufferPolicy::bounded(capacity)?,
    ))
}

fn capabilities_for(devices: &[CoreAudioDevice], fake: bool) -> Vec<HostBackendCapability> {
    let mut capabilities = Vec::new();
    if devices
        .iter()
        .any(|device| device.is_compatible_with(HostDirection::Input))
    {
        capabilities.push(HostBackendCapability::AudioInput);
    }
    if devices
        .iter()
        .any(|device| device.is_compatible_with(HostDirection::Output))
    {
        capabilities.push(HostBackendCapability::AudioOutput);
    }
    if devices
        .iter()
        .any(|device| device.direction() == HostDirection::Duplex)
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

fn latency_for(direction: HostDirection, timing: CoreAudioTiming) -> HostLatencyInfo {
    match direction {
        HostDirection::Input => HostLatencyInfo::new(timing.input_latency_frames(), 0),
        HostDirection::Output => HostLatencyInfo::new(0, timing.output_latency_frames()),
        HostDirection::Duplex => HostLatencyInfo::new(
            timing.input_latency_frames(),
            timing.output_latency_frames(),
        ),
    }
}
