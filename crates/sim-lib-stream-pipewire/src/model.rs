use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_audio::{PcmSampleFormat, PcmSpec};
use sim_lib_stream_host::HostDirection;

use crate::pipewire_backend_symbol;

/// PipeWire timing metadata accepted by an open stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipeWireTiming {
    sample_rate_hz: u32,
    quantum_frames: usize,
    input_latency_frames: u32,
    output_latency_frames: u32,
}

/// SIM-visible PipeWire node metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeWireNode {
    id: Symbol,
    client_name: String,
    node_name: String,
    direction: HostDirection,
    channels: usize,
    sample_format: PcmSampleFormat,
    timing: PipeWireTiming,
}

/// Visible PipeWire port owned by a node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeWirePort {
    id: Symbol,
    node: Symbol,
    name: String,
    direction: HostDirection,
    channel: usize,
}

impl PipeWireTiming {
    /// Builds timing metadata from a sample rate, quantum, and latencies.
    ///
    /// Errors if `sample_rate_hz` or `quantum_frames` is zero.
    pub fn new(
        sample_rate_hz: u32,
        quantum_frames: usize,
        input_latency_frames: u32,
        output_latency_frames: u32,
    ) -> Result<Self> {
        if sample_rate_hz == 0 {
            return Err(Error::Eval(
                "PipeWire sample rate must be greater than zero".to_owned(),
            ));
        }
        if quantum_frames == 0 {
            return Err(Error::Eval(
                "PipeWire quantum must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            sample_rate_hz,
            quantum_frames,
            input_latency_frames,
            output_latency_frames,
        })
    }

    /// Returns a typical desktop profile: 48 kHz, 128-frame quantum and latency.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_pipewire::PipeWireTiming;
    ///
    /// let timing = PipeWireTiming::desktop_default();
    /// assert_eq!(timing.sample_rate_hz(), 48_000);
    /// assert_eq!(timing.quantum_frames(), 128);
    /// ```
    pub fn desktop_default() -> Self {
        Self::new(48_000, 128, 128, 128).expect("valid desktop timing")
    }

    /// Returns the sample rate in hertz.
    pub fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }

    /// Returns the PipeWire quantum (graph period) in frames.
    pub fn quantum_frames(self) -> usize {
        self.quantum_frames
    }

    /// Returns the reported input (capture) latency in frames.
    pub fn input_latency_frames(self) -> u32 {
        self.input_latency_frames
    }

    /// Returns the reported output (playback) latency in frames.
    pub fn output_latency_frames(self) -> u32 {
        self.output_latency_frames
    }
}

impl PipeWireNode {
    /// Builds an output (playback) node with `f32` samples.
    ///
    /// Errors if `channels` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_pipewire::{PipeWireNode, PipeWireTiming};
    ///
    /// let node = PipeWireNode::playback(
    ///     "pw/out",
    ///     "PipeWire",
    ///     "Sink",
    ///     2,
    ///     PipeWireTiming::desktop_default(),
    /// )
    /// .unwrap();
    /// assert_eq!(node.channels(), 2);
    /// assert_eq!(node.ports().len(), 2);
    /// ```
    pub fn playback(
        id: impl Into<String>,
        client_name: impl Into<String>,
        node_name: impl Into<String>,
        channels: usize,
        timing: PipeWireTiming,
    ) -> Result<Self> {
        Self::new(
            id,
            client_name,
            node_name,
            HostDirection::Output,
            channels,
            PcmSampleFormat::F32,
            timing,
        )
    }

    /// Builds an input (capture) node with `f32` samples.
    ///
    /// Errors if `channels` is zero.
    pub fn capture(
        id: impl Into<String>,
        client_name: impl Into<String>,
        node_name: impl Into<String>,
        channels: usize,
        timing: PipeWireTiming,
    ) -> Result<Self> {
        Self::new(
            id,
            client_name,
            node_name,
            HostDirection::Input,
            channels,
            PcmSampleFormat::F32,
            timing,
        )
    }

    /// Builds the duplex SIM runtime client node (`pipewire/sim/client`).
    ///
    /// The node is named `SIM` with direction [`HostDirection::Duplex`] and
    /// `f32` samples. Errors if `channels` is zero.
    pub fn duplex_sim_client(channels: usize, timing: PipeWireTiming) -> Result<Self> {
        Self::new(
            "pipewire/sim/client",
            "SIM",
            "SIM Runtime",
            HostDirection::Duplex,
            channels,
            PcmSampleFormat::F32,
            timing,
        )
    }

