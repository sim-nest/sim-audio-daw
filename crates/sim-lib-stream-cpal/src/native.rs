//! Native cpal hardware site support.

use std::{collections::VecDeque, rc::Rc, sync::Mutex};

use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, ClockDomain, PcmSampleFormat, StreamMedia, StreamPacket};
use sim_lib_stream_host::{
    AudioDeviceCard, AudioSite, AudioSiteKey, HostClockInfo, HostDirection, HostLatencyInfo,
    HostOpenStream, HostStreamConfig, HostStreamConfigRequest, HostStreamDriver,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

const CPAL_DEFAULT_FRAMES_PER_BUFFER: u32 = 512;

/// Number of buffered host items pulled per refill attempt inside the realtime
/// output callback.
const QUEUE_DRAIN_BATCH: usize = 64;

/// Returns the hardware backend identity symbol for cpal streams.
pub fn cpal_hardware_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", crate::cpal_audio_backend_candidate())
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
        let site_name = format!("cpal-hardware-{index}");
        let key = AudioSiteKey(Symbol::qualified("audio/site", site_name));
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
    ///
    /// The realtime callback drains PCM packets buffered on `queue` into the
    /// device output buffer, so a live host stream reaches the hardware; any
    /// shortfall is filled with silence.
    pub fn spawn(
        device: &cpal::Device,
        config: cpal::SupportedStreamConfig,
        queue: sim_lib_stream_host::HostCallbackQueue,
    ) -> Result<Self> {
        let stream = build_output_stream(device, config, queue)?;
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

fn build_output_stream(
    device: &cpal::Device,
    config: cpal::SupportedStreamConfig,
    queue: sim_lib_stream_host::HostCallbackQueue,
) -> Result<cpal::Stream> {
    match config.sample_format() {
        cpal::SampleFormat::I8 => build_typed_output_stream::<i8>(device, config, queue),
        cpal::SampleFormat::I16 => build_typed_output_stream::<i16>(device, config, queue),
        cpal::SampleFormat::I32 => build_typed_output_stream::<i32>(device, config, queue),
        cpal::SampleFormat::I64 => build_typed_output_stream::<i64>(device, config, queue),
        cpal::SampleFormat::U8 => build_typed_output_stream::<u8>(device, config, queue),
        cpal::SampleFormat::U16 => build_typed_output_stream::<u16>(device, config, queue),
        cpal::SampleFormat::U32 => build_typed_output_stream::<u32>(device, config, queue),
        cpal::SampleFormat::U64 => build_typed_output_stream::<u64>(device, config, queue),
        cpal::SampleFormat::F32 => build_typed_output_stream::<f32>(device, config, queue),
        cpal::SampleFormat::F64 => build_typed_output_stream::<f64>(device, config, queue),
        format => Err(Error::Eval(format!(
            "unsupported cpal output sample format {format}"
        ))),
    }
}

fn build_typed_output_stream<T>(
    device: &cpal::Device,
    config: cpal::SupportedStreamConfig,
    queue: sim_lib_stream_host::HostCallbackQueue,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let stream_config = config.config();
    let mut pending = CallbackPending::with_capacity(pending_capacity(&config));
    device
        .build_output_stream(
            &stream_config,
            move |samples: &mut [T], _callback_info| {
                fill_output_from_queue(&queue, &mut pending, samples);
            },
            |_err| {},
            None,
        )
        .map_err(|err| Error::Eval(format!("cpal output stream build failed: {err}")))
}

pub(crate) struct CallbackPending {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl CallbackPending {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn push_sample(&mut self, sample: f32) {
        if self.samples.len() < self.capacity {
            self.samples.push_back(sample);
        }
    }

    fn pop_sample(&mut self) -> f32 {
        self.samples.pop_front().unwrap_or(0.0)
    }

    #[cfg(test)]
    pub(crate) fn allocated_capacity(&self) -> usize {
        self.samples.capacity()
    }
}

pub(crate) fn pending_capacity(config: &cpal::SupportedStreamConfig) -> usize {
    let frames = frames_per_buffer(config.buffer_size()) as usize;
    frames
        .saturating_mul(config.channels() as usize)
        .saturating_mul(2)
}

/// Drains buffered PCM packets from `queue` into the interleaved `samples`
/// output buffer, buffering any surplus in `pending` for the next callback and
/// zero-filling whatever the queue cannot supply.
///
/// Runs on the realtime audio thread, so it only touches the non-blocking queue
/// drain and never grows the fixed-capacity `pending` scratch buffer.
pub(crate) fn fill_output_from_queue<T>(
    queue: &sim_lib_stream_host::HostCallbackQueue,
    pending: &mut CallbackPending,
    samples: &mut [T],
) where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    while pending.len() < samples.len() {
        match queue.drain(QUEUE_DRAIN_BATCH) {
            Ok(items) if !items.is_empty() => {
                for item in items {
                    if let StreamPacket::Pcm(pcm) = item.packet() {
                        match pcm.sample_format() {
                            PcmSampleFormat::F32 => {
                                for sample in pcm.samples_f32() {
                                    pending.push_sample(*sample);
                                }
                            }
                            PcmSampleFormat::I16 => {
                                for sample in pcm.samples_i16() {
                                    pending.push_sample(f32::from(*sample) / f32::from(i16::MAX));
                                }
                            }
                        }
                    }
                }
            }
            _ => break,
        }
    }
    for sample in samples.iter_mut() {
        *sample = T::from_sample(pending.pop_sample());
    }
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
