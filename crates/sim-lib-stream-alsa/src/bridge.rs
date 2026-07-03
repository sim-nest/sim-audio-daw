use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};
use sim_lib_stream_audio::{
    PcmBuffer, PcmSampleFormat, PcmSpec, f32_interleaved_to_planar, f32_planar_to_interleaved,
    f32_samples_to_i16, i16_samples_to_f32,
};
use sim_lib_stream_core::{PcmPacket, PushResult, StreamPacket};
use sim_lib_stream_host::HostCallbackQueue;

/// Drives an audio graph processor from an ALSA playback callback.
#[derive(Debug)]
pub struct AlsaPlaybackBridge<P> {
    processor: P,
    spec: PcmSpec,
    max_block_frames: u32,
    scratch: BlockArena,
    sample_pos: u64,
}

/// Converts ALSA capture buffers into PCM stream packets.
#[derive(Clone)]
pub struct AlsaCaptureBridge {
    queue: HostCallbackQueue,
    spec: PcmSpec,
}

impl<P: Processor> AlsaPlaybackBridge<P> {
    /// Builds a playback bridge, preparing `processor` for the given spec.
    ///
    /// `max_block_frames` is the largest callback block the bridge will accept
    /// and must be greater than zero. The processor is prepared with the spec's
    /// sample rate, channel count, and this block bound.
    pub fn new(mut processor: P, spec: PcmSpec, max_block_frames: u32) -> Result<Self> {
        if max_block_frames == 0 {
            return Err(Error::Eval(
                "ALSA playback max block frames must be greater than zero".to_owned(),
            ));
        }
        processor.prepare(PrepareConfig::new(
            spec.sample_rate_hz(),
            max_block_frames,
            0,
            checked_channels(spec.channels(), "playback output")?,
        ));
        Ok(Self {
            processor,
            spec,
            max_block_frames,
            scratch: BlockArena::with_f32_capacity(max_block_frames as usize * spec.channels()),
            sample_pos: 0,
        })
    }

    /// Renders `frames` of audio into a `PcmBuffer` in the spec's format.
    ///
    /// F32 output is returned directly; I16 output is converted from the
    /// rendered F32 samples.
    pub fn render_buffer(&mut self, frames: usize) -> Result<PcmBuffer> {
        let interleaved = self.render_interleaved_f32(frames)?;
        match self.spec.sample_format() {
            PcmSampleFormat::F32 => PcmBuffer::f32(self.spec, frames, interleaved),
            PcmSampleFormat::I16 => {
                PcmBuffer::i16(self.spec, frames, f32_samples_to_i16(&interleaved)?)
            }
        }
    }

    /// Renders `frames` of audio and returns interleaved F32 samples.
    ///
    /// Errors when `frames` exceeds the configured `max_block_frames`. The
    /// processor runs over a single `ProcessBlock` with a playing transport;
    /// the bridge's sample position advances by `frames`.
    pub fn render_interleaved_f32(&mut self, frames: usize) -> Result<Vec<f32>> {
        if frames > self.max_block_frames as usize {
            return Err(Error::Eval(format!(
                "ALSA playback callback received {frames} frames, max is {}",
                self.max_block_frames
            )));
        }
        let mut output_planar = vec![vec![0.0; frames]; self.spec.channels()];
        {
            let input_refs: [&[f32]; 0] = [];
            let mut output_refs = output_planar
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            self.scratch.reset();
            let mut event_sink = NullEventSink;
            let mut block = ProcessBlock {
                frames: frames as u32,
                in_audio: &input_refs,
                out_audio: &mut output_refs,
                in_events: &[],
                out_events: &mut event_sink,
                transport: Transport {
                    playing: true,
                    sample_pos: self.sample_pos,
                    tempo_bpm: 120.0,
                    ppq_pos: 0.0,
                },
                scratch: &mut self.scratch,
            };
            block.validate_audio_lanes()?;
            self.processor.process(&mut block);
            block.validate_audio_lanes()?;
        }
        self.sample_pos = self.sample_pos.saturating_add(frames as u64);
        f32_planar_to_interleaved(&output_planar)
    }

    /// Returns the running sample position across rendered blocks.
    pub fn sample_pos(&self) -> u64 {
        self.sample_pos
    }

    /// Resets the processor, sample position, and scratch arena.
    pub fn reset(&mut self) {
        self.processor.reset();
        self.sample_pos = 0;
        self.scratch.reset();
    }
}

impl AlsaCaptureBridge {
    /// Builds a capture bridge that pushes packets onto `queue` in `spec`.
    pub fn new(queue: HostCallbackQueue, spec: PcmSpec) -> Self {
        Self { queue, spec }
    }

    /// Captures interleaved F32 `samples` into a PCM packet on the queue.
    ///
    /// The sample count must be a multiple of the spec's channel count. The
    /// packet is encoded in the spec's sample format (F32 directly, or I16 by
    /// conversion). Returns the queue's `PushResult`.
    pub fn capture_interleaved_f32(&self, samples: &[f32]) -> Result<PushResult> {
        let frames = samples_to_frames(samples.len(), self.spec.channels())?;
        let planar = f32_interleaved_to_planar(samples, self.spec.channels())?;
        let packet_samples = f32_planar_to_interleaved(&planar)?;
        let packet = match self.spec.sample_format() {
            PcmSampleFormat::F32 => PcmPacket::f32(self.spec.channels(), frames, packet_samples)?,
            PcmSampleFormat::I16 => PcmPacket::i16(
                self.spec.channels(),
                frames,
                f32_samples_to_i16(&packet_samples)?,
            )?,
        };
        self.queue.callback_packet(StreamPacket::Pcm(packet))
    }

    /// Captures interleaved I16 `samples` into a PCM packet on the queue.
    ///
    /// The sample count must be a multiple of the spec's channel count. The
    /// packet is encoded in the spec's sample format (I16 directly, or F32 by
    /// conversion). Returns the queue's `PushResult`.
    pub fn capture_interleaved_i16(&self, samples: &[i16]) -> Result<PushResult> {
        let frames = samples_to_frames(samples.len(), self.spec.channels())?;
        let f32_samples = i16_samples_to_f32(samples);
        let packet = match self.spec.sample_format() {
            PcmSampleFormat::F32 => PcmPacket::f32(self.spec.channels(), frames, f32_samples)?,
            PcmSampleFormat::I16 => PcmPacket::i16(self.spec.channels(), frames, samples.to_vec())?,
        };
        self.queue.callback_packet(StreamPacket::Pcm(packet))
    }

    /// Returns the PCM spec the bridge encodes packets in.
    pub fn spec(&self) -> PcmSpec {
        self.spec
    }
}

fn samples_to_frames(samples: usize, channels: usize) -> Result<usize> {
    if channels == 0 {
        return Err(Error::Eval(
            "ALSA capture channel count must be greater than zero".to_owned(),
        ));
    }
    if !samples.is_multiple_of(channels) {
        return Err(Error::Eval(format!(
            "ALSA capture sample length {samples} is not divisible by channels {channels}"
        )));
    }
    Ok(samples / channels)
}

fn checked_channels(channels: usize, role: &str) -> Result<u16> {
    u16::try_from(channels)
        .map_err(|_| Error::Eval(format!("ALSA {role} channel count exceeds u16")))
}