    /// Builds a node from explicit direction, channels, and sample format.
    ///
    /// Errors if `channels` is zero.
    pub fn new(
        id: impl Into<String>,
        client_name: impl Into<String>,
        node_name: impl Into<String>,
        direction: HostDirection,
        channels: usize,
        sample_format: PcmSampleFormat,
        timing: PipeWireTiming,
    ) -> Result<Self> {
        if channels == 0 {
            return Err(Error::Eval(
                "PipeWire node channel count must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id: Symbol::new(id.into()),
            client_name: client_name.into(),
            node_name: node_name.into(),
            direction,
            channels,
            sample_format,
            timing,
        })
    }

    /// Returns the node's stable id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the owning client name (for example `PipeWire` or `SIM`).
    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    /// Returns the human-readable node name.
    pub fn node_name(&self) -> &str {
        &self.node_name
    }

    /// Returns the node's I/O direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the node's channel count.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Returns the node's PCM sample format.
    pub fn sample_format(&self) -> PcmSampleFormat {
        self.sample_format
    }

    /// Returns the node's timing metadata.
    pub fn timing(&self) -> PipeWireTiming {
        self.timing
    }

    /// Builds the [`PcmSpec`] for this node from its format and sample rate.
    pub fn spec(&self) -> Result<PcmSpec> {
        match self.sample_format {
            PcmSampleFormat::I16 => PcmSpec::i16(self.channels, self.timing.sample_rate_hz),
            PcmSampleFormat::F32 => PcmSpec::f32(self.channels, self.timing.sample_rate_hz),
        }
    }

    /// Reports whether the node can serve the `requested` direction.
    ///
    /// True when the node's direction matches exactly or the node is duplex.
    pub fn is_compatible_with(&self, requested: HostDirection) -> bool {
        self.direction == requested || self.direction == HostDirection::Duplex
    }

    /// Builds one [`PipeWirePort`] per channel, named by the node direction.
    ///
    /// Ports are named `capture_N`, `playback_N`, or `duplex_N` and carry the
    /// node's direction and a `{id}/{name}` port id.
    pub fn ports(&self) -> Vec<PipeWirePort> {
        (0..self.channels)
            .map(|channel| {
                let name = match self.direction {
                    HostDirection::Input => format!("capture_{channel}"),
                    HostDirection::Output => format!("playback_{channel}"),
                    HostDirection::Duplex => format!("duplex_{channel}"),
                };
                PipeWirePort::new(
                    Symbol::new(format!("{}/{}", self.id, name)),
                    self.id.clone(),
                    name,
                    self.direction,
                    channel,
                )
            })
            .collect()
    }
}

impl PipeWirePort {
    /// Builds a port from its id, owning node, name, direction, and channel.
    pub fn new(
        id: Symbol,
        node: Symbol,
        name: impl Into<String>,
        direction: HostDirection,
        channel: usize,
    ) -> Self {
        Self {
            id,
            node,
            name: name.into(),
            direction,
            channel,
        }
    }

    /// Returns the port's stable id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the id of the node that owns this port.
    pub fn node(&self) -> &Symbol {
        &self.node
    }

    /// Returns the port name (for example `playback_0`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the port's I/O direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the zero-based channel index the port carries.
    pub fn channel(&self) -> usize {
        self.channel
    }
}

/// Linux simple-audio backend priority once native PipeWire exists.
///
/// Returns host-backend symbols in preference order: PipeWire first, then
/// PortAudio, RtAudio, and ALSA.
///
/// # Examples
///
/// ```
/// use sim_lib_stream_pipewire::{linux_audio_backend_priority, pipewire_backend_symbol};
///
/// let priority = linux_audio_backend_priority();
/// assert_eq!(priority.first(), Some(&pipewire_backend_symbol()));
/// ```
pub fn linux_audio_backend_priority() -> Vec<Symbol> {
    vec![
        pipewire_backend_symbol(),
        Symbol::qualified("stream/host", "portaudio"),
        Symbol::qualified("stream/host", "rtaudio"),
        Symbol::qualified("stream/host", "alsa"),
    ]
}
