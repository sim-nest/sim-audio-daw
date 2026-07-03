use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};
use sim_lib_stream_host::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostClockInfo, HostDeviceInventory,
    HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream, HostPortSpec, HostStreamConfig,
    HostStreamConfigRequest,
};

use crate::{AsioDriver, AsioTiming};

/// Returns the stream-host backend identifier for ASIO (`stream/host:asio`).
pub fn asio_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "asio")
}

/// Returns the transport identifier carried by ASIO host streams
/// (`stream/transport:asio`).
pub fn asio_transport_symbol() -> Symbol {
    Symbol::qualified("stream/transport", "asio")
}

/// Returns the clock identifier reported by ASIO host streams (`clock:asio`).
pub fn asio_clock_symbol() -> Symbol {
    Symbol::qualified("clock", "asio")
}

/// ASIO host backend with provider-supplied deterministic driver data.
#[derive(Clone, Debug)]
pub struct AsioBackend {
    info: HostBackendInfo,
    drivers: Vec<AsioDriver>,
}

impl Default for AsioBackend {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl AsioBackend {
    /// Builds a backend over the provider-reported `drivers`, marking it as a
    /// native (non-fake) backend and deriving its capabilities from the driver
    /// set.
    pub fn new(drivers: Vec<AsioDriver>) -> Self {
        Self {
            info: HostBackendInfo::new(
                asio_backend_symbol(),
                asio_transport_symbol(),
                StreamMedia::Pcm,
                true,
            )
            .with_capabilities(capabilities_for(&drivers, false)),
            drivers,
        }
    }

    /// Builds an offline backend exposing the single `SIM-ASIO` driver, marked
    /// as fake so it can run under CI without the ASIO SDK or audio hardware.
    pub fn fake() -> Self {
        let drivers = vec![AsioDriver::sim_default().expect("valid fake ASIO driver")];
        Self {
            info: HostBackendInfo::new(
                asio_backend_symbol(),
                asio_transport_symbol(),
                StreamMedia::Pcm,
                false,
            )
            .with_capabilities(capabilities_for(&drivers, true)),
            drivers,
        }
    }

    /// Returns the drivers this backend was constructed with.
    pub fn list_drivers(&self) -> &[AsioDriver] {
        &self.drivers
    }

    /// Returns the bundled `SIM-ASIO` driver if it is present in the backend.
    pub fn sim_driver(&self) -> Option<&AsioDriver> {
        self.drivers
            .iter()
            .find(|driver| driver.name() == "SIM-ASIO")
    }

    /// Opens a duplex stream on the bundled `SIM-ASIO` driver with a bounded
    /// buffer of `capacity` packets.
    ///
    /// Returns an error if the `SIM-ASIO` driver is not present in this backend.
    pub fn open_sim_driver(&self, capacity: usize) -> Result<HostOpenStream> {
        let driver = self
            .sim_driver()
            .ok_or_else(|| Error::Eval("SIM ASIO driver was not found".to_owned()))?;
        self.open(request(driver, HostDirection::Duplex, capacity)?)
    }

    fn require_driver(&self, driver_id: &Symbol, direction: HostDirection) -> Result<&AsioDriver> {
        let Some(driver) = self
            .drivers
            .iter()
            .find(|candidate| candidate.id() == driver_id)
        else {
            return Err(Error::Eval(format!(
                "ASIO driver {driver_id} was not found"
            )));
        };
        if !driver.is_compatible_with(direction) {
            return Err(Error::TypeMismatch {
                expected: "ASIO driver with requested audio direction",
                found: "ASIO driver with another audio direction",
            });
        }
        Ok(driver)
    }
}

impl HostBackend for AsioBackend {
    fn info(&self) -> &HostBackendInfo {
        &self.info
    }

    fn enumerate(&self) -> Result<HostDeviceInventory> {
        let devices = self
            .drivers
            .iter()
            .map(|driver| {
                Ok(HostDeviceSpec::new(
                    driver.id().clone(),
                    asio_backend_symbol(),
                    StreamMedia::Pcm,
                    driver.direction(),
                    asio_clock_symbol(),
                    BufferPolicy::bounded(driver.timing().buffer_frames())?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let ports = self
            .drivers
            .iter()
            .flat_map(AsioDriver::ports)
            .map(|port| {
                HostPortSpec::new(
                    port.id().clone(),
                    port.driver().clone(),
                    asio_backend_symbol(),
                    port.media(),
                    port.direction(),
                )
            })
            .collect();
        Ok(HostDeviceInventory::new(asio_backend_symbol())
            .with_devices(devices)
            .with_ports(ports))
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "ASIO backend cannot open {} requests",
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
        let driver = self.require_driver(request.device(), direction)?;
        let config = HostStreamConfig::from_request(
            request,
            latency_for(direction, driver.timing()),
            HostClockInfo::new(
                asio_clock_symbol(),
                Some(driver.timing().sample_rate_hz()),
                true,
            ),
        );
        Ok(HostOpenStream::new(config))
    }
}

fn request(
    driver: &AsioDriver,
    direction: HostDirection,
    capacity: usize,
) -> Result<HostStreamConfigRequest> {
    Ok(HostStreamConfigRequest::new(
        asio_backend_symbol(),
        driver.id().clone(),
        StreamMedia::Pcm,
        direction,
        BufferPolicy::bounded(capacity)?,
    ))
}

fn capabilities_for(drivers: &[AsioDriver], fake: bool) -> Vec<HostBackendCapability> {
    let mut capabilities = Vec::new();
    if drivers.iter().any(|driver| driver.audio_inputs() > 0) {
        capabilities.push(HostBackendCapability::AudioInput);
    }
    if drivers.iter().any(|driver| driver.audio_outputs() > 0) {
        capabilities.push(HostBackendCapability::AudioOutput);
    }
    if drivers
        .iter()
        .any(|driver| driver.direction() == HostDirection::Duplex)
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

fn latency_for(direction: HostDirection, timing: AsioTiming) -> HostLatencyInfo {
    match direction {
        HostDirection::Input => HostLatencyInfo::new(timing.input_latency_frames(), 0),
        HostDirection::Output => HostLatencyInfo::new(0, timing.output_latency_frames()),
        HostDirection::Duplex => HostLatencyInfo::new(
            timing.input_latency_frames(),
            timing.output_latency_frames(),
        ),
    }
}
