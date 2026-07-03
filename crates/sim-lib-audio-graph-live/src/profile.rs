use sim_kernel::{Error, Result, Symbol, Tick};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, LatencyClass, StreamCapability, StreamDirection, StreamEnvelope,
    StreamMedia, StreamMetadata, StreamPacket, TransportProfile,
};

/// One logical stream lane carried between the live runner and the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveStreamLane {
    /// Incoming audio (a source into the graph).
    AudioInput,
    /// Outgoing audio (a sink from the graph).
    AudioOutput,
    /// MIDI control events.
    Midi,
    /// Control-rate parameter updates.
    Parameter,
    /// Diagnostic packets.
    Diagnostic,
}

impl LiveStreamLane {
    const ALL: [Self; 5] = [
        Self::AudioInput,
        Self::AudioOutput,
        Self::Midi,
        Self::Parameter,
        Self::Diagnostic,
    ];

    /// Returns every lane in a stable order.
    pub fn all() -> &'static [Self] {
        &Self::ALL
    }

    /// Returns the stable wire label for this lane.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::AudioInput => "audio-input",
            Self::AudioOutput => "audio-output",
            Self::Midi => "midi",
            Self::Parameter => "parameter",
            Self::Diagnostic => "diagnostic",
        }
    }

    /// Returns the stream id symbol for this lane.
    pub fn stream_id(self) -> Symbol {
        Symbol::qualified("stream/live", self.wire_label())
    }

    /// Returns the stream media kind this lane carries.
    pub fn media(self) -> StreamMedia {
        match self {
            Self::AudioInput | Self::AudioOutput => StreamMedia::Pcm,
            Self::Midi => StreamMedia::Midi,
            Self::Parameter => StreamMedia::Data,
            Self::Diagnostic => StreamMedia::Diagnostic,
        }
    }

    /// Returns the stream direction (source or sink) for this lane.
    pub fn direction(self) -> StreamDirection {
        match self {
            Self::AudioOutput => StreamDirection::Sink,
            Self::AudioInput | Self::Midi | Self::Parameter | Self::Diagnostic => {
                StreamDirection::Source
            }
        }
    }

    /// Returns the clock domain this lane runs in.
    pub fn clock_domain(self) -> ClockDomain {
        match self {
            Self::AudioInput | Self::AudioOutput => ClockDomain::Sample,
            Self::Midi => ClockDomain::MidiTick,
            Self::Parameter => ClockDomain::Control,
            Self::Diagnostic => ClockDomain::Block,
        }
    }

    /// Builds the stream metadata for this lane with a bounded buffer of
    /// `capacity` items.
    pub fn metadata(self, capacity: usize) -> Result<StreamMetadata> {
        Ok(StreamMetadata::new(
            self.stream_id(),
            self.media(),
            self.direction(),
            self.clock_domain().symbol(),
            BufferPolicy::bounded(capacity)?,
        ))
    }

    /// Wraps a packet in an envelope using the realtime local audio profile.
    pub fn realtime_envelope(
        self,
        sequence: u64,
        ticks: Vec<Tick>,
        packet: StreamPacket,
    ) -> Result<StreamEnvelope> {
        self.envelope(
            sequence,
            ticks,
            realtime_local_audio_profile(),
            Vec::new(),
            packet,
        )
    }

    /// Wraps a packet in an envelope using the buffered PCM preview profile.
    pub fn buffered_preview_envelope(
        self,
        sequence: u64,
        ticks: Vec<Tick>,
        packet: StreamPacket,
    ) -> Result<StreamEnvelope> {
        self.envelope(
            sequence,
            ticks,
            buffered_pcm_preview_profile(),
            Vec::new(),
            packet,
        )
    }

    /// Wraps a packet in an envelope using the LAN buffered audio preview
    /// profile.
    pub fn lan_buffered_preview_envelope(
        self,
        sequence: u64,
        ticks: Vec<Tick>,
        packet: StreamPacket,
    ) -> Result<StreamEnvelope> {
        self.envelope(
            sequence,
            ticks,
            lan_buffered_audio_preview_profile(),
            Vec::new(),
            packet,
        )
    }

    fn envelope(
        self,
        sequence: u64,
        ticks: Vec<Tick>,
        profile: TransportProfile,
        diagnostics: Vec<Symbol>,
        packet: StreamPacket,
    ) -> Result<StreamEnvelope> {
        StreamEnvelope::new(
            self.stream_id(),
            packet_id(self, sequence),
            self.media(),
            self.direction(),
            sequence,
            ticks,
            self.clock_domain(),
            profile,
            diagnostics,
            packet,
        )
    }
}

/// Returns the realtime local audio transport profile.
pub fn realtime_local_audio_profile() -> TransportProfile {
    TransportProfile::realtime_local_audio()
}

/// Returns the buffered PCM preview transport profile.
pub fn buffered_pcm_preview_profile() -> TransportProfile {
    TransportProfile::buffered_pcm_preview()
}

/// Returns the LAN buffered audio preview transport profile.
pub fn lan_buffered_audio_preview_profile() -> TransportProfile {
    TransportProfile::lan_buffered_audio_preview()
}

/// Returns the LAN render-return transport profile.
pub fn lan_render_return_profile() -> TransportProfile {
    TransportProfile::lan_render_return()
}

/// Validates that a profile may enter the realtime local audio callback.
///
/// Rejects remote streams and requires realtime, bounded, sample-exact
/// capabilities and the `realtime-local-audio` profile name.
pub fn validate_realtime_local_audio_profile(profile: &TransportProfile) -> Result<()> {
    if profile.has_capability(StreamCapability::Remote)
        || profile.latency_class() == LatencyClass::RemoteCollaboration
    {
        return Err(Error::Eval(
            "remote streams cannot enter the realtime local audio callback".to_owned(),
        ));
    }
    if !profile.has_capability(StreamCapability::Realtime) {
        return Err(Error::Eval(
            "realtime local audio requires realtime transport capability".to_owned(),
        ));
    }
    if !profile.has_capability(StreamCapability::Bounded) {
        return Err(Error::Eval(
            "realtime local audio requires bounded transport capability".to_owned(),
        ));
    }
    if profile.latency_class() != LatencyClass::SampleExact {
        return Err(Error::Eval(
            "realtime local audio requires sample-exact latency".to_owned(),
        ));
    }
    if profile.name() != &Symbol::qualified("stream/profile", "realtime-local-audio") {
        return Err(Error::Eval(
            "callback entry requires the realtime-local-audio profile".to_owned(),
        ));
    }
    Ok(())
}

/// Refuses tunneling an unbuffered or realtime profile through the audio
/// callback, directing callers to the LAN buffered audio preview profile.
pub fn refuse_unbuffered_audio_callback_tunnel(profile: &TransportProfile) -> Result<()> {
    if profile.name() == &Symbol::qualified("stream/profile", "realtime-local-audio")
        || profile.has_capability(StreamCapability::Realtime)
    {
        return Err(Error::Eval(format!(
            "unbuffered audio callback tunneling is refused by default for {}; use stream/profile/lan-buffered-audio-preview",
            profile.name().as_qualified_str()
        )));
    }
    Ok(())
}

fn packet_id(lane: LiveStreamLane, sequence: u64) -> Symbol {
    Symbol::qualified(
        "stream/live-packet",
        format!("{}#{sequence}", lane.wire_label()),
    )
}
