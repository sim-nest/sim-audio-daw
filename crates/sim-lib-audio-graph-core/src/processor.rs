use sim_kernel::Result;
use sim_lib_stream_core::{ClockDomain, LatencyClass};

use crate::{PortDecl, PortDir, PortMedia, ProcessBlock};

/// Configuration handed to each processor at [`prepare`](Processor::prepare)
/// time: the sample rate, maximum block size, and channel counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrepareConfig {
    /// Sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Maximum number of frames in any single process block.
    pub max_block_frames: u32,
    /// Number of input channels.
    pub in_channels: u16,
    /// Number of output channels.
    pub out_channels: u16,
}

impl PrepareConfig {
    /// Creates a prepare configuration from its parts.
    pub fn new(
        sample_rate_hz: u32,
        max_block_frames: u32,
        in_channels: u16,
        out_channels: u16,
    ) -> Self {
        Self {
            sample_rate_hz,
            max_block_frames,
            in_channels,
            out_channels,
        }
    }
}

/// Transport state delivered to processors per block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transport {
    /// Whether transport is playing.
    pub playing: bool,
    /// Playhead position in samples.
    pub sample_pos: u64,
    /// Tempo in beats per minute.
    pub tempo_bpm: f64,
    /// Playhead position in quarter notes (pulses per quarter position).
    pub ppq_pos: f64,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            playing: false,
            sample_pos: 0,
            tempo_bpm: 120.0,
            ppq_pos: 0.0,
        }
    }
}

/// An event delivered to or emitted by a processor within a block, timestamped
/// by a sample `offset` into the block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockEvent<'a> {
    /// A short MIDI message of up to three bytes.
    Midi {
        /// Sample offset into the block.
        offset: u32,
        /// Message bytes (only the first `len` are valid).
        bytes: [u8; 3],
        /// Number of valid bytes in `bytes`.
        len: u8,
    },
    /// A long MIDI message (for example sysex) borrowing its bytes.
    MidiLong {
        /// Sample offset into the block.
        offset: u32,
        /// Borrowed message bytes.
        bytes: &'a [u8],
    },
    /// A parameter-set event.
    ParamSet {
        /// Sample offset into the block.
        offset: u32,
        /// Parameter index.
        param: u32,
        /// New parameter value.
        value: f64,
    },
    /// A note-on event.
    NoteOn {
        /// Sample offset into the block.
        offset: u32,
        /// MIDI channel.
        channel: u8,
        /// MIDI key number.
        key: u8,
        /// Normalized velocity.
        velocity: f32,
    },
    /// A note-off event.
    NoteOff {
        /// Sample offset into the block.
        offset: u32,
        /// MIDI channel.
        channel: u8,
        /// MIDI key number.
        key: u8,
        /// Normalized release velocity.
        velocity: f32,
    },
}

/// Sink that accepts events emitted by a processor during a block.
pub trait EventSink {
    /// Pushes one event into the sink.
    fn push(&mut self, event: BlockEvent<'_>) -> Result<()>;
}

/// An [`EventSink`] that discards every event.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullEventSink;

impl EventSink for NullEventSink {
    fn push(&mut self, _event: BlockEvent<'_>) -> Result<()> {
        Ok(())
    }
}

/// A node behavior in the audio graph: prepared once, then processes blocks.
///
/// Implementors must supply [`prepare`](Processor::prepare),
/// [`reset`](Processor::reset), and [`process`](Processor::process); the
/// remaining methods describe clocking, latency, and ports and have sensible
/// defaults (sample-rate audio, block-local latency, realtime-pinned).
pub trait Processor: Send {
    /// Prepares the processor for a sample rate, block size, and channel layout.
    fn prepare(&mut self, cfg: PrepareConfig);

    /// Clears any internal state to a fresh start.
    fn reset(&mut self);

    /// Processes one block of audio and events in place.
    fn process(&mut self, block: &mut ProcessBlock<'_>);

    /// Returns the processor's clock domain (defaults to sample rate).
    fn clock_domain(&self) -> ClockDomain {
        ClockDomain::Sample
    }

    /// Returns the processor's latency class (defaults to block-local).
    fn latency_class(&self) -> LatencyClass {
        LatencyClass::BlockLocal
    }

    /// Returns whether the processor must run in the realtime thread (defaults
    /// to `true`).
    fn realtime_pin(&self) -> bool {
        true
    }

    /// Returns the processor's port declarations for the given channel counts
    /// (defaults to a single audio in/out pair).
    fn ports(&self, in_channels: u16, out_channels: u16) -> Vec<PortDecl> {
        default_processor_ports(in_channels, out_channels)
    }

    /// Returns the full [`ProcessorDescriptor`] assembled from this processor's
    /// clock, latency, pinning, and ports.
    fn descriptor(&self, in_channels: u16, out_channels: u16) -> ProcessorDescriptor {
        ProcessorDescriptor::new(
            self.clock_domain(),
            self.latency_class(),
            self.realtime_pin(),
            self.ports(in_channels, out_channels),
        )
    }

    /// Returns the processor's release tail length in frames (defaults to `0`).
    fn tail_frames(&self) -> u64 {
        0
    }
}

/// Static description of a processor's clocking, latency, pinning, and ports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessorDescriptor {
    clock_domain: ClockDomain,
    latency_class: LatencyClass,
    realtime_pin: bool,
    ports: Vec<PortDecl>,
}

impl ProcessorDescriptor {
    /// Creates a descriptor from its parts.
    pub fn new(
        clock_domain: ClockDomain,
        latency_class: LatencyClass,
        realtime_pin: bool,
        ports: Vec<PortDecl>,
    ) -> Self {
        Self {
            clock_domain,
            latency_class,
            realtime_pin,
            ports,
        }
    }

    /// Returns the processor's clock domain.
    pub fn clock_domain(&self) -> ClockDomain {
        self.clock_domain
    }

    /// Returns the processor's latency class.
    pub fn latency_class(&self) -> LatencyClass {
        self.latency_class
    }

    /// Returns whether the processor is realtime-pinned.
    pub fn realtime_pin(&self) -> bool {
        self.realtime_pin
    }

    /// Returns the processor's port declarations.
    pub fn ports(&self) -> &[PortDecl] {
        &self.ports
    }
}

fn default_processor_ports(in_channels: u16, out_channels: u16) -> Vec<PortDecl> {
    let mut ports = Vec::new();
    if in_channels > 0 {
        ports.push(PortDecl::new(
            "in",
            PortMedia::Audio,
            PortDir::In,
            in_channels,
        ));
    }
    if out_channels > 0 {
        ports.push(PortDecl::new(
            "out",
            PortMedia::Audio,
            PortDir::Out,
            out_channels,
        ));
    }
    ports
}
