use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_audio::PcmSpec;
use sim_lib_stream_host::HostDirection;

use crate::coreaudio_backend_symbol;

/// Timing metadata accepted by a CoreAudio render callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreAudioTiming {
    sample_rate_hz: u32,
    buffer_frames: usize,
    input_latency_frames: u32,
    output_latency_frames: u32,
}

/// SIM-visible CoreAudio device metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreAudioDevice {
    id: Symbol,
    name: String,
    direction: HostDirection,
    channels: usize,
    timing: CoreAudioTiming,
    default_output: bool,
    default_input: bool,
}

impl CoreAudioTiming {
    /// Builds timing metadata from a sample rate, buffer size, and the input
    /// and output latencies, all measured in frames.
    ///
    /// Returns an error when the sample rate or buffer size is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_coreaudio::CoreAudioTiming;
    ///
    /// let timing = CoreAudioTiming::new(48_000, 128, 128, 128).unwrap();
    /// assert_eq!(timing.sample_rate_hz(), 48_000);
    /// assert!(CoreAudioTiming::new(0, 128, 0, 0).is_err());
    /// ```
    pub fn new(
        sample_rate_hz: u32,
        buffer_frames: usize,
        input_latency_frames: u32,
        output_latency_frames: u32,
    ) -> Result<Self> {
        if sample_rate_hz == 0 {
            return Err(Error::Eval(
                "CoreAudio sample rate must be greater than zero".to_owned(),
            ));
        }
        if buffer_frames == 0 {
            return Err(Error::Eval(
                "CoreAudio buffer size must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            sample_rate_hz,
            buffer_frames,
            input_latency_frames,
            output_latency_frames,
        })
    }

    /// Returns a default low-latency timing: 48 kHz, 128-frame buffer, and
    /// 128-frame input and output latencies.
    pub fn default_low_latency() -> Self {
        Self::new(48_000, 128, 128, 128).expect("valid CoreAudio timing")
    }

    /// Returns the sample rate in hertz.
    pub fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }

    /// Returns the callback buffer size in frames.
    pub fn buffer_frames(self) -> usize {
        self.buffer_frames
    }

    /// Returns the input latency in frames.
    pub fn input_latency_frames(self) -> u32 {
        self.input_latency_frames
    }

    /// Returns the output latency in frames.
    pub fn output_latency_frames(self) -> u32 {
        self.output_latency_frames
    }
}

impl CoreAudioDevice {
    /// Builds an output-direction device. See [`CoreAudioDevice::new`].
    pub fn output(
        id: impl Into<String>,
        name: impl Into<String>,
        channels: usize,
        timing: CoreAudioTiming,
    ) -> Result<Self> {
        Self::new(id, name, HostDirection::Output, channels, timing)
    }

    /// Builds an input-direction device. See [`CoreAudioDevice::new`].
    pub fn input(
        id: impl Into<String>,
        name: impl Into<String>,
        channels: usize,
        timing: CoreAudioTiming,
    ) -> Result<Self> {
        Self::new(id, name, HostDirection::Input, channels, timing)
    }

    /// Builds a duplex (input and output) device. See [`CoreAudioDevice::new`].
    pub fn duplex(
        id: impl Into<String>,
        name: impl Into<String>,
        channels: usize,
        timing: CoreAudioTiming,
    ) -> Result<Self> {
        Self::new(id, name, HostDirection::Duplex, channels, timing)
    }

    /// Builds a device from its id, display name, direction, channel count, and
    /// timing.
    ///
    /// The device starts flagged as neither the default input nor the default
    /// output. Returns an error when `channels` is zero.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        direction: HostDirection,
        channels: usize,
        timing: CoreAudioTiming,
    ) -> Result<Self> {
        if channels == 0 {
            return Err(Error::Eval(
                "CoreAudio device channel count must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id: Symbol::new(id.into()),
            name: name.into(),
            direction,
            channels,
            timing,
            default_output: false,
            default_input: false,
        })
    }

    /// Returns this device flagged as the default output device.
    pub fn with_default_output(mut self) -> Self {
        self.default_output = true;
        self
    }

    /// Returns this device flagged as the default input device.
    pub fn with_default_input(mut self) -> Self {
        self.default_input = true;
        self
    }

    /// Returns the device's identifying symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the device's human-readable display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the device's I/O direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the device's channel count.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Returns the device's timing metadata.
    pub fn timing(&self) -> CoreAudioTiming {
        self.timing
    }

    /// Returns whether this device is flagged as the default output.
    pub fn default_output(&self) -> bool {
        self.default_output
    }

    /// Returns whether this device is flagged as the default input.
    pub fn default_input(&self) -> bool {
        self.default_input
    }

    /// Returns the f32 PCM spec for this device's channels and sample rate.
    pub fn spec(&self) -> Result<PcmSpec> {
        PcmSpec::f32(self.channels, self.timing.sample_rate_hz)
    }

    /// Returns whether this device can serve a stream in the requested
    /// direction.
    ///
    /// A duplex device is compatible with any requested direction.
    pub fn is_compatible_with(&self, requested: HostDirection) -> bool {
        self.direction == requested || self.direction == HostDirection::Duplex
    }

    /// Returns the `<id>/port` symbol naming this device's host port.
    pub fn port_symbol(&self) -> Symbol {
        Symbol::new(format!("{}/port", self.id))
    }

    /// Returns the CoreAudio backend symbol this device belongs to.
    pub fn backend(&self) -> Symbol {
        coreaudio_backend_symbol()
    }
}

/// Preferred macOS PCM backend order: portable first, native when needed.
pub fn macos_audio_backend_priority() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream/host", "portaudio"),
        Symbol::qualified("stream/host", "rtaudio"),
        coreaudio_backend_symbol(),
    ]
}

/// Preferred macOS MIDI backend order keeps RtMidi first.
pub fn macos_midi_backend_priority() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream/host", "rtmidi"),
        Symbol::qualified("stream/host", "coremidi"),
    ]
}
