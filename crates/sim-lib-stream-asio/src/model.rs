use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_audio::PcmSpec;
use sim_lib_stream_core::StreamMedia;
use sim_lib_stream_host::HostDirection;

/// Timing metadata accepted by an ASIO buffer switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AsioTiming {
    sample_rate_hz: u32,
    buffer_frames: usize,
    input_latency_frames: u32,
    output_latency_frames: u32,
}

/// SIM-visible ASIO driver metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsioDriver {
    id: Symbol,
    name: String,
    timing: AsioTiming,
    audio_inputs: usize,
    audio_outputs: usize,
}

/// Routable ASIO port owned by a driver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsioPort {
    id: Symbol,
    driver: Symbol,
    name: String,
    media: StreamMedia,
    direction: HostDirection,
    index: usize,
}

impl AsioTiming {
    /// Builds timing from a sample rate, buffer size, and reported input/output
    /// latencies, all measured in frames except the rate in hertz.
    ///
    /// Returns an error if `sample_rate_hz` or `buffer_frames` is zero.
    pub fn new(
        sample_rate_hz: u32,
        buffer_frames: usize,
        input_latency_frames: u32,
        output_latency_frames: u32,
    ) -> Result<Self> {
        if sample_rate_hz == 0 {
            return Err(Error::Eval(
                "ASIO sample rate must be greater than zero".to_owned(),
            ));
        }
        if buffer_frames == 0 {
            return Err(Error::Eval(
                "ASIO buffer size must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            sample_rate_hz,
            buffer_frames,
            input_latency_frames,
            output_latency_frames,
        })
    }

    /// Returns a 48 kHz, 128-frame timing with 128-frame input/output latency,
    /// a typical low-latency pro-audio configuration.
    pub fn pro_audio_default() -> Self {
        Self::new(48_000, 128, 128, 128).expect("valid ASIO timing")
    }

    /// Returns the sample rate in hertz.
    pub fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }

    /// Returns the buffer switch size in frames.
    pub fn buffer_frames(self) -> usize {
        self.buffer_frames
    }

    /// Returns the reported input latency in frames.
    pub fn input_latency_frames(self) -> u32 {
        self.input_latency_frames
    }

    /// Returns the reported output latency in frames.
    pub fn output_latency_frames(self) -> u32 {
        self.output_latency_frames
    }
}

impl AsioDriver {
    /// Builds a driver with the given `name`, `timing`, and audio channel
    /// counts, deriving the stable id `asio/<name>/driver`.
    ///
    /// Returns an error if `name` is empty or the driver exposes no audio
    /// channels at all.
    pub fn new(
        name: impl Into<String>,
        timing: AsioTiming,
        audio_inputs: usize,
        audio_outputs: usize,
    ) -> Result<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(Error::Eval("ASIO driver name must not be empty".to_owned()));
        }
        if audio_inputs == 0 && audio_outputs == 0 {
            return Err(Error::Eval(
                "ASIO driver must expose at least one audio channel".to_owned(),
            ));
        }
        Ok(Self {
            id: Symbol::new(format!("asio/{name}/driver")),
            name,
            timing,
            audio_inputs,
            audio_outputs,
        })
    }

    /// Returns the bundled `SIM-ASIO` driver: stereo in/out with
    /// [`AsioTiming::pro_audio_default`] timing.
    pub fn sim_default() -> Result<Self> {
        Self::new("SIM-ASIO", AsioTiming::pro_audio_default(), 2, 2)
    }

    /// Returns the stable driver id (`asio/<name>/driver`).
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the human-readable driver name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the driver's buffer timing.
    pub fn timing(&self) -> AsioTiming {
        self.timing
    }

    /// Returns the number of audio input channels the driver exposes.
    pub fn audio_inputs(&self) -> usize {
        self.audio_inputs
    }

    /// Returns the number of audio output channels the driver exposes.
    pub fn audio_outputs(&self) -> usize {
        self.audio_outputs
    }

    /// Returns the audio direction implied by the input/output channel counts,
    /// treating a channelless driver as [`HostDirection::Duplex`].
    pub fn direction(&self) -> HostDirection {
        match (self.audio_inputs > 0, self.audio_outputs > 0) {
            (true, true) => HostDirection::Duplex,
            (true, false) => HostDirection::Input,
            (false, true) => HostDirection::Output,
            (false, false) => HostDirection::Duplex,
        }
    }

    /// Reports whether the driver can serve the `requested` direction, which
    /// holds for an exact match or for any request when the driver is duplex.
    pub fn is_compatible_with(&self, requested: HostDirection) -> bool {
        self.direction() == requested || self.direction() == HostDirection::Duplex
    }

    /// Returns an f32 PCM spec for the driver's output channels (at least one)
    /// at the driver sample rate.
    pub fn output_spec(&self) -> Result<PcmSpec> {
        PcmSpec::f32(self.audio_outputs.max(1), self.timing.sample_rate_hz)
    }

    /// Returns one [`AsioPort`] per input channel followed by one per output
    /// channel, with ids of the form `<driver-id>/input_<n>` and
    /// `<driver-id>/output_<n>`.
    pub fn ports(&self) -> Vec<AsioPort> {
        let inputs = (0..self.audio_inputs).map(|index| {
            AsioPort::new(
                Symbol::new(format!("{}/input_{index}", self.id)),
                self.id.clone(),
                format!("input_{index}"),
                HostDirection::Input,
                index,
            )
        });
        let outputs = (0..self.audio_outputs).map(|index| {
            AsioPort::new(
                Symbol::new(format!("{}/output_{index}", self.id)),
                self.id.clone(),
                format!("output_{index}"),
                HostDirection::Output,
                index,
            )
        });
        inputs.chain(outputs).collect()
    }
}

impl AsioPort {
    /// Builds a PCM port with the given id, owning driver id, name, direction,
    /// and per-direction channel index.
    pub fn new(
        id: Symbol,
        driver: Symbol,
        name: String,
        direction: HostDirection,
        index: usize,
    ) -> Self {
        Self {
            id,
            driver,
            name,
            media: StreamMedia::Pcm,
            direction,
            index,
        }
    }

    /// Returns the stable port id.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the id of the driver that owns this port.
    pub fn driver(&self) -> &Symbol {
        &self.driver
    }

    /// Returns the human-readable port name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the port media, always [`StreamMedia::Pcm`] for ASIO.
    pub fn media(&self) -> StreamMedia {
        self.media
    }

    /// Returns whether the port is an input or output.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the channel index within the port's direction.
    pub fn index(&self) -> usize {
        self.index
    }
}

/// Lists the prerequisites a downstream provider must satisfy to build a native
/// ASIO backend, none of which this validation-only crate supplies.
pub fn asio_sdk_build_requirements() -> Vec<&'static str> {
    vec![
        "Windows target",
        "Steinberg ASIO SDK headers supplied outside this repository",
        "vendor driver import library or COM registration",
        "SIM stream-asio feature enabled explicitly",
    ]
}
