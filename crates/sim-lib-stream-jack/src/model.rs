use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::Transport;
use sim_lib_stream_clock::Clock;
use sim_lib_stream_core::StreamMedia;
use sim_lib_stream_host::HostDirection;

/// JACK timing metadata for a registered client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JackTiming {
    sample_rate_hz: u32,
    block_frames: usize,
    input_latency_frames: u32,
    output_latency_frames: u32,
}

/// Snapshot of JACK transport state for one process callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JackTransportState {
    rolling: bool,
    sample_pos: u64,
    tempo_bpm: f64,
    ppq_pos: f64,
}

/// SIM-visible JACK client with registered audio and MIDI ports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JackClient {
    id: Symbol,
    name: String,
    timing: JackTiming,
    audio_inputs: usize,
    audio_outputs: usize,
    midi_inputs: usize,
    midi_outputs: usize,
}

/// Routable JACK port owned by a client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JackPort {
    id: Symbol,
    client: Symbol,
    name: String,
    media: StreamMedia,
    direction: HostDirection,
    index: usize,
}

impl JackTiming {
    /// Builds timing metadata for a JACK client.
    ///
    /// # Errors
    ///
    /// Returns an error when `sample_rate_hz` or `block_frames` is zero.
    pub fn new(
        sample_rate_hz: u32,
        block_frames: usize,
        input_latency_frames: u32,
        output_latency_frames: u32,
    ) -> Result<Self> {
        if sample_rate_hz == 0 {
            return Err(Error::Eval(
                "JACK sample rate must be greater than zero".to_owned(),
            ));
        }
        if block_frames == 0 {
            return Err(Error::Eval(
                "JACK block size must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            sample_rate_hz,
            block_frames,
            input_latency_frames,
            output_latency_frames,
        })
    }

    /// Returns the pro-audio default timing: 48 kHz, 128-frame blocks, and
    /// 128-frame input and output latency.
    pub fn pro_audio_default() -> Self {
        Self::new(48_000, 128, 128, 128).expect("valid JACK timing")
    }

    /// Returns the sample rate in hertz.
    pub fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }

    /// Returns the JACK block size in frames.
    pub fn block_frames(self) -> usize {
        self.block_frames
    }

    /// Returns the reported input (capture) latency in frames.
    pub fn input_latency_frames(self) -> u32 {
        self.input_latency_frames
    }

    /// Returns the reported output (playback) latency in frames.
    pub fn output_latency_frames(self) -> u32 {
        self.output_latency_frames
    }

    /// Builds a frame-counting [`Clock`] keyed by [`jack_clock_symbol`] at this
    /// timing's sample rate.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying clock rejects the rate.
    pub fn frame_clock(self) -> Result<Clock> {
        Clock::frame(jack_clock_symbol(), u64::from(self.sample_rate_hz))
    }
}

impl JackTransportState {
    /// Builds a stopped transport at `sample_pos` with a default 120 BPM tempo
    /// and a zero PPQ position.
    pub fn stopped(sample_pos: u64) -> Self {
        Self {
            rolling: false,
            sample_pos,
            tempo_bpm: 120.0,
            ppq_pos: 0.0,
        }
    }

    /// Builds a rolling transport at `sample_pos` with the given tempo and PPQ
    /// position.
    ///
    /// # Errors
    ///
    /// Returns an error when `tempo_bpm` is non-finite or not positive, or when
    /// `ppq_pos` is non-finite.
    pub fn rolling(sample_pos: u64, tempo_bpm: f64, ppq_pos: f64) -> Result<Self> {
        if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
            return Err(Error::Eval(
                "JACK transport tempo must be finite and positive".to_owned(),
            ));
        }
        if !ppq_pos.is_finite() {
            return Err(Error::Eval(
                "JACK transport PPQ position must be finite".to_owned(),
            ));
        }
        Ok(Self {
            rolling: true,
            sample_pos,
            tempo_bpm,
            ppq_pos,
        })
    }

    /// Returns whether the transport is rolling (playing).
    pub fn rolling_flag(self) -> bool {
        self.rolling
    }

    /// Returns the transport position in sample frames.
    pub fn sample_pos(self) -> u64 {
        self.sample_pos
    }

    /// Returns the transport tempo in beats per minute.
    pub fn tempo_bpm(self) -> f64 {
        self.tempo_bpm
    }

    /// Returns the transport position in pulses per quarter note.
    pub fn ppq_pos(self) -> f64 {
        self.ppq_pos
    }

    /// Converts this snapshot into the audio-graph [`Transport`] consumed by a
    /// process block.
    pub fn to_graph_transport(self) -> Transport {
        Transport {
            playing: self.rolling,
            sample_pos: self.sample_pos,
            tempo_bpm: self.tempo_bpm,
            ppq_pos: self.ppq_pos,
        }
    }
}

