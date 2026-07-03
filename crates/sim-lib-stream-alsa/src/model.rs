use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_audio::{PcmSampleFormat, PcmSpec};
use sim_lib_stream_host::HostDirection;

/// Supported ALSA PCM name family.
///
/// Distinguishes the three PCM naming forms this adapter accepts when parsing
/// device strings: the routed `default` device, raw `hw:*` hardware access, and
/// the format-converting `plughw:*` plugin layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlsaPcmNameKind {
    /// The ALSA `default` PCM, a routed fallback device.
    Default,
    /// A raw `hw:*` device addressed by card or `card,device`.
    Hw,
    /// A `plughw:*` device that adds automatic format and rate conversion.
    PlugHw,
}

/// Parsed ALSA PCM name accepted by this adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlsaPcmName {
    raw: String,
    kind: AlsaPcmNameKind,
}

/// SIM-visible ALSA PCM device metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlsaPcmDevice {
    id: Symbol,
    pcm_name: AlsaPcmName,
    display_name: String,
    direction: HostDirection,
    channels: usize,
    sample_rate_hz: u32,
    sample_format: PcmSampleFormat,
    buffer_frames: usize,
}

impl AlsaPcmName {
    /// Parses an ALSA PCM name, accepting `default`, `hw:*`, and `plughw:*`.
    ///
    /// The `hw:*` and `plughw:*` tails must be non-empty and whitespace-free.
    /// Any other name is rejected with an `Error::Eval`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_alsa::{AlsaPcmName, AlsaPcmNameKind};
    ///
    /// let name = AlsaPcmName::parse("plughw:1,0").unwrap();
    /// assert_eq!(name.kind(), AlsaPcmNameKind::PlugHw);
    /// assert_eq!(name.raw(), "plughw:1,0");
    /// assert!(!name.is_default());
    /// assert!(AlsaPcmName::parse("oss:0").is_err());
    /// ```
    pub fn parse(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        let kind = if raw == "default" {
            AlsaPcmNameKind::Default
        } else if let Some(rest) = raw.strip_prefix("hw:") {
            validate_pcm_tail(rest, "hw")?;
            AlsaPcmNameKind::Hw
        } else if let Some(rest) = raw.strip_prefix("plughw:") {
            validate_pcm_tail(rest, "plughw")?;
            AlsaPcmNameKind::PlugHw
        } else {
            return Err(Error::Eval(format!(
                "unsupported ALSA PCM name {raw}; expected default, hw:*, or plughw:*"
            )));
        };
        Ok(Self { raw, kind })
    }

    /// Returns the original PCM name string as parsed.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns the parsed name family.
    pub fn kind(&self) -> AlsaPcmNameKind {
        self.kind
    }

    /// Returns `true` when this is the ALSA `default` PCM.
    pub fn is_default(&self) -> bool {
        self.kind == AlsaPcmNameKind::Default
    }

    /// Builds the SIM device `Symbol` for this PCM name in the given
    /// `direction`, of the form `alsa/<raw>/<role>` where the role suffix is
    /// `capture`, `playback`, or `duplex`.
    pub fn device_symbol(&self, direction: HostDirection) -> Symbol {
        let suffix = match direction {
            HostDirection::Input => "capture",
            HostDirection::Output => "playback",
            HostDirection::Duplex => "duplex",
        };
        Symbol::new(format!("alsa/{}/{suffix}", self.raw))
    }
}

impl AlsaPcmDevice {
    /// Builds an output (playback) PCM device with the F32 sample format.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_alsa::AlsaPcmDevice;
    ///
    /// let device = AlsaPcmDevice::playback("hw:0,0", "Card 0", 2, 48_000).unwrap();
    /// assert_eq!(device.channels(), 2);
    /// assert_eq!(device.sample_rate_hz(), 48_000);
    /// assert!(!device.is_default());
    /// ```
    pub fn playback(
        pcm_name: impl Into<String>,
        display_name: impl Into<String>,
        channels: usize,
        sample_rate_hz: u32,
    ) -> Result<Self> {
        Self::new(
            AlsaPcmName::parse(pcm_name)?,
            display_name,
            HostDirection::Output,
            channels,
            sample_rate_hz,
            PcmSampleFormat::F32,
        )
    }

