use sim_lib_stream_core::{ClockDomain, DomainBridgeDescriptor};

use crate::{PortDecl, PortDir, PortMedia, ProcessBlock, Processor};

/// A [`Processor`] that carries domain-bridge metadata through a graph.
///
/// This node is metadata-only: it advertises a [`DomainBridgeDescriptor`] for
/// graph planning, copies audio, and forwards current-block events unchanged.
/// It does not resample audio, insert delay, or buffer events.
#[derive(Clone, Debug)]
pub struct DomainBridgeMetadataProcessor {
    descriptor: DomainBridgeDescriptor,
    media: PortMedia,
}

impl DomainBridgeMetadataProcessor {
    /// Creates a metadata bridge node from a descriptor and port media kind.
    pub fn new(descriptor: DomainBridgeDescriptor, media: PortMedia) -> Self {
        Self { descriptor, media }
    }

    /// Creates metadata for an audio sample-rate bridge from `input_hz` to `output_hz`.
    pub fn sample_rate_bridge_metadata(input_hz: u32, output_hz: u32) -> sim_kernel::Result<Self> {
        Ok(Self::new(
            DomainBridgeDescriptor::resampler(input_hz, output_hz)?,
            PortMedia::Audio,
        ))
    }

    /// Creates metadata for an event jitter bridge tolerating `max_late_packets`.
    pub fn event_jitter_bridge_metadata(max_late_packets: u32) -> Self {
        Self::new(
            DomainBridgeDescriptor::jitter_buffer(max_late_packets),
            PortMedia::Event,
        )
    }

    /// Creates metadata for an audio latency-compensation bridge of `frames` frames.
    pub fn audio_latency_bridge_metadata(frames: u64) -> Self {
        Self::new(
            DomainBridgeDescriptor::latency_comp_delay(frames),
            PortMedia::Audio,
        )
    }

    /// Creates metadata for an event-media block gate from `input_domain`.
    pub fn event_block_gate_metadata(input_domain: ClockDomain) -> sim_kernel::Result<Self> {
        Ok(Self::new(
            DomainBridgeDescriptor::event_rate_gate(input_domain)?,
            PortMedia::Event,
        ))
    }

    /// Creates metadata for a control-media block gate from `input_domain`.
    pub fn control_block_gate_metadata(input_domain: ClockDomain) -> sim_kernel::Result<Self> {
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

impl Processor for DomainBridgeMetadataProcessor {
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
        0
    }
}

fn copy_audio(block: &mut ProcessBlock<'_>) {
    let frames = block.frames as usize;
    for (channel, output) in block.out_audio.iter_mut().enumerate() {
        let output = &mut output[..frames];
        if let Some(input) = block.in_audio.get(channel) {
            output.copy_from_slice(&input[..frames]);
        } else {
            output.fill(0.0);
        }
    }
}
