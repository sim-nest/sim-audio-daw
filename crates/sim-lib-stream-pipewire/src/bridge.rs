use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};
use sim_lib_stream_audio::{
    PcmSampleFormat, PcmSpec, f32_interleaved_to_planar, f32_planar_to_interleaved,
    f32_samples_to_i16, i16_samples_to_f32,
};
use sim_lib_stream_core::{PcmPacket, PushResult, StreamPacket};
use sim_lib_stream_host::HostCallbackQueue;

/// Drives an audio processor from a PipeWire process callback.
#[derive(Debug)]
pub struct PipeWireGraphBridge<P> {
    processor: P,
    spec: PcmSpec,
    input_channels: usize,
    max_quantum_frames: u32,
    scratch: BlockArena,
    sample_pos: u64,
}

/// Records PipeWire capture buffers into a host callback queue.
#[derive(Clone)]
pub struct PipeWireCaptureBridge {
    queue: HostCallbackQueue,
    spec: PcmSpec,
}

impl<P: Processor> PipeWireGraphBridge<P> {
    /// Builds a bridge that drives `processor` at up to `max_quantum_frames`.
    ///
    /// Prepares the processor with the output `spec`'s sample rate and channel
    /// count plus `input_channels`, and sizes the scratch [`BlockArena`] for the
    /// widest lane. Errors if `max_quantum_frames` is zero or either channel
    /// count exceeds `u16`.
    pub fn new(
        mut processor: P,
        spec: PcmSpec,
        input_channels: usize,
        max_quantum_frames: u32,
    ) -> Result<Self> {
        if max_quantum_frames == 0 {
            return Err(Error::Eval(
                "PipeWire quantum must be greater than zero".to_owned(),
            ));
        }
        processor.prepare(PrepareConfig::new(
            spec.sample_rate_hz(),
            max_quantum_frames,
            checked_channels(input_channels, "input")?,
            checked_channels(spec.channels(), "output")?,
        ));
        Ok(Self {
            processor,
            spec,
            input_channels,
            max_quantum_frames,
            scratch: BlockArena::with_f32_capacity(
                max_quantum_frames as usize * spec.channels().max(input_channels).max(1),
            ),
            sample_pos: 0,
        })
    }

    /// Processes one callback of interleaved `f32` audio and returns the output.
    ///
    /// Deinterleaves `input` (or substitutes silence when `None`) into planar
    /// lanes, runs the processor over a [`ProcessBlock`] of `frames`, advances
    /// the running sample position, and reinterleaves the result. Errors if
    /// `frames` exceeds the configured maximum quantum.
    pub fn process_interleaved_f32(
        &mut self,
        input: Option<&[f32]>,
        frames: usize,
    ) -> Result<Vec<f32>> {
        if frames > self.max_quantum_frames as usize {
            return Err(Error::Eval(format!(
                "PipeWire callback received {frames} frames, max quantum is {}",
                self.max_quantum_frames
            )));
        }
        let input_planar = match input {
            Some(samples) => f32_interleaved_to_planar(samples, self.input_channels)?,
            None => vec![vec![0.0; frames]; self.input_channels],
        };
        let mut output_planar = vec![vec![0.0; frames]; self.spec.channels()];
        {
            let input_refs = input_planar.iter().map(Vec::as_slice).collect::<Vec<_>>();
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

    /// Returns the running sample position advanced by processed callbacks.
    pub fn sample_pos(&self) -> u64 {
        self.sample_pos
    }

    /// Resets the processor, the sample position, and the scratch arena.
    pub fn reset(&mut self) {
        self.processor.reset();
        self.sample_pos = 0;
        self.scratch.reset();
    }
}

impl PipeWireCaptureBridge {
    /// Builds a capture bridge that pushes packets into `queue` using `spec`.
    pub fn new(queue: HostCallbackQueue, spec: PcmSpec) -> Self {
        Self { queue, spec }
    }

    /// Records interleaved `f32` capture samples into the callback queue.
    ///
    /// Splits `samples` into frames per the spec channel count, encodes a
    /// [`PcmPacket`] in the spec's sample format (converting to `i16` when
    /// required), and enqueues it. Errors if the sample count is not a whole
    /// multiple of the channel count.
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

    /// Records interleaved `i16` capture samples into the callback queue.
    ///
    /// Splits `samples` into frames per the spec channel count and encodes a
    /// [`PcmPacket`] in the spec's sample format, converting to `f32` when the
    /// spec is float. Errors if the sample count is not a whole multiple of the
    /// channel count.
    pub fn capture_interleaved_i16(&self, samples: &[i16]) -> Result<PushResult> {
        let frames = samples_to_frames(samples.len(), self.spec.channels())?;
        let f32_samples = i16_samples_to_f32(samples);
        let packet = match self.spec.sample_format() {
            PcmSampleFormat::F32 => PcmPacket::f32(self.spec.channels(), frames, f32_samples)?,
            PcmSampleFormat::I16 => PcmPacket::i16(self.spec.channels(), frames, samples.to_vec())?,
        };
        self.queue.callback_packet(StreamPacket::Pcm(packet))
    }
}

fn samples_to_frames(samples: usize, channels: usize) -> Result<usize> {
    if channels == 0 {
        return Err(Error::Eval(
            "PipeWire capture channel count must be greater than zero".to_owned(),
        ));
    }
    if !samples.is_multiple_of(channels) {
        return Err(Error::Eval(format!(
            "PipeWire sample length {samples} is not divisible by channels {channels}"
        )));
    }
    Ok(samples / channels)
}

fn checked_channels(channels: usize, role: &str) -> Result<u16> {
    u16::try_from(channels)
        .map_err(|_| Error::Eval(format!("PipeWire {role} channel count exceeds u16")))
}