    /// Builds an input (capture) PCM device with the F32 sample format.
    pub fn capture(
        pcm_name: impl Into<String>,
        display_name: impl Into<String>,
        channels: usize,
        sample_rate_hz: u32,
    ) -> Result<Self> {
        Self::new(
            AlsaPcmName::parse(pcm_name)?,
            display_name,
            HostDirection::Input,
            channels,
            sample_rate_hz,
            PcmSampleFormat::F32,
        )
    }

    /// Builds the routed `default` PCM device for playback.
    pub fn default_playback(channels: usize, sample_rate_hz: u32) -> Result<Self> {
        Self::playback("default", "ALSA Default Playback", channels, sample_rate_hz)
    }

    /// Builds the routed `default` PCM device for capture.
    pub fn default_capture(channels: usize, sample_rate_hz: u32) -> Result<Self> {
        Self::capture("default", "ALSA Default Capture", channels, sample_rate_hz)
    }

    /// Builds a PCM device from explicit parts, defaulting the buffer to 256
    /// frames.
    ///
    /// Returns an `Error::Eval` when `channels` or `sample_rate_hz` is zero.
    /// The device `Symbol` id is derived from the name and `direction`.
    pub fn new(
        pcm_name: AlsaPcmName,
        display_name: impl Into<String>,
        direction: HostDirection,
        channels: usize,
        sample_rate_hz: u32,
        sample_format: PcmSampleFormat,
    ) -> Result<Self> {
        if channels == 0 {
            return Err(Error::Eval(
                "ALSA PCM channel count must be greater than zero".to_owned(),
            ));
        }
        if sample_rate_hz == 0 {
            return Err(Error::Eval(
                "ALSA PCM sample rate must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id: pcm_name.device_symbol(direction),
            pcm_name,
            display_name: display_name.into(),
            direction,
            channels,
            sample_rate_hz,
            sample_format,
            buffer_frames: 256,
        })
    }

    /// Returns a copy of this device with the buffer size set to
    /// `buffer_frames`, which must be greater than zero.
    pub fn with_buffer_frames(mut self, buffer_frames: usize) -> Result<Self> {
        if buffer_frames == 0 {
            return Err(Error::Eval(
                "ALSA PCM buffer frame count must be greater than zero".to_owned(),
            ));
        }
        self.buffer_frames = buffer_frames;
        Ok(self)
    }

    /// Returns the device's stable SIM identity symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the parsed ALSA PCM name.
    pub fn pcm_name(&self) -> &AlsaPcmName {
        &self.pcm_name
    }

    /// Returns the human-readable display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the device direction (input, output, or duplex).
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

    /// Returns the configured buffer size in frames.
    pub fn buffer_frames(&self) -> usize {
        self.buffer_frames
    }

    /// Builds the PCM spec (channels, rate, format) for this device.
    pub fn spec(&self) -> Result<PcmSpec> {
        match self.sample_format {
            PcmSampleFormat::I16 => PcmSpec::i16(self.channels, self.sample_rate_hz),
            PcmSampleFormat::F32 => PcmSpec::f32(self.channels, self.sample_rate_hz),
        }
    }

    /// Returns `true` when this device wraps the ALSA `default` PCM.
    pub fn is_default(&self) -> bool {
        self.pcm_name.is_default()
    }

    /// Returns `true` when the device can serve the `requested` direction.
    ///
    /// A duplex device satisfies any requested direction.
    pub fn is_compatible_with(&self, requested: HostDirection) -> bool {
        self.direction == requested || self.direction == HostDirection::Duplex
    }

    /// Returns the device's port symbol, the device id with a `/port` suffix.
    pub fn port_symbol(&self) -> Symbol {
        Symbol::new(format!("{}/port", self.id))
    }
}

fn validate_pcm_tail(tail: &str, family: &str) -> Result<()> {
    if tail.is_empty() {
        return Err(Error::Eval(format!(
            "ALSA {family}: name must include a card or card,device tail"
        )));
    }
    if tail.chars().any(char::is_whitespace) {
        return Err(Error::Eval(format!(
            "ALSA {family}: name must not contain whitespace"
        )));
    }
    Ok(())
}
