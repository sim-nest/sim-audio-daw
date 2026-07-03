use sim_lib_stream_core::{ClockDomain, DomainBridgeDescriptor};

use crate::{PortDecl, PortDir, PortMedia, ProcessBlock, Processor};

/// A [`Processor`] that bridges between clock domains or rate contracts.
///
/// Wraps a [`DomainBridgeDescriptor`] and carries audio through unchanged or
/// forwards events, depending on its [`PortMedia`]. Its declared latency and
/// clock domain come from the descriptor's output rate.
#[derive(Clone, Debug)]
pub struct DomainBridgeProcessor {
    descriptor: DomainBridgeDescriptor,
    media: PortMedia,
}

impl DomainBridgeProcessor {
    /// Creates a bridge processor from a descriptor and port media kind.
    pub fn new(descriptor: DomainBridgeDescriptor, media: PortMedia) -> Self {
        Self { descriptor, media }
    }

    /// Creates an audio resampling bridge from `input_hz` to `output_hz`.
    pub fn resampler(input_hz: u32, output_hz: u32) -> sim_kernel::Result<Self> {
        Ok(Self::new(
            DomainBridgeDescriptor::resampler(input_hz, output_hz)?,
            PortMedia::Audio,
        ))
    }

    /// Creates an event jitter-buffer bridge tolerating `max_late_packets`.
    pub fn jitter_buffer(max_late_packets: u32) -> Self {
        Self::new(
            DomainBridgeDescriptor::jitter_buffer(max_late_packets),
            PortMedia::Event,
        )
    }

    /// Creates an audio latency-compensation delay of `frames` frames.
    pub fn latency_comp_delay(frames: u64) -> Self {
        Self::new(
            DomainBridgeDescriptor::latency_comp_delay(frames),
            PortMedia::Audio,
        )
    }

    /// Creates an event-media rate gate bridging from `input_domain`.
    pub fn event_rate_gate(input_domain: ClockDomain) -> sim_kernel::Result<Self> {
        Ok(Self::new(
            DomainBridgeDescriptor::event_rate_gate(input_domain)?,
            PortMedia::Event,
        ))
    }

    /// Creates a control-media rate gate bridging from `input_domain`.
    pub fn control_rate_gate(input_domain: ClockDomain) -> sim_kernel::Result<Self> {
        Ok(Self::new(
            DomainBridgeDescriptor::event_rate_gate(input_domain)?,
            PortMedia::Control,
        ))
    }

    /// Returns the underlying domain-bridge descriptor.
    pub fn bridge_descriptor(&self) -> &DomainBridgeDescriptor {
        &self.descriptor
    }

    /// Returns the port media kind this bridge carries.
    pub fn media(&self) -> PortMedia {
        self.media
    }
}

impl Processor for DomainBridgeProcessor {
    fn prepare(&mut self, _cfg: crate::PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        match self.media {
            PortMedia::Audio => copy_audio(block),
            PortMedia::Control | PortMedia::Event => {
                for event in block.in_events {
                    let _ = block.out_events.push(*event);
                }
            }
        }
    }

    fn clock_domain(&self) -> ClockDomain {
        self.descriptor.output_rate().clock_domain()
    }

    fn latency_class(&self) -> sim_lib_stream_core::LatencyClass {
        self.descriptor.output_rate().latency_class()
    }

    fn realtime_pin(&self) -> bool {
        false
    }

    fn ports(&self, in_channels: u16, out_channels: u16) -> Vec<PortDecl> {
        let mut ports = Vec::new();
        if in_channels > 0 {
            ports.push(
                PortDecl::new("in", self.media, PortDir::In, in_channels)
                    .with_rate_contract(self.descriptor.input_rate()),
            );
        }
        if out_channels > 0 {
            ports.push(
                PortDecl::new("out", self.media, PortDir::Out, out_channels)
                    .with_rate_contract(self.descriptor.output_rate()),
            );
        }
        ports
    }

    fn tail_frames(&self) -> u64 {
        self.descriptor.latency().frame_count()
    }
}

fn copy_audio(block: &mut ProcessBlock<'_>) {
    let frames = block.frames as usize;
    for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
        output[..frames].copy_from_slice(&input[..frames]);
    }
}
