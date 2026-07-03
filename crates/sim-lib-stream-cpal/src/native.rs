//! Native cpal hardware site support.

use std::{rc::Rc, sync::Mutex};

use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, ClockDomain, StreamMedia};
use sim_lib_stream_host::{
    AudioDeviceCard, AudioSite, AudioSiteKey, HostClockInfo, HostDirection, HostLatencyInfo,
    HostOpenStream, HostStreamConfig, HostStreamConfigRequest, HostStreamDriver,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

const CPAL_DEFAULT_FRAMES_PER_BUFFER: u32 = 512;

/// Returns the hardware backend identity symbol for cpal streams.
pub fn cpal_hardware_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "cpal")
}

/// Native cpal output site for one host device.
pub struct CpalHardwareSite {
    key: AudioSiteKey,
    card: AudioDeviceCard,
    device: cpal::Device,
}

impl CpalHardwareSite {
    /// Builds a cpal hardware site from an enumerated output device.
    pub fn new(index: usize, device: cpal::Device) -> Self {
        let name = device.name().unwrap_or_else(|_| "cpal output".to_owned());
        let default_config = device.default_output_config().ok();
        let channels_out = default_config
            .as_ref()
            .map(|config| config.channels())
            .unwrap_or(2);
        let sample_rates = sample_rates(&device, default_config.as_ref());
        let key = AudioSiteKey::new(&format!("sim:cpal-hardware-{index}"));
        let card = AudioDeviceCard {
            key: key.clone(),
            display_name: name,
            channels_out,
            channels_in: 0,
            sample_rates,
            hardware_required: true,
        };
        Self { key, card, device }
    }

    fn cpal_config(&self) -> Result<cpal::SupportedStreamConfig> {
        self.device
            .default_output_config()
            .map_err(|err| Error::Eval(format!("cpal default output config failed: {err}")))
    }
}

impl AudioSite for CpalHardwareSite {
    fn key(&self) -> &AudioSiteKey {
        &self.key
    }

    fn card(&self) -> &AudioDeviceCard {
        &self.card
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != &cpal_hardware_backend_symbol() {
            return Err(Error::Eval(format!(
                "cpal hardware site cannot open {} requests",
                request.backend()
            )));
        }
        if request.media() != StreamMedia::Pcm {
            return Err(Error::TypeMismatch {
                expected: "PCM stream request",
                found: "non-PCM stream request",
            });
        }
        if request.direction() != HostDirection::Output {
            return Err(Error::TypeMismatch {
                expected: "output stream request",
                found: "non-output stream request",
            });
        }

        let cpal_config = self.cpal_config()?;
        let config = config_from_cpal(request.device().clone(), &cpal_config)?;
        HostOpenStream::try_new_realtime_local_audio_with_driver(config, |queue| {
            let driver = CpalDriver::spawn(&self.device, cpal_config, queue)?;
            Ok(Rc::new(driver))
        })
    }
}

/// Maps cpal's selected output configuration to a SIM host-stream config.
pub fn config_from_cpal(
    device: Symbol,
    config: &cpal::SupportedStreamConfig,
) -> Result<HostStreamConfig> {
    let frames = frames_per_buffer(config.buffer_size());
    let request = HostStreamConfigRequest::new(
        cpal_hardware_backend_symbol(),
        device,
        StreamMedia::Pcm,
        HostDirection::Output,
        BufferPolicy::bounded(frames as usize)?,
    )
    .with_clock(ClockDomain::Sample.symbol());
    Ok(HostStreamConfig::from_request(
        request,
        HostLatencyInfo::new(0, frames),
        HostClockInfo::new(
            ClockDomain::Sample.symbol(),
            Some(config.sample_rate().0),
            false,
        ),
    ))
}

/// Driver that keeps a native cpal stream alive for the opened host stream.
pub struct CpalDriver {
    stream: Mutex<Option<CpalDriverHandle>>,
}

impl CpalDriver {
    /// Builds and starts a cpal output stream for an opened SIM host stream.
    pub fn spawn(
        device: &cpal::Device,
        config: cpal::SupportedStreamConfig,
        _queue: sim_lib_stream_host::HostCallbackQueue,
    ) -> Result<Self> {
        let stream_config = config.config();
        let stream = device
            .build_output_stream(
                &stream_config,
                |samples: &mut [f32], _callback_info| {
                    // SAFETY: `samples.as_mut_ptr()` comes from this callback's
                    // exclusive `&mut [f32]`; the reconstructed slice uses the
                    // same length and does not escape the callback.
                    let out = unsafe {
                        std::slice::from_raw_parts_mut(samples.as_mut_ptr(), samples.len())
                    };
                    for sample in out {
                        *sample = 0.0;
                    }
                },
                |_err| {},
                None,
            )
            .map_err(|err| Error::Eval(format!("cpal output stream build failed: {err}")))?;
        stream
            .play()
            .map_err(|err| Error::Eval(format!("cpal output stream start failed: {err}")))?;
        Ok(Self {
            stream: Mutex::new(Some(CpalDriverHandle::Native { _stream: stream })),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_drop_probe(probe: impl Send + 'static) -> Self {
        Self {
            stream: Mutex::new(Some(CpalDriverHandle::Test {
                _probe: Box::new(probe),
            })),
        }
    }

    fn stop(&self) {
        if let Ok(mut stream) = self.stream.lock() {
            let _ = stream.take();
        }
    }
}

impl HostStreamDriver for CpalDriver {
    fn shutdown(&self) -> Result<()> {
        self.stop();
        Ok(())
    }
}

impl Drop for CpalDriver {
    fn drop(&mut self) {
        self.stop();
    }
}

enum CpalDriverHandle {
    Native {
        _stream: cpal::Stream,
    },
    #[cfg(test)]
    Test {
        _probe: Box<dyn Send>,
    },
}

/// Enumerates output devices as cpal hardware audio sites.
pub fn enumerate_cpal_sites() -> Result<Vec<CpalHardwareSite>> {
    enumerate_cpal_hardware_sites()
}

/// Enumerates output devices as cpal hardware audio sites.
pub fn enumerate_cpal_hardware_sites() -> Result<Vec<CpalHardwareSite>> {
    let host = cpal::default_host();
    let devices = host
        .output_devices()
        .map_err(|err| Error::Eval(format!("cpal output device enumeration failed: {err}")))?;
    Ok(devices
        .enumerate()
        .map(|(index, device)| CpalHardwareSite::new(index, device))
        .collect())
}

fn sample_rates(
    device: &cpal::Device,
    default_config: Option<&cpal::SupportedStreamConfig>,
) -> Vec<u32> {
    let mut rates = match device.supported_output_configs() {
        Ok(configs) => configs
            .flat_map(|config| [config.min_sample_rate().0, config.max_sample_rate().0])
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    if let Some(config) = default_config {
        rates.push(config.sample_rate().0);
    }
    if rates.is_empty() {
        rates.push(48_000);
    }
    rates.sort_unstable();
    rates.dedup();
    rates
}

fn frames_per_buffer(buffer_size: &cpal::SupportedBufferSize) -> u32 {
    match buffer_size {
        cpal::SupportedBufferSize::Range { min, max } => {
            CPAL_DEFAULT_FRAMES_PER_BUFFER.clamp(*min, *max)
        }
        cpal::SupportedBufferSize::Unknown => CPAL_DEFAULT_FRAMES_PER_BUFFER,
    }
}