impl JackClient {
    /// Builds a JACK client with the given name, timing, and port counts.
    ///
    /// The client id is derived from `name` as `jack/<name>/client`.
    ///
    /// # Errors
    ///
    /// Returns an error when `name` is empty or when the client registers no
    /// audio ports (both audio counts are zero).
    pub fn new(
        name: impl Into<String>,
        timing: JackTiming,
        audio_inputs: usize,
        audio_outputs: usize,
        midi_inputs: usize,
        midi_outputs: usize,
    ) -> Result<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(Error::Eval("JACK client name must not be empty".to_owned()));
        }
        if audio_inputs == 0 && audio_outputs == 0 {
            return Err(Error::Eval(
                "JACK client must register at least one audio port".to_owned(),
            ));
        }
        Ok(Self {
            id: Symbol::new(format!("jack/{name}/client")),
            name,
            timing,
            audio_inputs,
            audio_outputs,
            midi_inputs,
            midi_outputs,
        })
    }

    /// Builds the SIM default client: name `SIM`, [`JackTiming::pro_audio_default`],
    /// two audio inputs and outputs, and one MIDI input and output.
    ///
    /// # Errors
    ///
    /// Returns an error if the constructed client is rejected (it is not under
    /// the default configuration).
    pub fn sim_default() -> Result<Self> {
        Self::new("SIM", JackTiming::pro_audio_default(), 2, 2, 1, 1)
    }

    /// Returns the client's stable id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the client's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the client's timing metadata.
    pub fn timing(&self) -> JackTiming {
        self.timing
    }

    /// Returns the number of registered audio input ports.
    pub fn audio_inputs(&self) -> usize {
        self.audio_inputs
    }

    /// Returns the number of registered audio output ports.
    pub fn audio_outputs(&self) -> usize {
        self.audio_outputs
    }

    /// Returns the number of registered MIDI input ports.
    pub fn midi_inputs(&self) -> usize {
        self.midi_inputs
    }

    /// Returns the number of registered MIDI output ports.
    pub fn midi_outputs(&self) -> usize {
        self.midi_outputs
    }

    /// Returns the client's audio direction inferred from its port counts.
    ///
    /// Both-sided clients (and the degenerate no-audio case) report
    /// [`HostDirection::Duplex`].
    pub fn direction(&self) -> HostDirection {
        match (self.audio_inputs > 0, self.audio_outputs > 0) {
            (true, true) => HostDirection::Duplex,
            (true, false) => HostDirection::Input,
            (false, true) => HostDirection::Output,
            (false, false) => HostDirection::Duplex,
        }
    }

    /// Returns whether this client can serve a stream in the `requested`
    /// direction.
    ///
    /// A client matches its own direction, and a duplex client matches any
    /// request.
    pub fn is_compatible_with(&self, requested: HostDirection) -> bool {
        self.direction() == requested || self.direction() == HostDirection::Duplex
    }

    /// Returns the client's routable ports: audio inputs, audio outputs, MIDI
    /// inputs, then MIDI outputs, each numbered from zero.
    pub fn ports(&self) -> Vec<JackPort> {
        let mut ports = Vec::new();
        ports.extend(self.audio_ports(HostDirection::Input, self.audio_inputs, "audio_in"));
        ports.extend(self.audio_ports(HostDirection::Output, self.audio_outputs, "audio_out"));
        ports.extend(self.midi_ports(HostDirection::Input, self.midi_inputs, "midi_in"));
        ports.extend(self.midi_ports(HostDirection::Output, self.midi_outputs, "midi_out"));
        ports
    }

    fn audio_ports(&self, direction: HostDirection, count: usize, stem: &str) -> Vec<JackPort> {
        self.numbered_ports(StreamMedia::Pcm, direction, count, stem)
    }

    fn midi_ports(&self, direction: HostDirection, count: usize, stem: &str) -> Vec<JackPort> {
        self.numbered_ports(StreamMedia::Midi, direction, count, stem)
    }

    fn numbered_ports(
        &self,
        media: StreamMedia,
        direction: HostDirection,
        count: usize,
        stem: &str,
    ) -> Vec<JackPort> {
        (0..count)
            .map(|index| {
                let name = format!("{stem}_{index}");
                JackPort::new(
                    Symbol::new(format!("jack/{}/{}", self.name, name)),
                    self.id.clone(),
                    name,
                    media,
                    direction,
                    index,
                )
            })
            .collect()
    }
}

impl JackPort {
    /// Builds a port with the given id, owning client, name, media, direction,
    /// and per-direction index.
    pub fn new(
        id: Symbol,
        client: Symbol,
        name: impl Into<String>,
        media: StreamMedia,
        direction: HostDirection,
        index: usize,
    ) -> Self {
        Self {
            id,
            client,
            name: name.into(),
            media,
            direction,
            index,
        }
    }

    /// Returns the port's stable id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the id symbol of the client that owns this port.
    pub fn client(&self) -> &Symbol {
        &self.client
    }

    /// Returns the port's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the port's stream media (PCM audio or MIDI).
    pub fn media(&self) -> StreamMedia {
        self.media
    }

    /// Returns the port's direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the port's index among same-direction ports of its media.
    pub fn index(&self) -> usize {
        self.index
    }
}

/// Returns the symbol that identifies the JACK frame clock (`clock:jack`).
pub fn jack_clock_symbol() -> Symbol {
    Symbol::qualified("clock", "jack")
}
