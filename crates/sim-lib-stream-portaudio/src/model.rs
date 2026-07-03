use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_audio::{PcmSampleFormat, PcmSpec};
use sim_lib_stream_host::HostDirection;

use crate::portaudio_backend_symbol;

/// SIM-visible PortAudio device metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortAudioDevice {
    id: Symbol,
    name: String,
    direction: HostDirection,
    channels: usize,
    sample_rate_hz: u32,
    sample_format: PcmSampleFormat,
    default_output: bool,
    default_input: bool,
    buffer_frames: usize,
}

impl PortAudioDevice {
    /// Builds an output-direction device with the [`PcmSampleFormat::F32`]
    /// sample format.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_portaudio::PortAudioDevice;
    ///
    /// let device = PortAudioDevice::output("portaudio/out", "Out", 2, 48_000).unwrap();
    /// assert_eq!(device.channels(), 2);
    /// assert_eq!(device.sample_rate_hz(), 48_000);
    /// assert!(!device.default_output());
    /// ```
    pub fn output(
        id: impl Into<String>,
        name: impl Into<String>,
        channels: usize,
        sample_rate_hz: u32,
    ) -> Result<Self> {
        Self::new(
            id,
            name,
            HostDirection::Output,
            channels,
            sample_rate_hz,
            PcmSampleFormat::F32,
        )
    }

    /// Builds an input-direction device with the [`PcmSampleFormat::F32`]
    /// sample format.
    pub fn input(
        id: impl Into<String>,
        name: impl Into<String>,
        channels: usize,
        sample_rate_hz: u32,
    ) -> Result<Self> {
        Self::new(
            id,
            name,
            HostDirection::Input,
            channels,
            sample_rate_hz,
            PcmSampleFormat::F32,
        )
    }

    /// Builds a device with an explicit direction and sample format.
    ///
    /// The buffer size defaults to 256 frames and neither default-output nor
    /// default-input is set. Returns an error when `channels` or
    /// `sample_rate_hz` is zero.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        direction: HostDirection,
        channels: usize,
        sample_rate_hz: u32,
        sample_format: PcmSampleFormat,
    ) -> Result<Self> {
        if channels == 0 {
            return Err(Error::Eval(
                "PortAudio device channel count must be greater than zero".to_owned(),
            ));
        }
        if sample_rate_hz == 0 {
            return Err(Error::Eval(
                "PortAudio device sample rate must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id: Symbol::new(id.into()),
            name: name.into(),
            direction,
            channels,
            sample_rate_hz,
            sample_format,
            default_output: false,
            default_input: false,
            buffer_frames: 256,
        })
    }

    /// Marks this device as the default output, returning the updated device.
    pub fn with_default_output(mut self) -> Self {
        self.default_output = true;
        self
    }

    /// Marks this device as the default input, returning the updated device.
    pub fn with_default_input(mut self) -> Self {
        self.default_input = true;
        self
    }

    /// Sets the per-callback buffer size in frames, returning the updated
    /// device.
    ///
    /// Returns an error when `buffer_frames` is zero.
    pub fn with_buffer_frames(mut self, buffer_frames: usize) -> Result<Self> {
        if buffer_frames == 0 {
            return Err(Error::Eval(
                "PortAudio buffer frame count must be greater than zero".to_owned(),
            ));
        }
        self.buffer_frames = buffer_frames;
        Ok(self)
    }

    /// Returns the device identifier symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the human-readable device name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the device direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the channel count.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Returns the sample rate in hertz.
    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// Returns the PCM sample format.
    pub fn sample_format(&self) -> PcmSampleFormat {
        self.sample_format
    }

    /// Returns whether this device is flagged as the default output.
    pub fn default_output(&self) -> bool {
        self.default_output
    }

    /// Returns whether this device is flagged as the default input.
    pub fn default_input(&self) -> bool {
        self.default_input
    }

    /// Returns the per-callback buffer size in frames.
    pub fn buffer_frames(&self) -> usize {
        self.buffer_frames
    }

    /// Builds the [`PcmSpec`] for this device's channels, rate, and format.
    pub fn spec(&self) -> Result<PcmSpec> {
        match self.sample_format {
            PcmSampleFormat::I16 => PcmSpec::i16(self.channels, self.sample_rate_hz),
            PcmSampleFormat::F32 => PcmSpec::f32(self.channels, self.sample_rate_hz),
        }
    }

    /// Returns whether this device can serve the `requested` direction.
    ///
    /// A duplex device matches any requested direction; otherwise the
    /// directions must be equal.
    pub fn is_compatible_with(&self, requested: HostDirection) -> bool {
        self.direction == requested || self.direction == HostDirection::Duplex
    }

    /// Returns the port symbol derived from this device's identifier.
    pub fn port_symbol(&self) -> Symbol {
        Symbol::new(format!("{}/port", self.id))
    }

    /// Returns the PortAudio host backend symbol this device belongs to.
    pub fn backend(&self) -> Symbol {
        portaudio_backend_symbol()
    }
}

/// Returns the bootstrap backend priority before platform-specific backends
/// exist.
///
/// The order places native PipeWire first, then PortAudio as the portable
/// fallback, followed by RtAudio and ALSA.
///
/// # Examples
///
/// ```
/// use sim_lib_stream_portaudio::portaudio_backend_priority;
///
/// let priority = portaudio_backend_priority();
/// assert_eq!(priority.len(), 4);
/// ```
pub fn portaudio_backend_priority() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream/host", "pipewire"),
        portaudio_backend_symbol(),
        Symbol::qualified("stream/host", "rtaudio"),
        Symbol::qualified("stream/host", "alsa"),
    ]
}
